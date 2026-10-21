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
    #[expect(unused)]
    element_type: Arc<CassDataType>,

    // The elements of a vector cannot be null. `None` here only means
    // "not set yet" - such a vector is rejected upon serialization.
    #[expect(unused)]
    pub(crate) items: Vec<Option<CassCqlValue>>,
}

impl FFI for CassVector {
    type Origin = FromBox;
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

#[cfg(test)]
mod tests {
    use crate::argconv::{ArcFFI, BoxFFI, CConst};
    use crate::cass_error::CassError;
    use crate::cql_types::CassValueType;
    use crate::cql_types::data_type::{
        cass_data_type_free, cass_data_type_new, cass_data_type_new_vector,
        cass_data_type_vector_dimensions,
    };
    use crate::testing::utils::assert_cass_error_eq;
    use crate::types::cass_uint16_t;

    use super::{
        CassVector, cass_vector_data_type, cass_vector_free, cass_vector_new,
        cass_vector_new_from_data_type,
    };

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
