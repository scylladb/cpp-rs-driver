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
use crate::cql_types::CassValueType;
use crate::cql_types::data_type::{CassDataType, CassDataTypeInner, cass_data_type_new};
use crate::cql_types::value::CassCqlValue;
use crate::types::*;
use std::sync::Arc;

#[derive(Clone)]
pub struct CassVector {
    /// Contrary to collections and tuples, a vector is always typed: the wire
    /// representation of its elements depends on their type, so we cannot
    /// serialize a vector without knowing it.
    pub(crate) data_type: Arc<CassDataType>,

    /// The type of the vector's elements.
    ///
    /// This is a part of `data_type`. It is kept separately, so that no code
    /// has to destructure `data_type` and handle the impossible case of it
    /// not being a vector.
    #[expect(unused)]
    element_type: Arc<CassDataType>,

    /// The elements of a vector cannot be null. `None` here only means
    /// "not set yet" - such a vector is rejected upon serialization.
    #[expect(unused)]
    pub(crate) items: Vec<Option<CassCqlValue>>,
}

impl FFI for CassVector {
    type Origin = FromBox;
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
            typ: Arc::clone(&element_type),
            dimensions: dimensions_u16,
        }),
        element_type,
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
