use arc_swap::ArcSwap;
use scylla::response::query_result::ColumnSpecs;
use scylla::{statement::prepared::ColumnSpecsGuard, value::MaybeUnset::Unset};
use std::fmt::Debug;
use std::{os::raw::c_char, sync::Arc};

use crate::{
    argconv::*,
    cass_error::CassError,
    cql_types::data_type::{CassDataType, get_column_type},
    query_result::CassResultMetadata,
    statements::statement::{BoundPreparedStatement, CassStatement},
    types::size_t,
};
use scylla::statement::prepared::PreparedStatement;

struct CassResultMetadataCacheInner {
    guard: ColumnSpecsGuard,
    cass_result_metadata: Arc<CassResultMetadata>,
}

//TODO: Move to derive(Debug) when https://github.com/scylladb/scylla-rust-driver/issues/1492 is done
impl Debug for CassResultMetadataCacheInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CassResultMetadataCacheInner")
            .field("guard", &self.guard.get())
            .field("cass_result_metadata", &self.cass_result_metadata)
            .finish()
    }
}

/// Driver needs to create its own CassResultMetadata, it can't just use
/// result metadata from the Rust Driver.
/// Rust Driver result metadata is mutable, so we need to be able to recreate CassResultMetadata
/// when necessary.
/// This struct allows that.
#[derive(Debug)]
struct CassResultMetadataCache {
    inner: ArcSwap<CassResultMetadataCacheInner>,
}

impl CassResultMetadataCache {
    fn new(guard: ColumnSpecsGuard) -> Self {
        let new_cass_result_metadata = Arc::new(CassResultMetadata::from_column_specs(guard.get()));
        Self {
            inner: ArcSwap::new(Arc::new(CassResultMetadataCacheInner {
                guard,
                cass_result_metadata: new_cass_result_metadata,
            })),
        }
    }

    /// Takes `ColumnSpecsGuard`, which is fetched from `PreparedStatement`,
    /// and updates the cache to its metadata (if different than current metadata in cache).
    /// Please note, that even if you:
    /// - Perform a request
    /// - After it finishes, call this method with metadata fetched from the statement
    /// - Immediately after call `get_metadata_unless_stale`
    ///
    /// You DON'T have a guarantee that `get_metadata_unless_stale` returns `Some`.
    /// This is because `PreparedStatement::get_current_result_set_col_specs` fetches
    /// current metadata at a point in time, which may have been concurrently changed by execution
    /// of another request.
    pub(crate) fn update_if_necessary(&self, new_guard: ColumnSpecsGuard) {
        let inner_guard = self.inner.load();
        let old_column_specs_slice = inner_guard.guard.get().as_slice();
        let new_column_specs_slice = new_guard.get().as_slice();
        // Why only performing pointer comparison?
        // The only situation I can think of when pointers are not equal,
        // but the column specs are, is if there was new metadata received
        // from the server, and few requests updated it concurrently.
        // I don't think it matters - during schema changes we may do a few
        // more allocations, but it is only a temporary situation and should
        // quickly stabilize.
        // Also the current solution gives us a nice property that we always
        // store ColumnSpecsGuard, and CassResultMetadata that was created
        // from exactly this guard. This allows us to only perform pointer
        // comparison in `get_metadata_unless_stale`, which is important
        // because it is on a hot path.
        if !std::ptr::eq(old_column_specs_slice, new_column_specs_slice) {
            let new_cass_result_metadata =
                Arc::new(CassResultMetadata::from_column_specs(new_guard.get()));
            // No need for rcu, because it doesn't save us from expensive creation of
            // CassResultMetadata, and doesn't give us any benefits. For us,
            // all the metadatas are just opaque, we can't tell if one is "newer".
            self.inner.store(Arc::new(CassResultMetadataCacheInner {
                guard: new_guard,
                cass_result_metadata: new_cass_result_metadata,
            }));
        }
    }

    pub(crate) fn get_metadata_unless_stale(
        &self,
        result_column_specs: ColumnSpecs<'_, '_>,
    ) -> Option<Arc<CassResultMetadata>> {
        let inner_guard = self.inner.load();
        let self_column_specs_slice = inner_guard.guard.get().as_slice();
        let result_column_specs_slice = result_column_specs.as_slice();
        // Pointer comparison is enough, because we always make sure to
        // store the exact same guard that the CassResultMetadata was created from.
        if std::ptr::eq(self_column_specs_slice, result_column_specs_slice) {
            Some(inner_guard.cass_result_metadata.clone())
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct CassPrepared {
    // Data types of columns from PreparedMetadata.
    pub(crate) variable_col_data_types: Vec<Arc<CassDataType>>,

    // Cached result metadata.
    result_metadata_cache: Arc<CassResultMetadataCache>,
    pub(crate) statement: PreparedStatement,
}

impl CassPrepared {
    pub(crate) fn new_from_prepared_statement(mut statement: PreparedStatement) -> Self {
        // For Rust Driver 1.4 and Scylla 2025.3+ thanks to result metadata id extension,
        // correctness issue related to metadata caching is eliminated.
        // Metadata will be cached regardless of the setting below, and will
        // be updated when necessary.
        // For older versions in other drivers we usually disable this option in order to
        // err on the side of correctness rather than performance.
        // Here it is more difficult, because the driver does its own checking, and transforms
        // metadata in an expensive way. Disabling caching would require more changes, so let's
        // keep it enabled (at least for now).
        statement.set_use_cached_result_metadata(true);

        let variable_col_data_types = statement
            .get_variable_col_specs()
            .iter()
            .map(|col_spec| Arc::new(get_column_type(col_spec.typ())))
            .collect();

        let result_metadata_cache = Arc::new(CassResultMetadataCache::new(
            statement.get_current_result_set_col_specs(),
        ));

        Self {
            variable_col_data_types,
            result_metadata_cache,
            statement,
        }
    }

    pub(crate) fn get_variable_data_type_by_name(&self, name: &str) -> Option<&Arc<CassDataType>> {
        let index = self
            .statement
            .get_variable_col_specs()
            .iter()
            .position(|col_spec| col_spec.name() == name)?;

        match self.variable_col_data_types.get(index) {
            Some(dt) => Some(dt),
            // This is a violation of driver's internal invariant.
            // Since `self.variable_col_data_types` is created based on prepared statement's
            // col specs, and we found an index with a corresponding name, we should
            // find a CassDataType at given index.
            None => panic!(
                "Cannot find a data type of parameter with given name: {name}. This is a driver bug!",
            ),
        }
    }

    pub(crate) fn try_update_and_get_result_metadata(
        &self,
        result_column_specs: ColumnSpecs<'_, '_>,
    ) -> Option<Arc<CassResultMetadata>> {
        self.result_metadata_cache
            .update_if_necessary(self.statement.get_current_result_set_col_specs());
        self.result_metadata_cache
            .get_metadata_unless_stale(result_column_specs)
    }
}

impl FFI for CassPrepared {
    type Origin = FromArc;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_prepared_free(
    prepared_raw: CassOwnedSharedPtr<CassPrepared, CConst>,
) {
    ArcFFI::free(prepared_raw);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_prepared_bind(
    prepared_raw: CassBorrowedSharedPtr<CassPrepared, CConst>,
) -> CassOwnedExclusivePtr<CassStatement, CMut> {
    let Some(prepared) = ArcFFI::cloned_from_ptr(prepared_raw) else {
        tracing::error!("Provided null prepared statement pointer to cass_prepared_bind!");
        return BoxFFI::null_mut();
    };

    let bound_values_size = prepared.statement.get_variable_col_specs().len();

    // cloning prepared statement's arc, because creating CassStatement should not invalidate
    // the CassPrepared argument

    let statement = BoundPreparedStatement {
        statement: prepared,
        bound_values: vec![Unset; bound_values_size],
    };

    BoxFFI::into_ptr(Box::new(CassStatement::new_prepared(statement)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_prepared_parameter_name(
    prepared_raw: CassBorrowedSharedPtr<CassPrepared, CConst>,
    index: size_t,
    name: *mut *const c_char,
    name_length: *mut size_t,
) -> CassError {
    let Some(prepared) = ArcFFI::as_ref(prepared_raw) else {
        tracing::error!(
            "Provided null prepared statement pointer to cass_prepared_parameter_name!"
        );
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    let Some(col_spec) = prepared
        .statement
        .get_variable_col_specs()
        .get_by_index(index as usize)
    else {
        return CassError::CASS_ERROR_LIB_INDEX_OUT_OF_BOUNDS;
    };
    unsafe { write_str_to_c(col_spec.name(), name, name_length) };
    CassError::CASS_OK
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_prepared_parameter_data_type(
    prepared_raw: CassBorrowedSharedPtr<CassPrepared, CConst>,
    index: size_t,
) -> CassBorrowedSharedPtr<CassDataType, CConst> {
    let Some(prepared) = ArcFFI::as_ref(prepared_raw) else {
        tracing::error!(
            "Provided null prepared statement pointer to cass_prepared_parameter_data_type!"
        );
        return ArcFFI::null();
    };

    match prepared.variable_col_data_types.get(index as usize) {
        Some(dt) => ArcFFI::as_ptr(dt),
        None => ArcFFI::null(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_prepared_parameter_data_type_by_name(
    prepared_raw: CassBorrowedSharedPtr<CassPrepared, CConst>,
    name: *const c_char,
) -> CassBorrowedSharedPtr<CassDataType, CConst> {
    unsafe { cass_prepared_parameter_data_type_by_name_n(prepared_raw, name, strlen(name)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_prepared_parameter_data_type_by_name_n(
    prepared_raw: CassBorrowedSharedPtr<CassPrepared, CConst>,
    name: *const c_char,
    name_length: size_t,
) -> CassBorrowedSharedPtr<CassDataType, CConst> {
    let Some(prepared) = ArcFFI::as_ref(prepared_raw) else {
        tracing::error!(
            "Provided null prepared statement pointer to cass_prepared_parameter_data_type_by_name!"
        );
        return ArcFFI::null();
    };

    let parameter_name =
        unsafe { ptr_to_cstr_n(name, name_length).expect("Prepared parameter name is not UTF-8") };

    let data_type = prepared.get_variable_data_type_by_name(parameter_name);
    match data_type {
        Some(dt) => ArcFFI::as_ptr(dt),
        None => ArcFFI::null(),
    }
}
