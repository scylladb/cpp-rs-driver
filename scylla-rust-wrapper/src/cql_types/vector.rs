//! CQL vector: a fixed-size sequence of values of the same type.
//!
//! A vector gets its own type here, modelled after
//! [`CassTuple`](crate::cql_types::tuple::CassTuple). It is not served by
//! [`CassCollection`](crate::cql_types::collection::CassCollection). The
//! reason is the shape of the API:
//!
//! - A collection grows. The user appends to it with
//!   `cass_collection_append_*()`, and its items fill in order.
//! - A vector has a fixed number of slots. The user fills them by position
//!   with `cass_vector_set_*()`, and can set slot 2 before slot 0.
//!
//! A collection has no way to hold a partly filled, positional buffer. That
//! is why the elements here are a pre-sized `Vec` of `Option`s, as they are
//! in a tuple.

use crate::argconv::*;
use crate::cass_error::CassError;
use crate::cql_types::CassValueType;
use crate::cql_types::data_type::{
    CassDataType, CassDataTypeInner, cass_data_type_free, cass_data_type_new,
    cass_data_type_new_vector,
};
use crate::cql_types::value::CassCqlValue;
use crate::types::*;
use std::sync::Arc;

#[derive(Clone)]
pub struct CassVector {
    /// Contrary to a collection or a tuple, a vector is always fully typed.
    pub(crate) data_type: Arc<CassDataType>,

    /// The type of the vector's elements.
    //
    // This is a part of `data_type`. It is kept separately, so that no code
    // has to destructure `data_type` and handle the impossible case of it
    // not being a vector.
    element_type: Arc<CassDataType>,

    // The elements of a vector cannot be null. `None` here only means
    // "not set yet" - such a vector is rejected upon serialization.
    pub(crate) items: Vec<Option<CassCqlValue>>,
}

impl FFI for CassVector {
    type Origin = FromBox;
}

impl CassVector {
    /// Analogous to `CassTuple::bind_value`, except that:
    /// - a vector is always typed, so the value is always typechecked against the element type;
    /// - a vector can't contain NULL values, so `v` is not an Option.
    fn bind_value(&mut self, index: usize, v: CassCqlValue) -> CassError {
        if index >= self.items.len() {
            return CassError::CASS_ERROR_LIB_INDEX_OUT_OF_BOUNDS;
        }

        if !v.is_type_compatible(&self.element_type) {
            return CassError::CASS_ERROR_LIB_INVALID_VALUE_TYPE;
        }

        self.items[index] = Some(v);

        CassError::CASS_OK
    }
}

impl From<&CassVector> for CassCqlValue {
    fn from(vector: &CassVector) -> Self {
        CassCqlValue::Vector {
            element_type: Arc::clone(&vector.element_type),
            data_type: Arc::clone(&vector.data_type),
            values: vector.items.clone(),
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_vector_new(
    element_type: CassValueType,
    dimensions: cass_uint16_t,
) -> CassOwnedExclusivePtr<CassVector, CMut> {
    // Only native types can be provided this way. For a vector of any other type
    // (a UDT, a tuple, a collection or another vector), the user needs to build
    // the data type and use `cass_vector_new_from_data_type`.
    let element_type_ptr = unsafe { cass_data_type_new(element_type) };
    let Some(element_type) = ArcFFI::from_ptr(element_type_ptr) else {
        tracing::error!("Provided invalid element value type to cass_vector_new!");
        return BoxFFI::null_mut();
    };

    // `cass_data_type_new` happily creates untyped collections, tuples and UDTs,
    // but an untyped element type would contradict a vector always being fully
    // typed. Such element types have to be built by the user instead.
    if !matches!(
        unsafe { element_type.get_unchecked() },
        CassDataTypeInner::Value(_)
    ) {
        tracing::error!("Provided non-native element value type to cass_vector_new!");
        return BoxFFI::null_mut();
    }

    // The dimensions are validated once, in `cass_data_type_new_vector`.
    let data_type = unsafe { cass_data_type_new_vector(ArcFFI::as_ptr(&element_type), dimensions) };
    let vector = unsafe { cass_vector_new_from_data_type(data_type.borrow().into_c_const()) };
    unsafe { cass_data_type_free(data_type) };
    vector
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_vector_new_from_data_type(
    data_type: CassBorrowedSharedPtr<CassDataType, CConst>,
) -> CassOwnedExclusivePtr<CassVector, CMut> {
    let Some(data_type) = ArcFFI::cloned_from_ptr(data_type) else {
        tracing::error!("Provided null data type pointer to cass_vector_new_from_data_type!");
        return BoxFFI::null_mut();
    };

    let (element_type, dimensions) = match unsafe { data_type.get_unchecked() } {
        CassDataTypeInner::Vector { typ, dimensions } => (Arc::clone(typ), *dimensions as usize),
        _ => return BoxFFI::null_mut(),
    };

    BoxFFI::into_ptr(Box::new(CassVector {
        data_type,
        element_type,
        items: vec![None; dimensions],
    }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_vector_free(vector: CassOwnedExclusivePtr<CassVector, CMut>) {
    BoxFFI::free(vector);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_vector_data_type(
    vector: CassBorrowedSharedPtr<CassVector, CConst>,
) -> CassBorrowedSharedPtr<CassDataType, CConst> {
    let Some(vector) = BoxFFI::as_ref(vector) else {
        tracing::error!("Provided null vector pointer to cass_vector_data_type!");
        return ArcFFI::null();
    };

    ArcFFI::as_ptr(&vector.data_type)
}

// Notice the lack of `null` below: a vector element cannot be null. No setter
// here produces `None`, so the `None` arm is unreachable.
prepare_binders_macro!(@only_index CassVector, |vector: &mut CassVector, index, v: Option<CassCqlValue>| {
    match v {
        Some(v) => vector.bind_value(index, v),
        None => CassError::CASS_ERROR_LIB_BAD_PARAMS,
    }
});
make_binders!(int8, cass_vector_set_int8);
make_binders!(int16, cass_vector_set_int16);
make_binders!(int32, cass_vector_set_int32);
make_binders!(uint32, cass_vector_set_uint32);
make_binders!(int64, cass_vector_set_int64);
make_binders!(float, cass_vector_set_float);
make_binders!(double, cass_vector_set_double);
make_binders!(bool, cass_vector_set_bool);
make_binders!(string, cass_vector_set_string);
make_binders!(string_n, cass_vector_set_string_n);
make_binders!(bytes, cass_vector_set_bytes);
make_binders!(uuid, cass_vector_set_uuid);
make_binders!(inet, cass_vector_set_inet);
make_binders!(duration, cass_vector_set_duration);
make_binders!(decimal, cass_vector_set_decimal);
make_binders!(collection, cass_vector_set_collection);
make_binders!(tuple, cass_vector_set_tuple);
make_binders!(user_type, cass_vector_set_user_type);
make_binders!(vector, cass_vector_set_vector);

#[cfg(test)]
mod tests {
    use scylla::cluster::metadata::NativeType;
    use scylla::cluster::metadata::UserDefinedType;
    use scylla::frame::response::result::{CollectionType, ColumnType};
    use scylla::serialize::value::{
        BuiltinSerializationErrorKind, SerializeValue, VectorSerializationErrorKind,
    };
    use scylla::serialize::writers::CellWriter;
    use scylla::value::{CqlDate, CqlDecimal, CqlDuration, CqlTimestamp, CqlValue};
    use std::net::IpAddr;
    use std::sync::Arc;

    use uuid::Uuid;

    use crate::argconv::{
        ArcFFI, BoxFFI, CConst, CMut, CassBorrowedExclusivePtr, CassStrLen, CassStrLenDelimited,
        CassStrNulTerminated,
    };
    use crate::cass_error::CassError;
    use crate::cql_types::CassValueType;
    use crate::cql_types::collection::{
        cass_collection_append_int32, cass_collection_free, cass_collection_new_from_data_type,
    };
    use crate::cql_types::data_type::{
        cass_data_type_add_sub_value_type, cass_data_type_add_sub_value_type_by_name,
        cass_data_type_free, cass_data_type_new, cass_data_type_new_tuple, cass_data_type_new_udt,
        cass_data_type_new_vector, cass_data_type_vector_dimensions,
    };
    use crate::cql_types::inet::CassInet;
    use crate::cql_types::tuple::{
        cass_tuple_free, cass_tuple_new_from_data_type, cass_tuple_set_int32, cass_tuple_set_string,
    };
    use crate::cql_types::user_type::{
        cass_user_type_free, cass_user_type_new_from_data_type, cass_user_type_set_int32,
        cass_user_type_set_string,
    };
    use crate::cql_types::uuid::CassUuid;
    use crate::cql_types::value::{CassCqlValue, CassSerializationError, VectorElementNotSetError};
    use crate::testing::utils::assert_cass_error_eq;
    use crate::types::{cass_false, cass_true, cass_uint16_t, size_t};

    use super::{
        CassVector, cass_vector_data_type, cass_vector_free, cass_vector_new,
        cass_vector_new_from_data_type, cass_vector_set_bool, cass_vector_set_bytes,
        cass_vector_set_collection, cass_vector_set_decimal, cass_vector_set_double,
        cass_vector_set_duration, cass_vector_set_float, cass_vector_set_inet,
        cass_vector_set_int8, cass_vector_set_int16, cass_vector_set_int32, cass_vector_set_int64,
        cass_vector_set_string, cass_vector_set_string_n, cass_vector_set_tuple,
        cass_vector_set_uint32, cass_vector_set_user_type, cass_vector_set_uuid,
        cass_vector_set_vector,
    };

    /// Serializes the value the same way it would be serialized when bound to a statement.
    fn serialize(value: &CassCqlValue) -> Vec<u8> {
        let mut buf = Vec::new();
        value
            .serialize(
                // `typ` is not used anyway, so we can pass arbitrary type here.
                &ColumnType::Native(NativeType::Int),
                CellWriter::new(&mut buf),
            )
            .unwrap();
        buf
    }

    /// Serializes the value using rust-driver's own vector serialization,
    /// which is our reference implementation.
    fn serialize_reference<T: SerializeValue>(
        values: &Vec<T>,
        element_type: ColumnType,
    ) -> Vec<u8> {
        let typ = ColumnType::Vector {
            typ: Box::new(element_type),
            dimensions: values.len() as u16,
        };

        let mut buf = Vec::new();
        values.serialize(&typ, CellWriter::new(&mut buf)).unwrap();
        buf
    }

    /// Our serialization of a vector must agree with rust-driver's, both for
    /// fixed-size elements (written raw) and for variable-size ones (prefixed
    /// with an unsigned vint length).
    #[test]
    fn test_serialize_vector_matches_rust_driver() {
        /// Builds a vector of `c_values` through the C API, and asserts that it
        /// serializes exactly like rust-driver serializes `ref_values`.
        macro_rules! check {
            ($value_type:expr, $setter:ident, $c_values:expr, $ref_values:expr, $column_type:expr) => {{
                let c_values = $c_values;
                let ref_values = $ref_values;
                let mut vector = cass_vector_new($value_type, c_values.len() as cass_uint16_t);
                for (i, v) in c_values.iter().enumerate() {
                    assert_cass_error_eq!(
                        $setter(vector.borrow_mut(), i as size_t, *v),
                        CassError::CASS_OK
                    );
                }
                let value: CassCqlValue = BoxFFI::as_ref(vector.borrow().into_c_const())
                    .unwrap()
                    .into();
                assert_eq!(
                    serialize(&value),
                    serialize_reference(&ref_values, $column_type),
                    "serialization of a vector of {:?} differs from rust-driver's",
                    $value_type
                );
                cass_vector_free(vector);
            }};
        }

        unsafe {
            use CassValueType as V;

            // Fixed-size element types, of every width a vector knows.
            // They are written raw, with no length prefix at all.

            // 1 byte.
            check!(
                V::CASS_VALUE_TYPE_BOOLEAN,
                cass_vector_set_bool,
                [cass_true, cass_false, cass_true],
                vec![true, false, true],
                ColumnType::Native(NativeType::Boolean)
            );
            // 4 bytes.
            check!(
                V::CASS_VALUE_TYPE_FLOAT,
                cass_vector_set_float,
                [1.0_f32, -2.5, 0.0, 42.25],
                vec![1.0_f32, -2.5, 0.0, 42.25],
                ColumnType::Native(NativeType::Float)
            );
            check!(
                V::CASS_VALUE_TYPE_INT,
                cass_vector_set_int32,
                [42_i32, -1, 0],
                vec![42_i32, -1, 0],
                ColumnType::Native(NativeType::Int)
            );
            // 8 bytes.
            check!(
                V::CASS_VALUE_TYPE_DOUBLE,
                cass_vector_set_double,
                [1.0_f64, -2.5, 0.0],
                vec![1.0_f64, -2.5, 0.0],
                ColumnType::Native(NativeType::Double)
            );
            check!(
                V::CASS_VALUE_TYPE_BIGINT,
                cass_vector_set_int64,
                [42_i64, -1, 0],
                vec![42_i64, -1, 0],
                ColumnType::Native(NativeType::BigInt)
            );
            check!(
                V::CASS_VALUE_TYPE_TIMESTAMP,
                cass_vector_set_int64,
                [42_i64, -1, 0],
                vec![CqlTimestamp(42), CqlTimestamp(-1), CqlTimestamp(0)],
                ColumnType::Native(NativeType::Timestamp)
            );
            // 16 bytes.
            let uuids = [
                Uuid::from_u128(0),
                Uuid::from_u128(1),
                Uuid::from_u128(u128::MAX),
            ];
            check!(
                V::CASS_VALUE_TYPE_UUID,
                cass_vector_set_uuid,
                uuids
                    .iter()
                    .copied()
                    .map(CassUuid::from)
                    .collect::<Vec<_>>(),
                uuids.to_vec(),
                ColumnType::Native(NativeType::Uuid)
            );

            // Variable-size element types. They are prefixed with an unsigned
            // vint length. Note that `smallint` is one of them, even though its
            // values are of a fixed width everywhere else.
            check!(
                V::CASS_VALUE_TYPE_SMALL_INT,
                cass_vector_set_int16,
                [7_i16, -1],
                vec![7_i16, -1],
                ColumnType::Native(NativeType::SmallInt)
            );

            check!(
                V::CASS_VALUE_TYPE_TINY_INT,
                cass_vector_set_int8,
                [7_i8, -1],
                vec![7_i8, -1],
                ColumnType::Native(NativeType::TinyInt)
            );
            check!(
                V::CASS_VALUE_TYPE_DATE,
                cass_vector_set_uint32,
                [0_u32, 1, 100],
                vec![CqlDate(0), CqlDate(1), CqlDate(100)],
                ColumnType::Native(NativeType::Date)
            );
            let addresses = [
                IpAddr::from([127, 0, 0, 1]),
                IpAddr::from([0, 0, 0, 0, 0, 0, 0, 1]),
            ];
            check!(
                V::CASS_VALUE_TYPE_INET,
                cass_vector_set_inet,
                addresses
                    .iter()
                    .copied()
                    .map(CassInet::from)
                    .collect::<Vec<_>>(),
                addresses.to_vec(),
                ColumnType::Native(NativeType::Inet)
            );

            // `text`, with element lengths that need one and two vint bytes.
            let strings = vec!["".to_owned(), "alpha".to_owned(), "b".repeat(300)];
            let mut vector = cass_vector_new(V::CASS_VALUE_TYPE_TEXT, 3);
            for (i, v) in strings.iter().enumerate() {
                let cstr = std::ffi::CString::new(v.as_str()).unwrap();
                assert_cass_error_eq!(
                    cass_vector_set_string(
                        vector.borrow_mut(),
                        i as size_t,
                        CassStrNulTerminated::from_raw(cstr.as_ptr())
                    ),
                    CassError::CASS_OK
                );
            }
            let value: CassCqlValue = BoxFFI::as_ref(vector.borrow().into_c_const())
                .unwrap()
                .into();
            assert_eq!(
                serialize(&value),
                serialize_reference(&strings, ColumnType::Native(NativeType::Text))
            );
            cass_vector_free(vector);

            // Vectors of vectors. Whether the inner vector carries a length
            // prefix follows from its own element type, so both cases matter.
            check_nested_vector(
                V::CASS_VALUE_TYPE_FLOAT,
                ColumnType::Native(NativeType::Float),
                vec![vec![1.0_f32, -2.5], vec![0.0, 42.25]],
                |vector, index, value| cass_vector_set_float(vector, index, *value),
            );
            check_nested_vector(
                V::CASS_VALUE_TYPE_TEXT,
                ColumnType::Native(NativeType::Text),
                vec![
                    vec!["".to_owned(), "alpha".to_owned()],
                    vec!["b".repeat(300), "gamma".to_owned()],
                ],
                |vector, index, value| {
                    let cstr = std::ffi::CString::new(value.as_str()).unwrap();
                    cass_vector_set_string(
                        vector,
                        index,
                        CassStrNulTerminated::from_raw(cstr.as_ptr()),
                    )
                },
            );
        }
    }

    /// Builds a `vector<vector<element, N>, M>` through the C API and asserts
    /// that it serializes exactly like rust-driver serializes `values`.
    ///
    /// An inner vector of a fixed-size type is itself fixed-size, so the outer
    /// vector writes it raw. An inner vector of a variable-size type is not, so
    /// the outer vector prefixes it with a vint length - and the inner one
    /// prefixes its own elements too.
    unsafe fn check_nested_vector<T: SerializeValue>(
        element_value_type: CassValueType,
        element_column_type: ColumnType<'static>,
        values: Vec<Vec<T>>,
        set: unsafe fn(CassBorrowedExclusivePtr<CassVector, CMut>, size_t, &T) -> CassError,
    ) {
        unsafe {
            let element_type = cass_data_type_new(element_value_type);
            let inner_type = cass_data_type_new_vector(
                element_type.borrow().into_c_const(),
                values[0].len() as cass_uint16_t,
            );
            let outer_type = cass_data_type_new_vector(
                inner_type.borrow().into_c_const(),
                values.len() as cass_uint16_t,
            );

            let mut outer = cass_vector_new_from_data_type(outer_type.borrow().into_c_const());
            for (i, inner_values) in values.iter().enumerate() {
                let mut inner = cass_vector_new_from_data_type(inner_type.borrow().into_c_const());
                for (j, v) in inner_values.iter().enumerate() {
                    assert_cass_error_eq!(
                        set(inner.borrow_mut(), j as size_t, v),
                        CassError::CASS_OK
                    );
                }
                assert_cass_error_eq!(
                    cass_vector_set_vector(
                        outer.borrow_mut(),
                        i as size_t,
                        inner.borrow().into_c_const()
                    ),
                    CassError::CASS_OK
                );
                cass_vector_free(inner);
            }

            let value: CassCqlValue = BoxFFI::as_ref(outer.borrow().into_c_const())
                .unwrap()
                .into();
            assert_eq!(
                serialize(&value),
                serialize_reference(
                    &values,
                    ColumnType::Vector {
                        typ: Box::new(element_column_type),
                        dimensions: values[0].len() as u16,
                    }
                ),
                "serialization of a vector of vectors of {element_value_type:?} differs from rust-driver's"
            );

            cass_vector_free(outer);
            cass_data_type_free(outer_type);
            cass_data_type_free(inner_type);
            cass_data_type_free(element_type);
        }
    }

    /// A tuple is a composite element type, and it is always of a variable size.
    /// The vector prefixes each tuple with a vint length, and the tuple writes
    /// its own fields with 4-byte length prefixes. The two schemes must compose.
    #[test]
    fn test_serialize_vector_of_tuples_matches_rust_driver() {
        unsafe {
            let values: Vec<(i32, String)> = vec![
                (42, "alpha".to_owned()),
                (-1, "".to_owned()),
                (0, "b".repeat(300)),
            ];

            // The element type must be fully typed before the vector type is
            // created, because `cass_data_type_new_vector` rejects it otherwise.
            let tuple_type = cass_data_type_new_tuple(2);
            assert_cass_error_eq!(
                cass_data_type_add_sub_value_type(
                    tuple_type.borrow(),
                    CassValueType::CASS_VALUE_TYPE_INT
                ),
                CassError::CASS_OK
            );
            assert_cass_error_eq!(
                cass_data_type_add_sub_value_type(
                    tuple_type.borrow(),
                    CassValueType::CASS_VALUE_TYPE_TEXT
                ),
                CassError::CASS_OK
            );
            let vector_type = cass_data_type_new_vector(
                tuple_type.borrow().into_c_const(),
                values.len() as cass_uint16_t,
            );

            let mut vector = cass_vector_new_from_data_type(vector_type.borrow().into_c_const());
            for (i, (number, string)) in values.iter().enumerate() {
                let mut tuple = cass_tuple_new_from_data_type(tuple_type.borrow().into_c_const());
                assert_cass_error_eq!(
                    cass_tuple_set_int32(tuple.borrow_mut(), 0, *number),
                    CassError::CASS_OK
                );
                let cstr = std::ffi::CString::new(string.as_str()).unwrap();
                assert_cass_error_eq!(
                    cass_tuple_set_string(
                        tuple.borrow_mut(),
                        1,
                        CassStrNulTerminated::from_raw(cstr.as_ptr())
                    ),
                    CassError::CASS_OK
                );
                assert_cass_error_eq!(
                    cass_vector_set_tuple(
                        vector.borrow_mut(),
                        i as size_t,
                        tuple.borrow().into_c_const()
                    ),
                    CassError::CASS_OK
                );
                cass_tuple_free(tuple);
            }

            let value: CassCqlValue = BoxFFI::as_ref(vector.borrow().into_c_const())
                .unwrap()
                .into();
            assert_eq!(
                serialize(&value),
                serialize_reference(
                    &values,
                    ColumnType::Tuple(vec![
                        ColumnType::Native(NativeType::Int),
                        ColumnType::Native(NativeType::Text),
                    ])
                )
            );

            cass_vector_free(vector);
            cass_data_type_free(vector_type);
            cass_data_type_free(tuple_type);
        }
    }

    /// A collection is a composite element type too, and it is also always of a
    /// variable size. The vector prefixes each list with a vint length, and the
    /// list writes its own element count and per-element length prefixes.
    #[test]
    fn test_serialize_vector_of_lists_matches_rust_driver() {
        unsafe {
            let values: Vec<Vec<i32>> = vec![vec![], vec![42], vec![-1, 0, 7]];

            let list_type = cass_data_type_new(CassValueType::CASS_VALUE_TYPE_LIST);
            assert_cass_error_eq!(
                cass_data_type_add_sub_value_type(
                    list_type.borrow(),
                    CassValueType::CASS_VALUE_TYPE_INT
                ),
                CassError::CASS_OK
            );
            let vector_type = cass_data_type_new_vector(
                list_type.borrow().into_c_const(),
                values.len() as cass_uint16_t,
            );

            let mut vector = cass_vector_new_from_data_type(vector_type.borrow().into_c_const());
            for (i, list_values) in values.iter().enumerate() {
                let mut list = cass_collection_new_from_data_type(
                    list_type.borrow().into_c_const(),
                    list_values.len() as size_t,
                );
                for v in list_values {
                    assert_cass_error_eq!(
                        cass_collection_append_int32(list.borrow_mut(), *v),
                        CassError::CASS_OK
                    );
                }
                assert_cass_error_eq!(
                    cass_vector_set_collection(
                        vector.borrow_mut(),
                        i as size_t,
                        list.borrow().into_c_const()
                    ),
                    CassError::CASS_OK
                );
                cass_collection_free(list);
            }

            let value: CassCqlValue = BoxFFI::as_ref(vector.borrow().into_c_const())
                .unwrap()
                .into();
            assert_eq!(
                serialize(&value),
                serialize_reference(
                    &values,
                    ColumnType::Collection {
                        frozen: true,
                        typ: CollectionType::List(Box::new(ColumnType::Native(NativeType::Int))),
                    }
                )
            );

            cass_vector_free(vector);
            cass_data_type_free(vector_type);
            cass_data_type_free(list_type);
        }
    }

    /// Builds a vector through the C API, letting `set` place one element at a
    /// time, and asserts that it serializes exactly like rust-driver serializes
    /// `reference`.
    ///
    /// Unlike the `check!` macro, this takes a closure, so it also serves the
    /// setters that take more than one argument per element.
    unsafe fn check_vector<T: SerializeValue>(
        value_type: CassValueType,
        column_type: ColumnType<'static>,
        reference: Vec<T>,
        mut set: impl FnMut(CassBorrowedExclusivePtr<CassVector, CMut>, size_t),
    ) {
        unsafe {
            let mut vector = cass_vector_new(value_type, reference.len() as cass_uint16_t);
            for i in 0..reference.len() {
                set(vector.borrow_mut(), i as size_t);
            }

            let value: CassCqlValue = BoxFFI::as_ref(vector.borrow().into_c_const())
                .unwrap()
                .into();
            assert_eq!(
                serialize(&value),
                serialize_reference(&reference, column_type),
                "serialization of a vector of {value_type:?} differs from rust-driver's"
            );
            cass_vector_free(vector);
        }
    }

    /// Some element setters take more than one argument per element: a pointer
    /// and a length, or the three components of a duration. They all reach the
    /// same variable-length encoding, so each of them needs its own case.
    #[test]
    fn test_serialize_vector_with_multi_argument_setters_matches_rust_driver() {
        unsafe {
            // `text`, set through the length-delimited setter rather than the
            // nul-terminated one.
            let strings = ["".to_owned(), "alpha".to_owned(), "b".repeat(300)];
            check_vector(
                CassValueType::CASS_VALUE_TYPE_TEXT,
                ColumnType::Native(NativeType::Text),
                strings.to_vec(),
                |vector, index| {
                    let value = strings[index as usize].as_str();
                    assert_cass_error_eq!(
                        cass_vector_set_string_n(
                            vector,
                            index,
                            CassStrLenDelimited::from_raw(value.as_ptr().cast()),
                            CassStrLen::from_raw(value.len() as size_t)
                        ),
                        CassError::CASS_OK
                    );
                },
            );

            // `blob`, including an empty one.
            let blobs = [vec![], vec![0_u8], vec![1_u8, 2, 3]];
            check_vector(
                CassValueType::CASS_VALUE_TYPE_BLOB,
                ColumnType::Native(NativeType::Blob),
                blobs.iter().cloned().map(CqlValue::Blob).collect(),
                |vector, index| {
                    let value = &blobs[index as usize];
                    assert_cass_error_eq!(
                        cass_vector_set_bytes(vector, index, value.as_ptr(), value.len() as size_t),
                        CassError::CASS_OK
                    );
                },
            );

            // `decimal`, which is a varint plus a scale.
            let decimals: [(Vec<u8>, i32); 2] = [(vec![0x01, 0x02], 3), (vec![0xFF], -1)];
            check_vector(
                CassValueType::CASS_VALUE_TYPE_DECIMAL,
                ColumnType::Native(NativeType::Decimal),
                decimals
                    .iter()
                    .map(|(varint, scale)| {
                        CqlValue::Decimal(CqlDecimal::from_signed_be_bytes_slice_and_exponent(
                            varint, *scale,
                        ))
                    })
                    .collect(),
                |vector, index| {
                    let (varint, scale) = &decimals[index as usize];
                    assert_cass_error_eq!(
                        cass_vector_set_decimal(
                            vector,
                            index,
                            varint.as_ptr(),
                            varint.len() as size_t,
                            *scale
                        ),
                        CassError::CASS_OK
                    );
                },
            );

            // `duration`, which is months, days and nanoseconds.
            let durations = [(1_i32, 2_i32, 3_i64), (-1, -2, -3)];
            check_vector(
                CassValueType::CASS_VALUE_TYPE_DURATION,
                ColumnType::Native(NativeType::Duration),
                durations
                    .iter()
                    .map(|(months, days, nanoseconds)| {
                        CqlValue::Duration(CqlDuration {
                            months: *months,
                            days: *days,
                            nanoseconds: *nanoseconds,
                        })
                    })
                    .collect(),
                |vector, index| {
                    let (months, days, nanoseconds) = durations[index as usize];
                    assert_cass_error_eq!(
                        cass_vector_set_duration(vector, index, months, days, nanoseconds),
                        CassError::CASS_OK
                    );
                },
            );
        }
    }

    /// A UDT is a composite element type, like a tuple, but its fields are
    /// named. The vector prefixes each UDT with a vint length, and the UDT
    /// writes its own per-field 4-byte length prefixes.
    #[test]
    fn test_serialize_vector_of_udts_matches_rust_driver() {
        unsafe {
            let values = [("alpha", 1_i32), ("", -1), ("gamma", 0)];

            // The field types must be added before the vector type is created,
            // because `cass_data_type_new_vector` rejects a UDT with no fields.
            let udt_type = cass_data_type_new_udt(2);
            for (name, value_type) in [
                (c"f", CassValueType::CASS_VALUE_TYPE_TEXT),
                (c"g", CassValueType::CASS_VALUE_TYPE_INT),
            ] {
                assert_cass_error_eq!(
                    cass_data_type_add_sub_value_type_by_name(
                        udt_type.borrow(),
                        CassStrNulTerminated::from_cstr(name),
                        value_type
                    ),
                    CassError::CASS_OK
                );
            }
            let vector_type = cass_data_type_new_vector(
                udt_type.borrow().into_c_const(),
                values.len() as cass_uint16_t,
            );

            let mut vector = cass_vector_new_from_data_type(vector_type.borrow().into_c_const());
            for (i, (string, number)) in values.iter().enumerate() {
                let mut udt = cass_user_type_new_from_data_type(udt_type.borrow().into_c_const());
                let cstr = std::ffi::CString::new(*string).unwrap();
                assert_cass_error_eq!(
                    cass_user_type_set_string(
                        udt.borrow_mut(),
                        0,
                        CassStrNulTerminated::from_raw(cstr.as_ptr())
                    ),
                    CassError::CASS_OK
                );
                assert_cass_error_eq!(
                    cass_user_type_set_int32(udt.borrow_mut(), 1, *number),
                    CassError::CASS_OK
                );
                assert_cass_error_eq!(
                    cass_vector_set_user_type(
                        vector.borrow_mut(),
                        i as size_t,
                        udt.borrow().into_c_const()
                    ),
                    CassError::CASS_OK
                );
                cass_user_type_free(udt);
            }

            let value: CassCqlValue = BoxFFI::as_ref(vector.borrow().into_c_const())
                .unwrap()
                .into();
            assert_eq!(
                serialize(&value),
                serialize_reference(
                    &values
                        .iter()
                        .map(|(string, number)| CqlValue::UserDefinedType {
                            keyspace: String::new(),
                            name: String::new(),
                            fields: vec![
                                ("f".to_owned(), Some(CqlValue::Text((*string).to_owned()))),
                                ("g".to_owned(), Some(CqlValue::Int(*number))),
                            ],
                        })
                        .collect(),
                    ColumnType::UserDefinedType {
                        frozen: true,
                        definition: Arc::new(UserDefinedType {
                            keyspace: "".into(),
                            name: "".into(),
                            field_types: vec![
                                ("f".into(), ColumnType::Native(NativeType::Text)),
                                ("g".into(), ColumnType::Native(NativeType::Int)),
                            ],
                        }),
                    }
                )
            );

            cass_vector_free(vector);
            cass_data_type_free(vector_type);
            cass_data_type_free(udt_type);
        }
    }

    /// A vector element cannot be null, so serializing a vector with an element
    /// that was never set must fail instead of silently writing garbage.
    #[test]
    fn test_serialize_vector_with_unset_element_fails() {
        unsafe {
            let mut vector = cass_vector_new(CassValueType::CASS_VALUE_TYPE_FLOAT, 2);
            assert_cass_error_eq!(
                cass_vector_set_float(vector.borrow_mut(), 0, 1.0),
                CassError::CASS_OK
            );

            let value: CassCqlValue = BoxFFI::as_ref(vector.borrow().into_c_const())
                .unwrap()
                .into();

            let mut buf = Vec::new();
            let err = value
                .serialize(
                    // `typ` is not used anyway, so we can pass arbitrary type here.
                    &ColumnType::Native(NativeType::Int),
                    CellWriter::new(&mut buf),
                )
                .unwrap_err();

            // The error must say which element was left unset.
            let err = err
                .downcast_ref::<CassSerializationError>()
                .expect("expected a CassSerializationError");

            let BuiltinSerializationErrorKind::VectorError(
                VectorSerializationErrorKind::ElementSerializationFailed(element_err),
            ) = &err.kind
            else {
                panic!("expected a vector element error, got {:?}", err.kind)
            };

            let not_set = element_err
                .downcast_ref::<VectorElementNotSetError>()
                .expect("expected a VectorElementNotSetError");
            assert_eq!(not_set.index, 1);
            // The message must name the element, so that a user can act on it.
            assert!(
                not_set.to_string().contains('1'),
                "unhelpful message: {not_set}"
            );

            cass_vector_free(vector);
        }
    }

    #[test]
    fn test_vector_element_typecheck() {
        unsafe {
            let mut vector = cass_vector_new(CassValueType::CASS_VALUE_TYPE_FLOAT, 2);

            // Wrong element type.
            assert_cass_error_eq!(
                cass_vector_set_double(vector.borrow_mut(), 0, 1.0),
                CassError::CASS_ERROR_LIB_INVALID_VALUE_TYPE
            );

            // Index out of bounds.
            assert_cass_error_eq!(
                cass_vector_set_float(vector.borrow_mut(), 2, 1.0),
                CassError::CASS_ERROR_LIB_INDEX_OUT_OF_BOUNDS
            );

            assert_cass_error_eq!(
                cass_vector_set_float(vector.borrow_mut(), 1, 1.0),
                CassError::CASS_OK
            );

            cass_vector_free(vector);
        }
    }

    #[test]
    fn test_vector_construction() {
        unsafe {
            // A vector requires a native element type...
            assert!(
                BoxFFI::as_ref(
                    cass_vector_new(CassValueType::CASS_VALUE_TYPE_UNKNOWN, 3)
                        .borrow()
                        .into_c_const()
                )
                .is_none()
            );

            // ...and a valid number of dimensions.
            assert!(
                BoxFFI::as_ref(
                    cass_vector_new(CassValueType::CASS_VALUE_TYPE_FLOAT, 0)
                        .borrow()
                        .into_c_const()
                )
                .is_none()
            );

            // A non-native element type has to be built by the user and passed
            // to `cass_vector_new_from_data_type` instead - an untyped one would
            // contradict a vector always being fully typed.
            for value_type in [
                CassValueType::CASS_VALUE_TYPE_LIST,
                CassValueType::CASS_VALUE_TYPE_SET,
                CassValueType::CASS_VALUE_TYPE_MAP,
                CassValueType::CASS_VALUE_TYPE_TUPLE,
                CassValueType::CASS_VALUE_TYPE_UDT,
                CassValueType::CASS_VALUE_TYPE_VECTOR,
            ] {
                assert!(
                    BoxFFI::as_ref(cass_vector_new(value_type, 3).borrow().into_c_const())
                        .is_none()
                );
            }

            // A vector built from a data type reports that very data type back.
            let element_type = cass_data_type_new(CassValueType::CASS_VALUE_TYPE_FLOAT);
            let vector_type = cass_data_type_new_vector(element_type.borrow().into_c_const(), 3);

            let vector = cass_vector_new_from_data_type(vector_type.borrow().into_c_const());
            let data_type = cass_vector_data_type(vector.borrow().into_c_const());
            assert!(ArcFFI::as_ref(data_type.borrow()).is_some());

            let mut dimensions: cass_uint16_t = 0;
            assert_cass_error_eq!(
                cass_data_type_vector_dimensions(data_type, &raw mut dimensions),
                CassError::CASS_OK
            );
            assert_eq!(dimensions, 3);

            // A non-vector data type is rejected.
            assert!(
                BoxFFI::as_ref(
                    cass_vector_new_from_data_type(element_type.borrow().into_c_const())
                        .borrow()
                        .into_c_const()
                )
                .is_none()
            );

            cass_vector_free(vector);
            cass_data_type_free(vector_type);
            cass_data_type_free(element_type);
        }
    }

    /// Every public vector function must reject a null pointer or an invalid
    /// argument with a null result, instead of dereferencing or panicking.
    #[test]
    fn test_vector_api_rejects_invalid_arguments() {
        unsafe {
            // No dimensions at all.
            let vector = cass_vector_new(CassValueType::CASS_VALUE_TYPE_FLOAT, 0);
            assert!(BoxFFI::as_ref(vector.borrow().into_c_const()).is_none());

            // A null data type.
            let vector = cass_vector_new_from_data_type(ArcFFI::null());
            assert!(BoxFFI::as_ref(vector.borrow().into_c_const()).is_none());

            // A data type that is not a vector.
            let not_a_vector = cass_data_type_new(CassValueType::CASS_VALUE_TYPE_FLOAT);
            let vector = cass_vector_new_from_data_type(not_a_vector.borrow().into_c_const());
            assert!(BoxFFI::as_ref(vector.borrow().into_c_const()).is_none());
            cass_data_type_free(not_a_vector);

            // A null vector.
            assert!(
                ArcFFI::as_ref(cass_vector_data_type(
                    <CassVector as BoxFFI>::null::<CConst>()
                ))
                .is_none()
            );
        }
    }
}
