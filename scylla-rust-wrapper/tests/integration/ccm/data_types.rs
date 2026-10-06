//! End-to-end tests of data-type handling against a real ScyllaDB cluster
//! started via CCM. The driver is exercised exclusively through its C API,
//! mirroring how a C/C++ consumer would use it.

use std::ffi::CString;

use scylla_ccm_bridge::cluster::{Cluster, ClusterOptions};
use scylla_ccm_bridge::{CLUSTER_VERSION, run_ccm_test};

use scylladb::api::cluster::{
    cass_cluster_free, cass_cluster_new, cass_cluster_set_contact_points,
};
use scylladb::api::collection::{
    cass_collection_append_string, cass_collection_free, cass_collection_new_from_data_type,
};
use scylladb::api::data_type::{
    cass_data_type_add_sub_value_type, cass_data_type_add_sub_value_type_by_name,
    cass_data_type_free, cass_data_type_new, cass_data_type_new_tuple, cass_data_type_new_udt,
};
use scylladb::api::error::CassError;
use scylladb::api::future::{
    cass_future_error_code, cass_future_free, cass_future_get_prepared, cass_future_get_result,
    cass_future_wait,
};
use scylladb::api::iterator::{
    cass_iterator_fields_from_user_type, cass_iterator_free, cass_iterator_from_collection,
    cass_iterator_from_map, cass_iterator_from_tuple, cass_iterator_get_map_key,
    cass_iterator_get_map_value, cass_iterator_get_user_type_field_value, cass_iterator_get_value,
    cass_iterator_next,
};
use scylladb::api::prepared::{cass_prepared_bind, cass_prepared_free};
use scylladb::api::result::{cass_result_first_row, cass_result_free};
use scylladb::api::row::cass_row_get_column_by_name;
use scylladb::api::session::{
    CassSession, cass_session_close, cass_session_connect, cass_session_execute, cass_session_free,
    cass_session_new, cass_session_prepare,
};
use scylladb::api::statement::{
    cass_statement_bind_collection, cass_statement_bind_tuple, cass_statement_bind_user_type,
    cass_statement_free, cass_statement_new,
};
use scylladb::api::tuple::{cass_tuple_free, cass_tuple_new_from_data_type, cass_tuple_set_string};
use scylladb::api::user_type::{
    cass_user_type_free, cass_user_type_new_from_data_type, cass_user_type_set_string_by_name,
};
use scylladb::api::value::{CassValue, CassValueType, cass_value_get_string};
use scylladb::argconv::{CConst, CMut, CassBorrowedSharedPtr, CassStrNulTerminated};

use crate::utils::{assert_cass_error_eq, setup_tracing};

use super::warn_if_partial_cluster_version;

fn cluster_1_node() -> ClusterOptions {
    ClusterOptions {
        name: "cluster_data_types_1_node".to_string(),
        version: CLUSTER_VERSION.clone(),
        nodes_per_dc: vec![1],
        ..ClusterOptions::default()
    }
}

/// A fully typed container is one built from a data type that names its
/// subtypes. Only such a container has its subtypes compared against the
/// metadata of a prepared statement.
///
/// The CQL `text` type has two names in the C API, `CASS_VALUE_TYPE_TEXT` and
/// `CASS_VALUE_TYPE_VARCHAR`, while the protocol has a single representation of
/// it, which the driver reports as the latter. A container the user typed with
/// the former must bind all the same.
#[tokio::test]
#[ntest::timeout(600000)]
async fn text_subtypes_bind_to_prepared_statements() {
    setup_tracing();
    warn_if_partial_cluster_version();

    run_ccm_test(cluster_1_node, async |cluster: &mut Cluster| {
        let contact_points = cluster
            .nodes()
            .iter()
            .map(|node| node.broadcast_rpc_address().to_string())
            .collect::<Vec<_>>()
            .join(",");

        tokio::task::spawn_blocking(move || unsafe { run_test(&contact_points) })
            .await
            .unwrap()
    })
    .await
}

/// Reads a `text` value and returns it as a `String`.
unsafe fn get_string(value: CassBorrowedSharedPtr<CassValue, CConst>) -> String {
    let mut text: *const std::os::raw::c_char = std::ptr::null();
    let mut text_length: u64 = 0;
    unsafe {
        assert_cass_error_eq(
            CassError::CASS_OK,
            cass_value_get_string(value, &raw mut text, &raw mut text_length),
        );
        std::str::from_utf8(std::slice::from_raw_parts(
            text.cast::<u8>(),
            text_length as usize,
        ))
        .unwrap()
        .to_owned()
    }
}

/// # Safety
///
/// Must be called on a thread where blocking is acceptable (it blocks on the
/// driver futures via `cass_future_wait`).
unsafe fn run_test(contact_points: &str) {
    unsafe {
        let mut cluster_raw = cass_cluster_new();
        let contact_points_c = CString::new(contact_points).unwrap();
        cass_cluster_set_contact_points(
            cluster_raw.borrow_mut(),
            CassStrNulTerminated::from_cstr(&contact_points_c),
        );

        let session_raw = cass_session_new();
        let connect_fut =
            cass_session_connect(session_raw.borrow(), cluster_raw.borrow().into_c_const());
        cass_future_wait(connect_fut.borrow());
        assert_cass_error_eq(
            CassError::CASS_OK,
            cass_future_error_code(connect_fut.borrow()),
        );
        cass_future_free(connect_fut);

        for ddl in [
            c"CREATE KEYSPACE IF NOT EXISTS data_types WITH replication = \
              {'class': 'NetworkTopologyStrategy', 'replication_factor': 1}",
            c"CREATE TYPE IF NOT EXISTS data_types.udt (f text)",
            c"CREATE TABLE IF NOT EXISTS data_types.containers ( \
                 id int PRIMARY KEY, \
                 l list<text>, \
                 s set<text>, \
                 m map<text, text>, \
                 t frozen<tuple<text>>, \
                 u frozen<data_types.udt>)",
        ] {
            execute(session_raw.borrow(), ddl);
        }

        // Prepare the insert: only a prepared statement carries the metadata
        // that the bound values are typechecked against.
        let query = c"INSERT INTO data_types.containers (id, l, s, m, t, u) \
                      VALUES (0, ?, ?, ?, ?, ?)";
        let prepare_fut =
            cass_session_prepare(session_raw.borrow(), CassStrNulTerminated::from_cstr(query));
        cass_future_wait(prepare_fut.borrow());
        assert_cass_error_eq(
            CassError::CASS_OK,
            cass_future_error_code(prepare_fut.borrow()),
        );
        let prepared = cass_future_get_prepared(prepare_fut.borrow());
        cass_future_free(prepare_fut);
        let mut statement = cass_prepared_bind(prepared.borrow().into_c_const());

        let text = c"alpha";
        let text_arg = || CassStrNulTerminated::from_cstr(text);

        // Every bind is recorded instead of asserted right away, so that a
        // failure names every container shape that it affects.
        let mut binds: Vec<(&str, CassError)> = Vec::new();

        // list<text>, built with CASS_VALUE_TYPE_TEXT.
        let list_type = cass_data_type_new(CassValueType::CASS_VALUE_TYPE_LIST);
        assert_cass_error_eq(
            CassError::CASS_OK,
            cass_data_type_add_sub_value_type(
                list_type.borrow(),
                CassValueType::CASS_VALUE_TYPE_TEXT,
            ),
        );
        let mut list = cass_collection_new_from_data_type(list_type.borrow().into_c_const(), 1);
        assert_cass_error_eq(
            CassError::CASS_OK,
            cass_collection_append_string(list.borrow_mut(), text_arg()),
        );
        binds.push((
            "list<text>",
            cass_statement_bind_collection(statement.borrow_mut(), 0, list.borrow().into_c_const()),
        ));

        // set<text>
        let set_type = cass_data_type_new(CassValueType::CASS_VALUE_TYPE_SET);
        assert_cass_error_eq(
            CassError::CASS_OK,
            cass_data_type_add_sub_value_type(
                set_type.borrow(),
                CassValueType::CASS_VALUE_TYPE_TEXT,
            ),
        );
        let mut set = cass_collection_new_from_data_type(set_type.borrow().into_c_const(), 1);
        assert_cass_error_eq(
            CassError::CASS_OK,
            cass_collection_append_string(set.borrow_mut(), text_arg()),
        );
        binds.push((
            "set<text>",
            cass_statement_bind_collection(statement.borrow_mut(), 1, set.borrow().into_c_const()),
        ));

        // map<text, text>
        let map_type = cass_data_type_new(CassValueType::CASS_VALUE_TYPE_MAP);
        for _ in 0..2 {
            assert_cass_error_eq(
                CassError::CASS_OK,
                cass_data_type_add_sub_value_type(
                    map_type.borrow(),
                    CassValueType::CASS_VALUE_TYPE_TEXT,
                ),
            );
        }
        let mut map = cass_collection_new_from_data_type(map_type.borrow().into_c_const(), 1);
        for _ in 0..2 {
            assert_cass_error_eq(
                CassError::CASS_OK,
                cass_collection_append_string(map.borrow_mut(), text_arg()),
            );
        }
        binds.push((
            "map<text, text>",
            cass_statement_bind_collection(statement.borrow_mut(), 2, map.borrow().into_c_const()),
        ));

        // tuple<text>
        let tuple_type = cass_data_type_new_tuple(1);
        assert_cass_error_eq(
            CassError::CASS_OK,
            cass_data_type_add_sub_value_type(
                tuple_type.borrow(),
                CassValueType::CASS_VALUE_TYPE_TEXT,
            ),
        );
        let mut tuple = cass_tuple_new_from_data_type(tuple_type.borrow().into_c_const());
        assert_cass_error_eq(
            CassError::CASS_OK,
            cass_tuple_set_string(tuple.borrow_mut(), 0, text_arg()),
        );
        binds.push((
            "tuple<text>",
            cass_statement_bind_tuple(statement.borrow_mut(), 3, tuple.borrow().into_c_const()),
        ));

        // A UDT with a text field.
        let udt_type = cass_data_type_new_udt(1);
        assert_cass_error_eq(
            CassError::CASS_OK,
            cass_data_type_add_sub_value_type_by_name(
                udt_type.borrow(),
                CassStrNulTerminated::from_cstr(c"f"),
                CassValueType::CASS_VALUE_TYPE_TEXT,
            ),
        );
        let mut udt = cass_user_type_new_from_data_type(udt_type.borrow().into_c_const());
        assert_cass_error_eq(
            CassError::CASS_OK,
            cass_user_type_set_string_by_name(
                udt.borrow_mut(),
                CassStrNulTerminated::from_cstr(c"f"),
                text_arg(),
            ),
        );
        binds.push((
            "udt with a text field",
            cass_statement_bind_user_type(statement.borrow_mut(), 4, udt.borrow().into_c_const()),
        ));

        let failed: Vec<&str> = binds
            .iter()
            .filter(|(_, err)| *err != CassError::CASS_OK)
            .map(|(name, _)| *name)
            .collect();
        assert!(failed.is_empty(), "failed to bind: {failed:?}");

        let insert_fut =
            cass_session_execute(session_raw.borrow(), statement.borrow().into_c_const());
        cass_future_wait(insert_fut.borrow());
        assert_cass_error_eq(
            CassError::CASS_OK,
            cass_future_error_code(insert_fut.borrow()),
        );
        cass_future_free(insert_fut);

        cass_collection_free(list);
        cass_collection_free(set);
        cass_collection_free(map);
        cass_tuple_free(tuple);
        cass_user_type_free(udt);
        cass_data_type_free(list_type);
        cass_data_type_free(set_type);
        cass_data_type_free(map_type);
        cass_data_type_free(tuple_type);
        cass_data_type_free(udt_type);
        cass_statement_free(statement);
        cass_prepared_free(prepared);

        // Read everything back, to prove the values really landed.
        let select = c"SELECT l, s, m, t, u FROM data_types.containers WHERE id = 0";
        let statement = cass_statement_new(CassStrNulTerminated::from_cstr(select), 0);
        let select_fut =
            cass_session_execute(session_raw.borrow(), statement.borrow().into_c_const());
        cass_future_wait(select_fut.borrow());
        assert_cass_error_eq(
            CassError::CASS_OK,
            cass_future_error_code(select_fut.borrow()),
        );
        let result = cass_future_get_result(select_fut.borrow());
        cass_future_free(select_fut);
        cass_statement_free(statement);

        let row = cass_result_first_row(result.borrow());
        let column = |name: &std::ffi::CStr| {
            cass_row_get_column_by_name(row.borrow(), CassStrNulTerminated::from_cstr(name))
        };

        // list<text> and set<text>
        for name in [c"l", c"s"] {
            let mut iter = cass_iterator_from_collection(column(name));
            assert_eq!(cass_iterator_next(iter.borrow_mut()), 1);
            assert_eq!(
                get_string(cass_iterator_get_value(iter.borrow().into_c_const())),
                "alpha"
            );
            assert_eq!(cass_iterator_next(iter.borrow_mut()), 0);
            cass_iterator_free(iter);
        }

        // map<text, text>
        let mut iter = cass_iterator_from_map(column(c"m"));
        assert_eq!(cass_iterator_next(iter.borrow_mut()), 1);
        assert_eq!(
            get_string(cass_iterator_get_map_key(iter.borrow().into_c_const())),
            "alpha"
        );
        assert_eq!(
            get_string(cass_iterator_get_map_value(iter.borrow().into_c_const())),
            "alpha"
        );
        cass_iterator_free(iter);

        // tuple<text>
        let mut iter = cass_iterator_from_tuple(column(c"t"));
        assert_eq!(cass_iterator_next(iter.borrow_mut()), 1);
        assert_eq!(
            get_string(cass_iterator_get_value(iter.borrow().into_c_const())),
            "alpha"
        );
        cass_iterator_free(iter);

        // The UDT's text field.
        let mut iter = cass_iterator_fields_from_user_type(column(c"u"));
        assert_eq!(cass_iterator_next(iter.borrow_mut()), 1);
        assert_eq!(
            get_string(cass_iterator_get_user_type_field_value(
                iter.borrow().into_c_const()
            )),
            "alpha"
        );
        cass_iterator_free(iter);

        cass_result_free(result);

        let close_fut = cass_session_close(session_raw.borrow());
        cass_future_wait(close_fut.borrow());
        cass_future_free(close_fut);
        cass_session_free(session_raw);
        cass_cluster_free(cluster_raw);
    }
}

/// Executes a statement that returns nothing, asserting that it succeeded.
unsafe fn execute(session: CassBorrowedSharedPtr<CassSession, CMut>, query: &std::ffi::CStr) {
    unsafe {
        let statement = cass_statement_new(CassStrNulTerminated::from_cstr(query), 0);
        let fut = cass_session_execute(session, statement.borrow().into_c_const());
        cass_future_wait(fut.borrow());
        assert_cass_error_eq(CassError::CASS_OK, cass_future_error_code(fut.borrow()));
        cass_future_free(fut);
        cass_statement_free(statement);
    }
}
