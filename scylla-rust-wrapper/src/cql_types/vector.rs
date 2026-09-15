//! CQL vector: a fixed-size sequence of values of the same type.
//!
//! Notice that a vector is **not** a CQL collection:
//! - its number of dimensions is part of its type,
//! - its elements cannot be null,
//! - it can only be updated as a whole.
//!
//! This is why vectors get their own type in the API, instead of being
//! served by [`CassCollection`](crate::cql_types::collection::CassCollection).

use crate::argconv::*;
use crate::cass_error::CassError;
use crate::cql_types::CassValueType;
use crate::cql_types::data_type::{CassDataType, CassDataTypeInner, cass_data_type_new};
use crate::cql_types::value;
use crate::cql_types::value::CassCqlValue;
use crate::types::*;
use std::sync::Arc;

#[derive(Clone)]
pub struct CassVector {
    /// Contrary to collections and tuples, a vector is always typed: the wire
    /// representation of its elements depends on their type, so we cannot
    /// serialize a vector without knowing it.
    pub(crate) data_type: Arc<CassDataType>,
    /// The elements of a vector cannot be null. `None` here only means
    /// "not set yet" - such a vector is rejected upon serialization.
    pub(crate) items: Vec<Option<CassCqlValue>>,
}

impl FFI for CassVector {
    type Origin = FromBox;
}

impl CassVector {
    /// Returns the type of the vector's elements.
    fn get_element_type(&self) -> &Arc<CassDataType> {
        match unsafe { self.data_type.as_ref().get_unchecked() } {
            CassDataTypeInner::Vector { typ, .. } => typ,
            _ => unreachable!("CassVector with a non-vector data type!"),
        }
    }

    /// Analogous to `CassTuple::bind_value`, except that a vector is always typed,
    /// so the value is always typechecked against the element type.
    fn bind_value(&mut self, index: usize, v: Option<CassCqlValue>) -> CassError {
        if index >= self.items.len() {
            return CassError::CASS_ERROR_LIB_INDEX_OUT_OF_BOUNDS;
        }

        if !value::is_type_compatible(&v, self.get_element_type()) {
            return CassError::CASS_ERROR_LIB_INVALID_VALUE_TYPE;
        }

        self.items[index] = v;

        CassError::CASS_OK
    }
}

impl From<&CassVector> for CassCqlValue {
    fn from(vector: &CassVector) -> Self {
        CassCqlValue::Vector {
            data_type: vector.data_type.clone(),
            values: vector.items.clone(),
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_vector_new(
    element_type: CassValueType,
    dimensions: size_t,
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

    let Ok(dimensions_u16) = u16::try_from(dimensions) else {
        tracing::error!("Provided invalid number of dimensions to cass_vector_new: {dimensions}!");
        return BoxFFI::null_mut();
    };

    if dimensions_u16 == 0 {
        tracing::error!("Provided zero dimensions to cass_vector_new!");
        return BoxFFI::null_mut();
    }

    BoxFFI::into_ptr(Box::new(CassVector {
        data_type: CassDataType::new_arced(CassDataTypeInner::Vector {
            typ: element_type,
            dimensions: dimensions_u16,
        }),
        items: vec![None; dimensions as usize],
    }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_vector_new_from_data_type(
    data_type: CassBorrowedSharedPtr<CassDataType, CConst>,
) -> CassOwnedExclusivePtr<CassVector, CMut> {
    let Some(data_type) = ArcFFI::cloned_from_ptr(data_type) else {
        tracing::error!("Provided null data type pointer to cass_vector_new_from_data_type!");
        return BoxFFI::null_mut();
    };

    let dimensions = match unsafe { data_type.get_unchecked() } {
        CassDataTypeInner::Vector { dimensions, .. } => *dimensions as usize,
        _ => return BoxFFI::null_mut(),
    };

    BoxFFI::into_ptr(Box::new(CassVector {
        data_type,
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

prepare_binders_macro!(@only_index CassVector, |vector: &mut CassVector, index, v| vector.bind_value(index, v));
// Notice the lack of `null`: vector elements cannot be null.
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
    use scylla::frame::response::result::ColumnType;
    use scylla::serialize::value::SerializeValue;
    use scylla::serialize::writers::CellWriter;

    use crate::argconv::{ArcFFI, BoxFFI, CassStrNulTerminated};
    use crate::cass_error::CassError;
    use crate::cql_types::CassValueType;
    use crate::cql_types::data_type::{
        cass_data_type_free, cass_data_type_new, cass_data_type_new_vector,
        cass_data_type_vector_dimensions,
    };
    use crate::cql_types::value::CassCqlValue;
    use crate::testing::utils::assert_cass_error_eq;
    use crate::types::size_t;

    use super::{
        cass_vector_data_type, cass_vector_free, cass_vector_new, cass_vector_new_from_data_type,
        cass_vector_set_double, cass_vector_set_float, cass_vector_set_string,
    };

    /// Serializes the value the same way it would be serialized when bound to a statement.
    fn serialize(value: &CassCqlValue) -> Vec<u8> {
        let mut buf = Vec::new();
        value
            .serialize(
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
        unsafe {
            let floats = vec![1.0_f32, -2.5, 0.0, 42.25];
            let mut vector = cass_vector_new(CassValueType::CASS_VALUE_TYPE_FLOAT, 4);
            for (i, v) in floats.iter().enumerate() {
                assert_cass_error_eq!(
                    cass_vector_set_float(vector.borrow_mut(), i as size_t, *v),
                    CassError::CASS_OK
                );
            }
            let value: CassCqlValue = BoxFFI::as_ref(vector.borrow().into_c_const())
                .unwrap()
                .into();
            assert_eq!(
                serialize(&value),
                serialize_reference(&floats, ColumnType::Native(NativeType::Float))
            );
            cass_vector_free(vector);

            let strings = vec!["".to_owned(), "alpha".to_owned(), "b".repeat(300)];
            let mut vector = cass_vector_new(CassValueType::CASS_VALUE_TYPE_TEXT, 3);
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
            assert!(
                value
                    .serialize(
                        &ColumnType::Native(NativeType::Int),
                        CellWriter::new(&mut buf)
                    )
                    .is_err()
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

            let mut dimensions: size_t = 0;
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
}
