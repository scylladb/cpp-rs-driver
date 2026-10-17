//! End-to-end tests of vectors nested inside other CQL types, against a real
//! ScyllaDB cluster started via CCM. The driver is exercised exclusively
//! through its C API, mirroring how a C/C++ consumer would use it.
//!
//! The opposite direction - a vector *of* a composite type - is covered by the
//! unit tests of vector serialization, which compare the produced bytes with
//! rust-driver's own.

use std::ffi::CString;

use scylla_ccm_bridge::cluster::{Cluster, ClusterOptions};
use scylla_ccm_bridge::{CLUSTER_VERSION, run_ccm_test};

use scylladb::api::cluster::{
    cass_cluster_free, cass_cluster_new, cass_cluster_set_contact_points,
};
use scylladb::api::collection::{
    cass_collection_append_vector, cass_collection_free, cass_collection_new_from_data_type,
};
use scylladb::api::data_type::{
    CassDataType, cass_data_type_add_sub_type, cass_data_type_add_sub_type_by_name,
    cass_data_type_add_sub_value_type, cass_data_type_free, cass_data_type_new,
    cass_data_type_new_tuple, cass_data_type_new_udt, cass_data_type_new_vector,
};
use scylladb::api::error::CassError;
use scylladb::api::future::{
    cass_future_error_code, cass_future_free, cass_future_get_prepared, cass_future_get_result,
    cass_future_wait,
};
use scylladb::api::iterator::{
    cass_iterator_fields_from_user_type, cass_iterator_free, cass_iterator_from_collection,
    cass_iterator_from_tuple, cass_iterator_from_vector, cass_iterator_get_user_type_field_value,
    cass_iterator_get_value, cass_iterator_next,
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
use scylladb::api::tuple::{
    cass_tuple_free, cass_tuple_new_from_data_type, cass_tuple_set_int32, cass_tuple_set_vector,
};
use scylladb::api::user_type::{
    cass_user_type_free, cass_user_type_new_from_data_type, cass_user_type_set_vector,
    cass_user_type_set_vector_by_name, cass_user_type_set_vector_by_name_n,
};
use scylladb::api::value::{CassValue, CassValueType, cass_value_get_float, cass_value_get_int32};
use scylladb::api::vector::{
    CassVector, cass_vector_free, cass_vector_new_from_data_type, cass_vector_set_float,
};
use scylladb::argconv::{
    CConst, CMut, CassBorrowedSharedPtr, CassOwnedExclusivePtr, CassStrNulTerminated,
};

use crate::utils::{assert_cass_error_eq, setup_tracing};

use super::warn_if_partial_cluster_version;

/// The dimensions of every vector used in this test.
const DIMENSIONS: usize = 3;

fn cluster_1_node() -> ClusterOptions {
    ClusterOptions {
        name: "cluster_vector_nesting_1_node".to_string(),
        version: CLUSTER_VERSION.clone(),
        nodes_per_dc: vec![1],
        ..ClusterOptions::default()
    }
}

/// A vector can be an element of a collection, a field of a tuple and a field
/// of a UDT. Each of those is reached through its own setter, and each of them
/// must bind against the metadata of a prepared statement and round-trip.
#[tokio::test]
#[ntest::timeout(600000)]
async fn vectors_nest_inside_other_types() {
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

/// Builds a `vector<float, DIMENSIONS>` holding `values`.
unsafe fn make_vector(
    vector_type: CassBorrowedSharedPtr<CassDataType, CConst>,
    values: [f32; DIMENSIONS],
) -> CassOwnedExclusivePtr<CassVector, CMut> {
    unsafe {
        let mut vector = cass_vector_new_from_data_type(vector_type);
        for (i, v) in values.iter().enumerate() {
            assert_cass_error_eq(
                CassError::CASS_OK,
                cass_vector_set_float(vector.borrow_mut(), i as u64, *v),
            );
        }
        vector
    }
}

/// Reads a `vector<float, DIMENSIONS>` value back into an array.
unsafe fn get_floats(value: CassBorrowedSharedPtr<CassValue, CConst>) -> [f32; DIMENSIONS] {
    unsafe {
        let mut read = [0.0_f32; DIMENSIONS];
        let mut iter = cass_iterator_from_vector(value);
        for slot in read.iter_mut() {
            assert_eq!(cass_iterator_next(iter.borrow_mut()), 1);
            assert_cass_error_eq(
                CassError::CASS_OK,
                cass_value_get_float(
                    cass_iterator_get_value(iter.borrow().into_c_const()),
                    &raw mut *slot,
                ),
            );
        }
        assert_eq!(cass_iterator_next(iter.borrow_mut()), 0);
        cass_iterator_free(iter);
        read
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
            c"CREATE KEYSPACE IF NOT EXISTS vector_nesting WITH replication = \
              {'class': 'NetworkTopologyStrategy', 'replication_factor': 1}",
            c"CREATE TYPE IF NOT EXISTS vector_nesting.udt ( \
                 a vector<float, 3>, \
                 b vector<float, 3>, \
                 c vector<float, 3>)",
            c"CREATE TABLE IF NOT EXISTS vector_nesting.containers ( \
                 id int PRIMARY KEY, \
                 l list<frozen<vector<float, 3>>>, \
                 t frozen<tuple<int, vector<float, 3>>>, \
                 u frozen<vector_nesting.udt>)",
        ] {
            execute(session_raw.borrow(), ddl);
        }

        // Only a prepared statement carries the metadata that the bound values
        // are typechecked against.
        let query = c"INSERT INTO vector_nesting.containers (id, l, t, u) VALUES (0, ?, ?, ?)";
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

        // The element type must be complete before the vector type is created.
        let float_type = cass_data_type_new(CassValueType::CASS_VALUE_TYPE_FLOAT);
        let vector_type =
            cass_data_type_new_vector(float_type.borrow().into_c_const(), DIMENSIONS as u64);

        // Every bind is recorded instead of asserted right away, so that a
        // failure names every nesting that it affects.
        let mut binds: Vec<(&str, CassError)> = Vec::new();

        // list<frozen<vector<float, 3>>>
        let list_type = cass_data_type_new(CassValueType::CASS_VALUE_TYPE_LIST);
        assert_cass_error_eq(
            CassError::CASS_OK,
            cass_data_type_add_sub_type(list_type.borrow(), vector_type.borrow().into_c_const()),
        );
        let mut list = cass_collection_new_from_data_type(list_type.borrow().into_c_const(), 2);
        let list_items = [[1.0_f32, 2.0, 3.0], [4.0, 5.0, 6.0]];
        for values in list_items {
            let item = make_vector(vector_type.borrow().into_c_const(), values);
            binds.push((
                "vector in a list",
                cass_collection_append_vector(list.borrow_mut(), item.borrow().into_c_const()),
            ));
            cass_vector_free(item);
        }
        binds.push((
            "list of vectors",
            cass_statement_bind_collection(statement.borrow_mut(), 0, list.borrow().into_c_const()),
        ));

        // tuple<int, vector<float, 3>>
        let tuple_type = cass_data_type_new_tuple(2);
        assert_cass_error_eq(
            CassError::CASS_OK,
            cass_data_type_add_sub_value_type(
                tuple_type.borrow(),
                CassValueType::CASS_VALUE_TYPE_INT,
            ),
        );
        assert_cass_error_eq(
            CassError::CASS_OK,
            cass_data_type_add_sub_type(tuple_type.borrow(), vector_type.borrow().into_c_const()),
        );
        let mut tuple = cass_tuple_new_from_data_type(tuple_type.borrow().into_c_const());
        assert_cass_error_eq(
            CassError::CASS_OK,
            cass_tuple_set_int32(tuple.borrow_mut(), 0, 42),
        );
        let tuple_item = [7.0_f32, 8.0, 9.0];
        let item = make_vector(vector_type.borrow().into_c_const(), tuple_item);
        binds.push((
            "vector in a tuple",
            cass_tuple_set_vector(tuple.borrow_mut(), 1, item.borrow().into_c_const()),
        ));
        cass_vector_free(item);
        binds.push((
            "tuple with a vector",
            cass_statement_bind_tuple(statement.borrow_mut(), 1, tuple.borrow().into_c_const()),
        ));

        // A UDT with three vector fields, so that each of the three setters -
        // by index, by name and by length-delimited name - is exercised.
        let udt_field_names = [c"a", c"b", c"c"];
        let udt_type = cass_data_type_new_udt(udt_field_names.len() as u64);
        for name in udt_field_names {
            assert_cass_error_eq(
                CassError::CASS_OK,
                cass_data_type_add_sub_type_by_name(
                    udt_type.borrow(),
                    CassStrNulTerminated::from_cstr(name),
                    vector_type.borrow().into_c_const(),
                ),
            );
        }
        let mut udt = cass_user_type_new_from_data_type(udt_type.borrow().into_c_const());
        let udt_fields = [
            [10.0_f32, 11.0, 12.0],
            [13.0, 14.0, 15.0],
            [16.0, 17.0, 18.0],
        ];

        let item = make_vector(vector_type.borrow().into_c_const(), udt_fields[0]);
        binds.push((
            "vector in a UDT, by index",
            cass_user_type_set_vector(udt.borrow_mut(), 0, item.borrow().into_c_const()),
        ));
        cass_vector_free(item);

        let item = make_vector(vector_type.borrow().into_c_const(), udt_fields[1]);
        binds.push((
            "vector in a UDT, by name",
            cass_user_type_set_vector_by_name(
                udt.borrow_mut(),
                CassStrNulTerminated::from_cstr(c"b"),
                item.borrow().into_c_const(),
            ),
        ));
        cass_vector_free(item);

        let item = make_vector(vector_type.borrow().into_c_const(), udt_fields[2]);
        let (name, name_length) = CassStrNulTerminated::from_cstr(c"c").as_len_delimited();
        binds.push((
            "vector in a UDT, by length-delimited name",
            cass_user_type_set_vector_by_name_n(
                udt.borrow_mut(),
                name,
                name_length,
                item.borrow().into_c_const(),
            ),
        ));
        cass_vector_free(item);

        binds.push((
            "UDT with vector fields",
            cass_statement_bind_user_type(statement.borrow_mut(), 2, udt.borrow().into_c_const()),
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
        cass_tuple_free(tuple);
        cass_user_type_free(udt);
        cass_data_type_free(list_type);
        cass_data_type_free(tuple_type);
        cass_data_type_free(udt_type);
        cass_data_type_free(vector_type);
        cass_data_type_free(float_type);
        cass_statement_free(statement);
        cass_prepared_free(prepared);

        // Read everything back, to prove the values really landed.
        let select = c"SELECT l, t, u FROM vector_nesting.containers WHERE id = 0";
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

        // The list of vectors.
        let mut iter = cass_iterator_from_collection(column(c"l"));
        for expected in list_items {
            assert_eq!(cass_iterator_next(iter.borrow_mut()), 1);
            assert_eq!(
                get_floats(cass_iterator_get_value(iter.borrow().into_c_const())),
                expected
            );
        }
        assert_eq!(cass_iterator_next(iter.borrow_mut()), 0);
        cass_iterator_free(iter);

        // The tuple's int field, then its vector field.
        let mut iter = cass_iterator_from_tuple(column(c"t"));
        assert_eq!(cass_iterator_next(iter.borrow_mut()), 1);
        let mut number = 0_i32;
        assert_cass_error_eq(
            CassError::CASS_OK,
            cass_value_get_int32(
                cass_iterator_get_value(iter.borrow().into_c_const()),
                &raw mut number,
            ),
        );
        assert_eq!(number, 42);
        assert_eq!(cass_iterator_next(iter.borrow_mut()), 1);
        assert_eq!(
            get_floats(cass_iterator_get_value(iter.borrow().into_c_const())),
            tuple_item
        );
        cass_iterator_free(iter);

        // The UDT's three vector fields, in declaration order.
        let mut iter = cass_iterator_fields_from_user_type(column(c"u"));
        for expected in udt_fields {
            assert_eq!(cass_iterator_next(iter.borrow_mut()), 1);
            assert_eq!(
                get_floats(cass_iterator_get_user_type_field_value(
                    iter.borrow().into_c_const()
                )),
                expected
            );
        }
        assert_eq!(cass_iterator_next(iter.borrow_mut()), 0);
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
