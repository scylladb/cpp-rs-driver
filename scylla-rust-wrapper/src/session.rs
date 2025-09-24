use crate::argconv::*;
use crate::cass_error::*;
use crate::cass_metrics_types::CassMetrics;
use crate::cluster::CassCluster;
use crate::cql_types::data_type::get_column_type;
use crate::cql_types::uuid::CassUuid;
use crate::exec_profile::{CassExecProfile, ExecProfileName, PerStatementExecProfile};
use crate::future::{CassFuture, CassFutureResult, CassResultValue};
use crate::metadata::create_table_metadata;
use crate::metadata::{CassKeyspaceMeta, CassMaterializedViewMeta, CassSchemaMeta};
use crate::query_result::{CassResult, CassResultKind, CassResultMetadata};
use crate::runtime::Runtime;
use crate::statements::batch::CassBatch;
use crate::statements::prepared::CassPrepared;
use crate::statements::statement::{BoundStatement, CassStatement, SimpleQueryRowSerializer};
use crate::types::size_t;
use scylla::client::execution_profile::ExecutionProfileHandle;
use scylla::client::session::Session;
use scylla::client::session_builder::SessionBuilder;
use scylla::cluster::metadata::ColumnType;
use scylla::errors::ExecutionError;
use scylla::observability::metrics::MetricsError;
use scylla::policies::host_filter::HostFilter;
use scylla::response::PagingStateResponse;
use scylla::response::query_result::QueryResult;
use scylla::statement::unprepared::Statement;
use std::collections::HashMap;
use std::future::Future;
use std::ops::Deref;
use std::os::raw::c_char;
use std::sync::Arc;
use tokio::sync::RwLock;

pub(crate) struct CassConnectedSession {
    runtime: Arc<Runtime>,
    session: Session,
    exec_profile_map: HashMap<ExecProfileName, ExecutionProfileHandle>,
}

impl CassConnectedSession {
    pub(crate) fn resolve_exec_profile(
        &self,
        name: &ExecProfileName,
    ) -> Result<&ExecutionProfileHandle, (CassError, String)> {
        // Empty name means no execution profile set.
        self.exec_profile_map.get(name).ok_or_else(|| {
            (
                CassError::CASS_ERROR_LIB_EXECUTION_PROFILE_INVALID,
                format!("{} does not exist", name.deref()),
            )
        })
    }

    // Clippy claims it is possible to make this `async fn`, but it's terribly wrong,
    // because async fn can't have its future bound to a specific lifetime, which is
    // required in this case.
    #[allow(clippy::manual_async_fn)]
    fn get_or_resolve_profile_handle<'a>(
        &'a self,
        exec_profile: Option<&'a PerStatementExecProfile>,
    ) -> impl Future<Output = Result<Option<ExecutionProfileHandle>, (CassError, String)>> + 'a
    {
        async move {
            let Some(profile) = exec_profile else {
                return Ok(None);
            };
            let handle = profile.get_or_resolve_profile_handle(self).await?;
            Ok(Some(handle))
        }
    }

    fn connect(
        session: Arc<CassSession>,
        cluster: &CassCluster,
        keyspace: Option<String>,
    ) -> CassOwnedSharedPtr<CassFuture, CMut> {
        let session_builder = cluster.build_session_builder();
        let exec_profile_map = cluster.execution_profile_map().clone();
        let host_filter = cluster.build_host_filter();
        let cluster_client_id = cluster.get_client_id();

        let runtime = cluster.get_runtime();

        let fut = Self::connect_fut(
            Arc::clone(&runtime),
            session,
            session_builder,
            cluster_client_id,
            exec_profile_map,
            host_filter,
            keyspace,
        );

        CassFuture::make_raw(
            runtime,
            fut,
            #[cfg(cpp_integration_testing)]
            None,
        )
    }

    async fn connect_fut(
        runtime: Arc<Runtime>,
        session: Arc<CassSession>,
        session_builder_fut: impl Future<Output = SessionBuilder>,
        cluster_client_id: Option<uuid::Uuid>,
        exec_profile_builder_map: HashMap<ExecProfileName, CassExecProfile>,
        host_filter: Arc<dyn HostFilter>,
        keyspace: Option<String>,
    ) -> CassFutureResult {
        // This can sleep for a long time, but only if someone connects/closes session
        // from more than 1 thread concurrently, which is inherently stupid thing to do.
        let mut session_guard = session.write().await;

        if session_guard.connected.is_some() {
            return Err((
                CassError::CASS_ERROR_LIB_UNABLE_TO_CONNECT,
                "Already connecting, closing, or connected".msg(),
            ));
        }

        if let Some(cluster_client_id) = cluster_client_id {
            // If the user set a client id, use it instead of the random one.
            session_guard.client_id = cluster_client_id;
        }

        let mut session_builder = session_builder_fut.await;
        let default_profile = session_builder
            .config
            .default_execution_profile_handle
            .to_profile();

        let mut exec_profile_map = HashMap::with_capacity(exec_profile_builder_map.len());
        for (name, builder) in exec_profile_builder_map {
            exec_profile_map.insert(name, builder.build(&default_profile).await.into_handle());
        }

        if let Some(maybe_quoted_keyspace) = keyspace {
            // Handle case-sensitivity. If the keyspace name is enclosed in quotes, it is case-sensitive.
            let (unquoted_keyspace, case_sensitive) =
                if maybe_quoted_keyspace.starts_with('"') && maybe_quoted_keyspace.ends_with('"') {
                    // Keyspace is case-sensitive. We acknowledge that and remove the quotes,
                    // as the Rust Driver expects keyspace name without quotes and rejects non-alphanumeric characters.
                    let mut quoted_keyspace = maybe_quoted_keyspace;
                    quoted_keyspace.remove(0); // Remove the first quote.
                    quoted_keyspace.pop(); // Remove the last quote.
                    (quoted_keyspace, true)
                } else {
                    (maybe_quoted_keyspace, false)
                };

            session_builder = session_builder.use_keyspace(unquoted_keyspace, case_sensitive);
        }

        let session = session_builder
            .host_filter(host_filter)
            .build()
            .await
            .map_err(|err| (err.to_cass_error(), err.msg()))?;

        session_guard.connected = Some(CassConnectedSession {
            runtime,
            session,
            exec_profile_map,
        });
        Ok(CassResultValue::Empty)
    }

    fn close_fut(session_opt: Arc<RwLock<CassSessionInner>>) -> Arc<CassFuture> {
        let runtime = {
            let Ok(session_guard) = session_opt.try_read() else {
                return CassFuture::new_ready(Err((
                    CassError::CASS_ERROR_LIB_NO_HOSTS_AVAILABLE,
                    "Still connecting or already closing".msg(),
                )));
            };

            let Some(connected_session) = session_guard.connected.as_ref() else {
                return CassFuture::new_ready(Err((
                    CassError::CASS_ERROR_LIB_NO_HOSTS_AVAILABLE,
                    "Session is not connected".msg(),
                )));
            };

            Arc::clone(&connected_session.runtime)
        };

        CassFuture::new_from_future(
            runtime,
            async move {
                let mut session_guard = session_opt.write().await;
                if session_guard.connected.is_none() {
                    return Err((
                        CassError::CASS_ERROR_LIB_UNABLE_TO_CLOSE,
                        "Session is not connected".msg(),
                    ));
                }

                session_guard.connected = None;

                Ok(CassResultValue::Empty)
            },
            #[cfg(cpp_integration_testing)]
            None,
        )
    }
}

// Technically, we should not allow this struct to be public,
// but this would require a lot of changes in the codebase:
// CassSession would need to be a newtype wrapper around this struct
// instead of a type alias.
#[expect(unnameable_types)]
pub struct CassSessionInner {
    // This is an `Option` to allow the session to be closed.
    // If it is `None`, the session is closed.
    connected: Option<CassConnectedSession>,
    // This is the same that the CPP Driver does: it generates a random UUID v4
    // and stores it in the session. Upon connection, if the CassCluster
    // has a client_id set, it will be used instead.
    client_id: uuid::Uuid,
}

pub type CassSession = RwLock<CassSessionInner>;

impl FFI for CassSession {
    type Origin = FromArc;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_session_new() -> CassOwnedSharedPtr<CassSession, CMut> {
    let session = Arc::new(RwLock::new(CassSessionInner {
        connected: None,
        client_id: uuid::Uuid::new_v4(),
    }));
    ArcFFI::into_ptr(session)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_session_connect(
    session_raw: CassBorrowedSharedPtr<CassSession, CMut>,
    cluster_raw: CassBorrowedSharedPtr<CassCluster, CConst>,
) -> CassOwnedSharedPtr<CassFuture, CMut> {
    let Some(session) = ArcFFI::cloned_from_ptr(session_raw) else {
        tracing::error!("Provided null session pointer to cass_session_connect!");
        return ArcFFI::null();
    };
    let Some(cluster) = BoxFFI::as_ref(cluster_raw) else {
        tracing::error!("Provided null cluster pointer to cass_session_connect!");
        return ArcFFI::null();
    };

    CassConnectedSession::connect(session, cluster, None)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_session_connect_keyspace(
    session_raw: CassBorrowedSharedPtr<CassSession, CMut>,
    cluster_raw: CassBorrowedSharedPtr<CassCluster, CConst>,
    keyspace: *const c_char,
) -> CassOwnedSharedPtr<CassFuture, CMut> {
    unsafe { cass_session_connect_keyspace_n(session_raw, cluster_raw, keyspace, strlen(keyspace)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_session_connect_keyspace_n(
    session_raw: CassBorrowedSharedPtr<CassSession, CMut>,
    cluster_raw: CassBorrowedSharedPtr<CassCluster, CConst>,
    keyspace: *const c_char,
    keyspace_length: size_t,
) -> CassOwnedSharedPtr<CassFuture, CMut> {
    let Some(session) = ArcFFI::cloned_from_ptr(session_raw) else {
        tracing::error!("Provided null session pointer to cass_session_connect_keyspace_n!");
        return ArcFFI::null();
    };
    let Some(cluster) = BoxFFI::as_ref(cluster_raw) else {
        tracing::error!("Provided null cluster pointer to cass_session_connect_keyspace_n!");
        return ArcFFI::null();
    };
    let keyspace = unsafe { ptr_to_cstr_n(keyspace, keyspace_length) }.map(ToOwned::to_owned);

    CassConnectedSession::connect(session, cluster, keyspace)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_session_execute_batch(
    session_raw: CassBorrowedSharedPtr<CassSession, CMut>,
    batch_raw: CassBorrowedSharedPtr<CassBatch, CConst>,
) -> CassOwnedSharedPtr<CassFuture, CMut> {
    let Some(session_opt) = ArcFFI::cloned_from_ptr(session_raw) else {
        tracing::error!("Provided null session pointer to cass_session_execute_batch!");
        return ArcFFI::null();
    };
    let Some(batch_from_raw) = BoxFFI::as_ref(batch_raw) else {
        tracing::error!("Provided null batch pointer to cass_session_execute_batch!");
        return ArcFFI::null();
    };

    let Ok(session_guard) = session_opt.try_read_owned() else {
        return CassFuture::make_ready_raw(Err((
            CassError::CASS_ERROR_LIB_NO_HOSTS_AVAILABLE,
            "Session is not connected".msg(),
        )));
    };

    let mut state = batch_from_raw.state.clone();
    let batch_exec_profile = batch_from_raw.exec_profile.clone();

    let Some(connected_session) = session_guard.connected.as_ref() else {
        return CassFuture::make_ready_raw(Err((
            CassError::CASS_ERROR_LIB_NO_HOSTS_AVAILABLE,
            "Session is not connected".msg(),
        )));
    };

    let runtime = Arc::clone(&connected_session.runtime);

    let future = async move {
        let connected_session = session_guard
            .connected
            .as_ref()
            .expect("This should have been handled synchronously!");

        let session = &connected_session.session;

        let handle = connected_session
            .get_or_resolve_profile_handle(batch_exec_profile.as_ref())
            .await?;

        Arc::make_mut(&mut state)
            .batch
            .set_execution_profile_handle(handle);

        let query_res = session.batch(&state.batch, &state.bound_values).await;
        match query_res {
            Ok(result) => Ok(CassResultValue::QueryResult(Arc::new(CassResult {
                tracing_id: None,
                paging_state_response: PagingStateResponse::NoMorePages,
                kind: CassResultKind::NonRows,
                coordinator: Some(result.request_coordinator().clone()),
            }))),
            Err(err) => Ok(CassResultValue::QueryError(Arc::new(err.into()))),
        }
    };

    CassFuture::make_raw(
        runtime,
        future,
        #[cfg(cpp_integration_testing)]
        None,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_session_execute(
    session_raw: CassBorrowedSharedPtr<CassSession, CMut>,
    statement_raw: CassBorrowedSharedPtr<CassStatement, CConst>,
) -> CassOwnedSharedPtr<CassFuture, CMut> {
    let Some(session_opt) = ArcFFI::cloned_from_ptr(session_raw) else {
        tracing::error!("Provided null session pointer to cass_session_execute!");
        return ArcFFI::null();
    };

    let Some(statement_opt) = BoxFFI::as_ref(statement_raw) else {
        tracing::error!("Provided null statement pointer to cass_session_execute!");
        return ArcFFI::null();
    };

    let Ok(session_guard) = session_opt.try_read_owned() else {
        return CassFuture::make_ready_raw(Err((
            CassError::CASS_ERROR_LIB_NO_HOSTS_AVAILABLE,
            "Session is not connected".msg(),
        )));
    };

    let Some(connected_session) = session_guard.connected.as_ref() else {
        return CassFuture::make_ready_raw(Err((
            CassError::CASS_ERROR_LIB_NO_HOSTS_AVAILABLE,
            "Session is not connected".msg(),
        )));
    };

    let runtime = Arc::clone(&connected_session.runtime);

    let paging_state = statement_opt.paging_state.clone();
    let paging_enabled = statement_opt.paging_enabled;
    let mut statement = statement_opt.statement.clone();

    #[cfg(cpp_integration_testing)]
    let recording_listener = statement_opt.record_hosts.then(|| {
        let recording_listener =
            Arc::new(crate::testing::integration::RecordingHistoryListener::new());
        match statement {
            BoundStatement::Simple(ref mut unprepared) => {
                unprepared
                    .query
                    .set_history_listener(Arc::clone(&recording_listener) as _);
            }
            BoundStatement::Prepared(ref mut prepared) => {
                // It's extremely interesting to me that this `as _` cast is required
                // for the type checker to accept this code.
                Arc::make_mut(&mut prepared.statement)
                    .statement
                    .set_history_listener(Arc::clone(&recording_listener) as _);
            }
        };
        recording_listener
    });

    let statement_exec_profile = statement_opt.exec_profile.clone();

    let future = async move {
        let connected_session = session_guard
            .connected
            .as_ref()
            .expect("This should have been handled synchronously!");
        let session = &connected_session.session;

        let handle = connected_session
            .get_or_resolve_profile_handle(statement_exec_profile.as_ref())
            .await?;

        match &mut statement {
            BoundStatement::Simple(query) => query.query.set_execution_profile_handle(handle),
            BoundStatement::Prepared(prepared) => Arc::make_mut(&mut prepared.statement)
                .statement
                .set_execution_profile_handle(handle),
        }

        // Creating a type alias here to fix clippy lints.
        // I want this type to be explicit, so future developers can understand
        // what's going on here (and why we include some weird Option of data types).
        type QueryRes = Result<
            (
                QueryResult,
                PagingStateResponse,
                // We unfortunately have to retrieve the metadata here.
                // Since `query.query` is consumed, we cannot match the statement
                // after execution, to retrieve the cached metadata in case
                // of prepared statements.
                Option<Arc<CassResultMetadata>>,
            ),
            ExecutionError,
        >;
        let query_res: QueryRes = match statement {
            BoundStatement::Simple(query) => {
                // We don't store result metadata for Queries - return None.
                let maybe_result_metadata = None;

                let bound_values = SimpleQueryRowSerializer {
                    bound_values: query.bound_values,
                    name_to_bound_index: query.name_to_bound_index,
                };

                if paging_enabled {
                    session
                        .query_single_page(query.query, bound_values, paging_state)
                        .await
                        .map(|(qr, psr)| (qr, psr, maybe_result_metadata))
                } else {
                    session
                        .query_unpaged(query.query, bound_values)
                        .await
                        .map(|result| {
                            (
                                result,
                                PagingStateResponse::NoMorePages,
                                maybe_result_metadata,
                            )
                        })
                }
            }
            BoundStatement::Prepared(prepared) => {
                // Clone result metadata, so we don't need to construct it from scratch in
                // `CassResultMetadata::from_column_specs` - it requires a lot of allocations for complex types.
                let maybe_result_metadata = Some(Arc::clone(&prepared.statement.result_metadata));

                if paging_enabled {
                    session
                        .execute_single_page(
                            &prepared.statement.statement,
                            prepared.bound_values,
                            paging_state,
                        )
                        .await
                        .map(|(qr, psr)| (qr, psr, maybe_result_metadata))
                } else {
                    session
                        .execute_unpaged(&prepared.statement.statement, prepared.bound_values)
                        .await
                        .map(|result| {
                            (
                                result,
                                PagingStateResponse::NoMorePages,
                                maybe_result_metadata,
                            )
                        })
                }
            }
        };

        match query_res {
            Ok((result, paging_state_response, maybe_result_metadata)) => {
                match CassResult::from_result_payload(
                    result,
                    paging_state_response,
                    maybe_result_metadata,
                ) {
                    Ok(result) => Ok(CassResultValue::QueryResult(Arc::new(result))),
                    Err(e) => Ok(CassResultValue::QueryError(e)),
                }
            }
            Err(err) => Ok(CassResultValue::QueryError(Arc::new(err.into()))),
        }
    };

    CassFuture::make_raw(
        runtime,
        future,
        #[cfg(cpp_integration_testing)]
        recording_listener,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_session_prepare_from_existing(
    cass_session: CassBorrowedSharedPtr<CassSession, CMut>,
    statement: CassBorrowedSharedPtr<CassStatement, CMut>,
) -> CassOwnedSharedPtr<CassFuture, CMut> {
    let Some(session) = ArcFFI::cloned_from_ptr(cass_session) else {
        tracing::error!("Provided null session pointer to cass_session_prepare_from_existing!");
        return ArcFFI::null();
    };
    let Some(cass_statement) = BoxFFI::as_ref(statement) else {
        tracing::error!("Provided null statement pointer to cass_session_prepare_from_existing!");
        return ArcFFI::null();
    };

    let Ok(session_guard) = session.try_read_owned() else {
        return CassFuture::make_ready_raw(Err((
            CassError::CASS_ERROR_LIB_NO_HOSTS_AVAILABLE,
            "Session is not connected".msg(),
        )));
    };

    let Some(connected_session) = session_guard.connected.as_ref() else {
        return CassFuture::make_ready_raw(Err((
            CassError::CASS_ERROR_LIB_NO_HOSTS_AVAILABLE,
            "Session is not connected".msg(),
        )));
    };

    let runtime = Arc::clone(&connected_session.runtime);

    let statement = cass_statement.statement.clone();

    CassFuture::make_raw(
        runtime,
        async move {
            let query = match &statement {
                BoundStatement::Simple(q) => q,
                BoundStatement::Prepared(ps) => {
                    return Ok(CassResultValue::Prepared(Arc::clone(&ps.statement)));
                }
            };

            let connected_session = session_guard
                .connected
                .as_ref()
                .expect("This should have been handled synchronously!");

            let prepared = connected_session
                .session
                .prepare(query.query.clone())
                .await
                .map_err(|err| (err.to_cass_error(), err.msg()))?;

            Ok(CassResultValue::Prepared(Arc::new(
                CassPrepared::new_from_prepared_statement(prepared),
            )))
        },
        #[cfg(cpp_integration_testing)]
        None,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_session_prepare(
    session: CassBorrowedSharedPtr<CassSession, CMut>,
    query: *const c_char,
) -> CassOwnedSharedPtr<CassFuture, CMut> {
    unsafe { cass_session_prepare_n(session, query, strlen(query)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_session_prepare_n(
    cass_session_raw: CassBorrowedSharedPtr<CassSession, CMut>,
    query: *const c_char,
    query_length: size_t,
) -> CassOwnedSharedPtr<CassFuture, CMut> {
    let Some(cass_session) = ArcFFI::cloned_from_ptr(cass_session_raw) else {
        tracing::error!("Provided null session pointer to cass_session_prepare_n!");
        return ArcFFI::null();
    };

    let query_str = unsafe { ptr_to_cstr_n(query, query_length) }
        // Apparently nullptr denotes an empty statement string.
        // It seems to be intended (for some weird reason, why not save a round-trip???)
        // to receive a server error in such case (CASS_ERROR_SERVER_SYNTAX_ERROR).
        // There is a test for this: `NullStringApiArgsTest.Integration_Cassandra_PrepareNullQuery`.
        .unwrap_or_default();

    let Ok(session_guard) = cass_session.try_read_owned() else {
        return CassFuture::make_ready_raw(Err((
            CassError::CASS_ERROR_LIB_NO_HOSTS_AVAILABLE,
            "Session is not connected".msg(),
        )));
    };

    let Some(connected_session) = session_guard.connected.as_ref() else {
        return CassFuture::make_ready_raw(Err((
            CassError::CASS_ERROR_LIB_NO_HOSTS_AVAILABLE,
            "Session is not connected".msg(),
        )));
    };

    let runtime = Arc::clone(&connected_session.runtime);

    let query = Statement::new(query_str.to_string());

    let fut = async move {
        let connected_session = session_guard
            .connected
            .as_ref()
            .expect("This should have been handled synchronously!");

        let prepared = connected_session
            .session
            .prepare(query)
            .await
            .map_err(|err| (err.to_cass_error(), err.msg()))?;

        Ok(CassResultValue::Prepared(Arc::new(
            CassPrepared::new_from_prepared_statement(prepared),
        )))
    };

    CassFuture::make_raw(
        runtime,
        fut,
        #[cfg(cpp_integration_testing)]
        None,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_session_free(session_raw: CassOwnedSharedPtr<CassSession, CMut>) {
    let Some(session_opt) = ArcFFI::from_ptr(session_raw) else {
        // `free()` does nothing on null pointers, so by analogy let's do nothing here.
        return;
    };

    let close_fut = CassConnectedSession::close_fut(session_opt);
    close_fut.waited_result();

    // We don't have to drop the session's Arc explicitly, because it has been moved
    // into the CassFuture, which is dropped here with the end of the scope.
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_session_close(
    session: CassBorrowedSharedPtr<CassSession, CMut>,
) -> CassOwnedSharedPtr<CassFuture, CMut> {
    let Some(session_opt) = ArcFFI::cloned_from_ptr(session) else {
        tracing::error!("Provided null session pointer to cass_session_close!");
        return ArcFFI::null();
    };

    CassConnectedSession::close_fut(session_opt).into_raw()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_session_get_client_id(
    session: CassBorrowedSharedPtr<CassSession, CMut>,
) -> CassUuid {
    let Some(cass_session) = ArcFFI::as_ref(session) else {
        tracing::error!("Provided null session pointer to cass_session_get_client_id!");
        return uuid::Uuid::nil().into();
    };

    let Ok(session_guard) = cass_session.try_read() else {
        tracing::error!(
            "Called cass_session_get_client_id on a connecting/disconnecting session!\
            Wait for it to finish first."
        );
        return uuid::Uuid::nil().into();
    };

    let client_id: uuid::Uuid = session_guard.client_id;
    client_id.into()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_session_get_schema_meta(
    session: CassBorrowedSharedPtr<CassSession, CConst>,
) -> CassOwnedExclusivePtr<CassSchemaMeta, CConst> {
    let Some(cass_session) = ArcFFI::as_ref(session) else {
        tracing::error!("Provided null session pointer to cass_session_get_schema_meta!");
        return CassPtr::null();
    };

    let Ok(session_guard) = cass_session.try_read() else {
        tracing::error!(
            "Called cass_session_get_schema_meta on a connecting/disconnecting session!\
            Wait for it to finish first."
        );
        return CassPtr::null();
    };

    let Some(cass_connected_session) = session_guard.connected.as_ref() else {
        tracing::error!("Called cass_session_get_schema_meta on a disconnected session!");
        return CassPtr::null();
    };

    let mut keyspaces: HashMap<String, CassKeyspaceMeta> = HashMap::new();

    for (keyspace_name, keyspace) in cass_connected_session
        .session
        .get_cluster_state()
        .keyspaces_iter()
    {
        let mut user_defined_type_data_type = HashMap::new();
        let mut tables = HashMap::new();
        let mut views = HashMap::new();

        for (udt_name, udt) in keyspace.user_defined_types.iter() {
            user_defined_type_data_type.insert(
                udt_name.clone(),
                Arc::new(get_column_type(&ColumnType::UserDefinedType {
                    definition: Arc::clone(udt),
                    frozen: false,
                })),
            );
        }

        for (table_name, table_metadata) in &keyspace.tables {
            let cass_table_meta_arced = Arc::new_cyclic(|weak_cass_table_meta| {
                let mut cass_table_meta = create_table_metadata(table_name, table_metadata);

                let mut table_views = HashMap::new();
                for (view_name, view_metadata) in &keyspace.views {
                    let cass_view_table_meta =
                        create_table_metadata(view_name, &view_metadata.view_metadata);
                    let cass_view_meta = CassMaterializedViewMeta {
                        name: view_name.clone(),
                        view_metadata: cass_view_table_meta,
                        base_table: weak_cass_table_meta.clone(),
                    };
                    let cass_view_meta_arced = Arc::new(cass_view_meta);
                    table_views.insert(view_name.clone(), cass_view_meta_arced.clone());

                    views.insert(view_name.clone(), cass_view_meta_arced);
                }

                cass_table_meta.views = table_views;

                cass_table_meta
            });

            tables.insert(table_name.clone(), cass_table_meta_arced);
        }

        keyspaces.insert(
            keyspace_name.to_owned(),
            CassKeyspaceMeta {
                name: keyspace_name.to_owned(),
                user_defined_type_data_type,
                tables,
                views,
            },
        );
    }

    BoxFFI::into_ptr(Box::new(CassSchemaMeta { keyspaces }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_session_get_metrics(
    session_raw: CassBorrowedSharedPtr<CassSession, CConst>,
    metrics: *mut CassMetrics,
) {
    let Some(maybe_session_lock) = ArcFFI::as_ref(session_raw) else {
        tracing::error!("Provided null session pointer to cass_session_get_metrics!");
        return;
    };
    if metrics.is_null() {
        tracing::error!("Provided null metrics pointer to cass_session_get_metrics!");
        return;
    }

    let Ok(session_guard) = maybe_session_lock.try_read() else {
        tracing::error!(
            "Called cass_session_get_metrics on a connecting/disconnecting session!\
            Wait for it to finish first."
        );
        return;
    };
    let Some(session) = session_guard.connected.as_ref() else {
        tracing::warn!("Attempted to get metrics before connecting session object");
        return;
    };

    let rust_metrics = session.session.get_metrics();
    // TODO (rust-driver): Add Snapshot::default() or Snapshot::empty() with 0-initialized snapshot.
    let (
        min,
        max,
        mean,
        stddev,
        median,
        percentile_75,
        percentile_95,
        percentile_98,
        percentile_99,
        percentile_99_9,
    ) = match rust_metrics.get_snapshot() {
        Ok(snapshot) => (
            snapshot.min,
            snapshot.max,
            snapshot.mean,
            snapshot.stddev,
            snapshot.median,
            snapshot.percentile_75,
            snapshot.percentile_95,
            snapshot.percentile_98,
            snapshot.percentile_99,
            snapshot.percentile_99_9,
        ),
        // Histogram is empty, but we don't want to return because there
        // are other metrics that don't depend on histogram.
        Err(MetricsError::Empty) => (0, 0, 0, 0, 0, 0, 0, 0, 0, 0),
        Err(e) => {
            tracing::error!("Failed to get metrics snapshot: {}", e);
            return;
        }
    };

    const MILLIS_TO_MICROS: u64 = 1000;
    // SAFETY: We assume that user provided valid CassMetrics pointer.
    unsafe {
        (*metrics).requests.min = min * MILLIS_TO_MICROS;
        (*metrics).requests.max = max * MILLIS_TO_MICROS;
        (*metrics).requests.mean = mean * MILLIS_TO_MICROS;
        (*metrics).requests.stddev = stddev * MILLIS_TO_MICROS;
        (*metrics).requests.median = median * MILLIS_TO_MICROS;
        (*metrics).requests.percentile_75th = percentile_75 * MILLIS_TO_MICROS;
        (*metrics).requests.percentile_95th = percentile_95 * MILLIS_TO_MICROS;
        (*metrics).requests.percentile_98th = percentile_98 * MILLIS_TO_MICROS;
        (*metrics).requests.percentile_99th = percentile_99 * MILLIS_TO_MICROS;
        (*metrics).requests.percentile_999th = percentile_99_9 * MILLIS_TO_MICROS;
        (*metrics).requests.mean_rate = rust_metrics.get_mean_rate();
        (*metrics).requests.one_minute_rate = rust_metrics.get_one_minute_rate();
        (*metrics).requests.five_minute_rate = rust_metrics.get_five_minute_rate();
        (*metrics).requests.fifteen_minute_rate = rust_metrics.get_fifteen_minute_rate();

        (*metrics).stats.total_connections = rust_metrics.get_total_connections();
        (*metrics).stats.available_connections = 0; // Deprecated
        (*metrics).stats.exceeded_pending_requests_water_mark = 0; // Deprecated
        (*metrics).stats.exceeded_write_bytes_water_mark = 0; // Deprecated

        (*metrics).errors.connection_timeouts = rust_metrics.get_connection_timeouts();
        (*metrics).errors.pending_request_timeouts = 0; // Deprecated
        (*metrics).errors.request_timeouts = rust_metrics.get_request_timeouts();
    }
}

#[cfg(test)]
mod tests {
    use scylla_proxy::{Condition, RequestOpcode, RequestReaction, RequestRule, RunningProxy};
    use tracing::instrument::WithSubscriber;

    use super::*;
    use crate::{
        argconv::make_c_str,
        cluster::{
            cass_cluster_free, cass_cluster_new, cass_cluster_set_contact_points_n,
            cass_cluster_set_execution_profile,
        },
        exec_profile::{
            ExecProfileName, cass_batch_set_execution_profile, cass_batch_set_execution_profile_n,
            cass_execution_profile_free, cass_execution_profile_new,
            cass_statement_set_execution_profile, cass_statement_set_execution_profile_n,
        },
        future::cass_future_error_code,
        statements::batch::{
            CassBatchType, cass_batch_add_statement, cass_batch_free, cass_batch_new,
        },
        statements::statement::{cass_statement_free, cass_statement_new},
        testing::utils::{
            assert_cass_error_eq, cass_future_wait_check_and_free, generic_drop_queries_rules,
            handshake_rules, setup_tracing, test_with_one_proxy,
        },
    };
    use std::{collections::HashSet, iter, net::SocketAddr};

    #[tokio::test]
    #[ntest::timeout(5000)]
    async fn session_clones_and_freezes_exec_profiles_mapping() {
        setup_tracing();
        test_with_one_proxy(
            session_clones_and_freezes_exec_profiles_mapping_do,
            handshake_rules()
                .into_iter()
                .chain(generic_drop_queries_rules()),
        )
        .with_current_subscriber()
        .await;
    }

    fn session_clones_and_freezes_exec_profiles_mapping_do(
        node_addr: SocketAddr,
        proxy: RunningProxy,
    ) -> RunningProxy {
        unsafe {
            let mut cluster_raw = cass_cluster_new();
            let ip = node_addr.ip().to_string();
            let (c_ip, c_ip_len) = str_to_c_str_n(ip.as_str());

            assert_cass_error_eq!(
                cass_cluster_set_contact_points_n(cluster_raw.borrow_mut(), c_ip, c_ip_len),
                CassError::CASS_OK
            );
            let session_raw = cass_session_new();
            let mut profile_raw = cass_execution_profile_new();
            {
                cass_future_wait_check_and_free(cass_session_connect(
                    session_raw.borrow(),
                    cluster_raw.borrow().into_c_const(),
                ));
                // Initially, the profile map is empty.

                assert!(
                    ArcFFI::as_ref(session_raw.borrow())
                        .unwrap()
                        .blocking_read()
                        .connected
                        .as_ref()
                        .unwrap()
                        .exec_profile_map
                        .is_empty()
                );

                cass_cluster_set_execution_profile(
                    cluster_raw.borrow_mut(),
                    make_c_str!("prof"),
                    profile_raw.borrow_mut(),
                );
                // Mutations in cluster do not affect the session that was connected before.
                assert!(
                    ArcFFI::as_ref(session_raw.borrow())
                        .unwrap()
                        .blocking_read()
                        .connected
                        .as_ref()
                        .unwrap()
                        .exec_profile_map
                        .is_empty()
                );

                cass_future_wait_check_and_free(cass_session_close(session_raw.borrow()));

                // Mutations in cluster are now propagated to the session.
                cass_future_wait_check_and_free(cass_session_connect(
                    session_raw.borrow(),
                    cluster_raw.borrow().into_c_const(),
                ));
                let profile_map_keys = ArcFFI::as_ref(session_raw.borrow())
                    .unwrap()
                    .blocking_read()
                    .connected
                    .as_ref()
                    .unwrap()
                    .exec_profile_map
                    .keys()
                    .cloned()
                    .collect::<HashSet<_>>();
                assert_eq!(
                    profile_map_keys,
                    std::iter::once(ExecProfileName::try_from("prof".to_owned()).unwrap())
                        .collect::<HashSet<_>>()
                );
                cass_future_wait_check_and_free(cass_session_close(session_raw.borrow()));
            }
            cass_execution_profile_free(profile_raw);
            cass_session_free(session_raw);
            cass_cluster_free(cluster_raw);
        }
        proxy
    }

    #[tokio::test]
    #[ntest::timeout(5000)]
    async fn session_resolves_exec_profile_on_first_query() {
        setup_tracing();
        test_with_one_proxy(
            session_resolves_exec_profile_on_first_query_do,
            handshake_rules().into_iter().chain(
                iter::once(RequestRule(
                    Condition::RequestOpcode(RequestOpcode::Query)
                        .or(Condition::RequestOpcode(RequestOpcode::Batch))
                        .and(Condition::BodyContainsCaseInsensitive(Box::new(
                            *b"INSERT INTO system.",
                        ))),
                    // We simulate the write failure error that a ScyllaDB node would respond with anyway.
                    RequestReaction::forge().write_failure(),
                ))
                .chain(generic_drop_queries_rules()),
            ),
        )
        .with_current_subscriber()
        .await;
    }

    fn session_resolves_exec_profile_on_first_query_do(
        node_addr: SocketAddr,
        proxy: RunningProxy,
    ) -> RunningProxy {
        unsafe {
            let mut cluster_raw = cass_cluster_new();
            let ip = node_addr.ip().to_string();
            let (c_ip, c_ip_len) = str_to_c_str_n(ip.as_str());

            assert_cass_error_eq!(
                cass_cluster_set_contact_points_n(cluster_raw.borrow_mut(), c_ip, c_ip_len),
                CassError::CASS_OK
            );

            let session_raw = cass_session_new();

            let mut profile_raw = cass_execution_profile_new();
            // A name of a profile that will have been registered in the Cluster.
            let valid_name = "profile";
            let valid_name_c_str = make_c_str!("profile");
            // A name of a profile that won't have been registered in the Cluster.
            let nonexisting_name = "profile1";
            let (nonexisting_name_c_str, nonexisting_name_len) = str_to_c_str_n(nonexisting_name);

            // Inserting into virtual system tables is prohibited and results in WriteFailure error.
            let invalid_query = make_c_str!(
                "INSERT INTO system.runtime_info (group, item, value) VALUES ('bindings_test', 'bindings_test', 'bindings_test')"
            );
            let mut statement_raw = cass_statement_new(invalid_query, 0);
            let mut batch_raw = cass_batch_new(CassBatchType::CASS_BATCH_TYPE_LOGGED);
            assert_cass_error_eq!(
                cass_batch_add_statement(batch_raw.borrow_mut(), statement_raw.borrow()),
                CassError::CASS_OK
            );

            assert_cass_error_eq!(
                cass_cluster_set_execution_profile(
                    cluster_raw.borrow_mut(),
                    valid_name_c_str,
                    profile_raw.borrow_mut(),
                ),
                CassError::CASS_OK
            );

            cass_future_wait_check_and_free(cass_session_connect(
                session_raw.borrow(),
                cluster_raw.borrow().into_c_const(),
            ));
            {
                /* Test valid configurations */
                {
                    let statement = BoxFFI::as_ref(statement_raw.borrow()).unwrap();
                    let batch = BoxFFI::as_ref(batch_raw.borrow()).unwrap();
                    assert!(statement.exec_profile.is_none());
                    assert!(batch.exec_profile.is_none());

                    // Set exec profile - it is not yet resolved.
                    assert_cass_error_eq!(
                        cass_statement_set_execution_profile(
                            statement_raw.borrow_mut(),
                            valid_name_c_str,
                        ),
                        CassError::CASS_OK
                    );
                    assert_cass_error_eq!(
                        cass_batch_set_execution_profile(batch_raw.borrow_mut(), valid_name_c_str,),
                        CassError::CASS_OK
                    );

                    let statement = BoxFFI::as_ref(statement_raw.borrow()).unwrap();
                    let batch = BoxFFI::as_ref(batch_raw.borrow()).unwrap();
                    assert_eq!(
                        statement
                            .exec_profile
                            .as_ref()
                            .unwrap()
                            .inner()
                            .read()
                            .unwrap()
                            .as_name()
                            .unwrap(),
                        &valid_name.to_owned().try_into().unwrap()
                    );
                    assert_eq!(
                        batch
                            .exec_profile
                            .as_ref()
                            .unwrap()
                            .inner()
                            .read()
                            .unwrap()
                            .as_name()
                            .unwrap(),
                        &valid_name.to_owned().try_into().unwrap()
                    );

                    // Make a query - this should resolve the profile.
                    assert_cass_error_eq!(
                        cass_future_error_code(
                            cass_session_execute(
                                session_raw.borrow(),
                                statement_raw.borrow().into_c_const()
                            )
                            .borrow()
                        ),
                        CassError::CASS_ERROR_SERVER_WRITE_FAILURE
                    );
                    assert!(
                        statement
                            .exec_profile
                            .as_ref()
                            .unwrap()
                            .inner()
                            .read()
                            .unwrap()
                            .as_handle()
                            .is_some()
                    );
                    assert_cass_error_eq!(
                        cass_future_error_code(
                            cass_session_execute_batch(
                                session_raw.borrow(),
                                batch_raw.borrow().into_c_const(),
                            )
                            .borrow()
                        ),
                        CassError::CASS_ERROR_SERVER_WRITE_FAILURE
                    );
                    assert!(
                        batch
                            .exec_profile
                            .as_ref()
                            .unwrap()
                            .inner()
                            .read()
                            .unwrap()
                            .as_handle()
                            .is_some()
                    );

                    // NULL name sets exec profile to None
                    assert_cass_error_eq!(
                        cass_statement_set_execution_profile(
                            statement_raw.borrow_mut(),
                            std::ptr::null::<i8>()
                        ),
                        CassError::CASS_OK
                    );
                    assert_cass_error_eq!(
                        cass_batch_set_execution_profile(
                            batch_raw.borrow_mut(),
                            std::ptr::null::<i8>()
                        ),
                        CassError::CASS_OK
                    );

                    let statement = BoxFFI::as_ref(statement_raw.borrow()).unwrap();
                    let batch = BoxFFI::as_ref(batch_raw.borrow()).unwrap();
                    assert!(statement.exec_profile.is_none());
                    assert!(batch.exec_profile.is_none());

                    // valid name again, but of nonexisting profile!
                    assert_cass_error_eq!(
                        cass_statement_set_execution_profile_n(
                            statement_raw.borrow_mut(),
                            nonexisting_name_c_str,
                            nonexisting_name_len,
                        ),
                        CassError::CASS_OK
                    );
                    assert_cass_error_eq!(
                        cass_batch_set_execution_profile_n(
                            batch_raw.borrow_mut(),
                            nonexisting_name_c_str,
                            nonexisting_name_len,
                        ),
                        CassError::CASS_OK
                    );

                    let statement = BoxFFI::as_ref(statement_raw.borrow()).unwrap();
                    let batch = BoxFFI::as_ref(batch_raw.borrow()).unwrap();
                    assert_eq!(
                        statement
                            .exec_profile
                            .as_ref()
                            .unwrap()
                            .inner()
                            .read()
                            .unwrap()
                            .as_name()
                            .unwrap(),
                        &nonexisting_name.to_owned().try_into().unwrap()
                    );
                    assert_eq!(
                        batch
                            .exec_profile
                            .as_ref()
                            .unwrap()
                            .inner()
                            .read()
                            .unwrap()
                            .as_name()
                            .unwrap(),
                        &nonexisting_name.to_owned().try_into().unwrap()
                    );

                    // So when we now issue a query, it should end with error and leave exec_profile_handle uninitialised.
                    assert_cass_error_eq!(
                        cass_future_error_code(
                            cass_session_execute(
                                session_raw.borrow(),
                                statement_raw.borrow().into_c_const()
                            )
                            .borrow()
                        ),
                        CassError::CASS_ERROR_LIB_EXECUTION_PROFILE_INVALID
                    );
                    assert_eq!(
                        statement
                            .exec_profile
                            .as_ref()
                            .unwrap()
                            .inner()
                            .read()
                            .unwrap()
                            .as_name()
                            .unwrap(),
                        &nonexisting_name.to_owned().try_into().unwrap()
                    );
                    assert_cass_error_eq!(
                        cass_future_error_code(
                            cass_session_execute_batch(
                                session_raw.borrow(),
                                batch_raw.borrow().into_c_const()
                            )
                            .borrow()
                        ),
                        CassError::CASS_ERROR_LIB_EXECUTION_PROFILE_INVALID
                    );
                    assert_eq!(
                        batch
                            .exec_profile
                            .as_ref()
                            .unwrap()
                            .inner()
                            .read()
                            .unwrap()
                            .as_name()
                            .unwrap(),
                        &nonexisting_name.to_owned().try_into().unwrap()
                    );
                }
            }

            cass_future_wait_check_and_free(cass_session_close(session_raw.borrow()));
            cass_execution_profile_free(profile_raw);
            cass_statement_free(statement_raw);
            cass_batch_free(batch_raw);
            cass_session_free(session_raw);
            cass_cluster_free(cluster_raw);
        }
        proxy
    }
}
