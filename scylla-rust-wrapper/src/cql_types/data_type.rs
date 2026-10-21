use crate::argconv::*;
use crate::cass_error::CassError;
use crate::cql_types::CassValueType;
use crate::statements::batch::CassBatchType;
use crate::types::*;
use scylla::cluster::metadata::{CollectionType, NativeType};
use scylla::frame::response::result::ColumnType;
use scylla::statement::batch::BatchType;
use std::cell::UnsafeCell;
use std::os::raw::c_char;
use std::sync::Arc;

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub(crate) struct UdtDataType {
    // Vec to preserve the order of types
    pub(crate) field_types: Vec<(String, Arc<CassDataType>)>,

    pub(crate) keyspace: String,
    pub(crate) name: String,
    pub(crate) frozen: bool,
}

impl UdtDataType {
    pub(crate) fn new() -> UdtDataType {
        UdtDataType {
            field_types: Vec::new(),
            keyspace: "".to_string(),
            name: "".to_string(),
            frozen: false,
        }
    }

    pub(crate) fn with_capacity(capacity: usize) -> UdtDataType {
        UdtDataType {
            field_types: Vec::with_capacity(capacity),
            keyspace: "".to_string(),
            name: "".to_string(),
            frozen: false,
        }
    }

    pub(crate) fn get_field_by_name(&self, name: &str) -> Option<&Arc<CassDataType>> {
        self.field_types
            .iter()
            .find(|(field_name, _)| field_name == name)
            .map(|(_, t)| t)
    }

    fn typecheck_equals(&self, other: &UdtDataType) -> bool {
        // See: https://github.com/scylladb/cpp-driver/blob/master/src/data_type.hpp#L354-L386

        if !any_string_empty_or_both_equal(&self.keyspace, &other.keyspace) {
            return false;
        }
        if !any_string_empty_or_both_equal(&self.name, &other.name) {
            return false;
        }

        // A comment from cpp-driver:
        //// UDT's can be considered equal as long as the mutual first fields shared
        //// between them are equal. UDT's are append only as far as fields go, so a
        //// newer 'version' of the UDT data type after a schema change event should be
        //// treated as equivalent in this scenario, by simply looking at the first N
        //// mutual fields they should share.
        //
        // Iterator returned from zip() is perfect for checking the first mutual fields.
        for (field, other_field) in self.field_types.iter().zip(other.field_types.iter()) {
            // Compare field names.
            if field.0 != other_field.0 {
                return false;
            }
            // Compare field types.
            if unsafe {
                !field
                    .1
                    .get_unchecked()
                    .typecheck_equals(other_field.1.get_unchecked())
            } {
                return false;
            }
        }

        true
    }
}

fn any_string_empty_or_both_equal(s1: &str, s2: &str) -> bool {
    s1.is_empty() || s2.is_empty() || s1 == s2
}

/// Compares two scalar value types for the purpose of a typecheck.
fn value_types_equal(t1: CassValueType, t2: CassValueType) -> bool {
    use CassValueType as V;
    match (t1, t2) {
        // `CASS_VALUE_TYPE_TEXT` and `CASS_VALUE_TYPE_VARCHAR` both denote the CQL
        // `text` type. The protocol has a single representation of it, which
        // rust-driver models as [`NativeType::Text`], and which we map back to
        // `CASS_VALUE_TYPE_VARCHAR`. A data type that the user built with
        // `CASS_VALUE_TYPE_TEXT` thus has to compare equal to one that was built
        // from result metadata.
        (V::CASS_VALUE_TYPE_TEXT, V::CASS_VALUE_TYPE_VARCHAR)
        | (V::CASS_VALUE_TYPE_VARCHAR, V::CASS_VALUE_TYPE_TEXT) => true,
        (t1, t2) => t1 == t2,
    }
}

impl Default for UdtDataType {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub(crate) enum MapDataType {
    Untyped,
    Key(Arc<CassDataType>),
    KeyAndValue(Arc<CassDataType>, Arc<CassDataType>),
}

#[derive(Debug)]
pub(crate) struct CassColumnSpec {
    pub(crate) name: String,
    pub(crate) data_type: Arc<CassDataType>,
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub(crate) enum CassDataTypeInner {
    Value(CassValueType),
    Udt(UdtDataType),
    List {
        // None stands for untyped list.
        typ: Option<Arc<CassDataType>>,
        frozen: bool,
    },
    Set {
        // None stands for untyped set.
        typ: Option<Arc<CassDataType>>,
        frozen: bool,
    },
    Map {
        typ: MapDataType,
        frozen: bool,
    },
    // Empty vector stands for untyped tuple.
    Tuple(Vec<Arc<CassDataType>>),
    /// A CQL vector: a fixed-size sequence of values of the same type.
    ///
    /// Notice that, contrary to collections and tuples, there is no untyped
    /// vector: both the element type and the number of dimensions are part
    /// of the type and are required upfront.
    Vector {
        typ: Arc<CassDataType>,
        dimensions: u16,
    },
    Custom(String),
}

impl FFI for CassDataType {
    type Origin = FromArc;
}

impl CassDataTypeInner {
    /// Checks for equality during typechecks.
    ///
    /// This takes into account the fact that tuples/collections may be untyped.
    /// Returns whether this data type names all of its subtypes.
    ///
    /// A collection, a tuple or a UDT may be built without them, and
    /// [`Self::typecheck_equals`] then treats the missing ones as wildcards.
    /// That is harmless for those types: their wire representation does not
    /// depend on the type of the values they hold. A vector is different -
    /// its elements are written according to their type - so a wildcard in a
    /// vector's element type could not be served.
    pub(crate) fn is_fully_typed(&self) -> bool {
        // SAFETY: the caller of a public API that reaches here holds the
        // data type, and `CassDataType` is only mutated through the C API.
        let sub_is_fully_typed =
            |typ: &Arc<CassDataType>| unsafe { typ.get_unchecked() }.is_fully_typed();

        match self {
            CassDataTypeInner::Value(_) => true,
            // A custom type is never fully typed, named or not. The driver has
            // no representation of what is inside one, and `typecheck_equals()`
            // cannot compare two of them. A name alone describes nothing.
            CassDataTypeInner::Custom(_) => false,
            CassDataTypeInner::List { typ, .. } | CassDataTypeInner::Set { typ, .. } => {
                typ.as_ref().is_some_and(sub_is_fully_typed)
            }
            CassDataTypeInner::Map { typ, .. } => match typ {
                // A map needs both its key and its value type.
                MapDataType::Untyped | MapDataType::Key(_) => false,
                MapDataType::KeyAndValue(key, value) => {
                    sub_is_fully_typed(key) && sub_is_fully_typed(value)
                }
            },
            // An empty tuple or UDT is the untyped one: `cass_data_type_new_tuple()`
            // and `cass_data_type_new_udt()` only reserve capacity for the subtypes.
            CassDataTypeInner::Tuple(subtypes) => {
                !subtypes.is_empty() && subtypes.iter().all(sub_is_fully_typed)
            }
            CassDataTypeInner::Udt(udt) => {
                !udt.field_types.is_empty()
                    && udt
                        .field_types
                        .iter()
                        .all(|(_, typ)| sub_is_fully_typed(typ))
            }
            // A vector is fully typed by construction, but recurse all the same.
            CassDataTypeInner::Vector { typ, .. } => sub_is_fully_typed(typ),
        }
    }

    pub(crate) fn typecheck_equals(&self, other: &CassDataTypeInner) -> bool {
        match self {
            CassDataTypeInner::Value(t) => value_types_equal(*t, other.get_value_type()),
            CassDataTypeInner::Udt(udt) => match other {
                CassDataTypeInner::Udt(other_udt) => udt.typecheck_equals(other_udt),
                _ => false,
            },
            CassDataTypeInner::List { typ, .. } | CassDataTypeInner::Set { typ, .. } => match other
            {
                CassDataTypeInner::List { typ: other_typ, .. }
                | CassDataTypeInner::Set { typ: other_typ, .. } => {
                    // If one of them is list, and the other is set, fail the typecheck.
                    if self.get_value_type() != other.get_value_type() {
                        return false;
                    }
                    match (typ, other_typ) {
                        // One of them is untyped, skip the typecheck for subtype.
                        (None, _) | (_, None) => true,
                        (Some(typ), Some(other_typ)) => unsafe {
                            typ.get_unchecked()
                                .typecheck_equals(other_typ.get_unchecked())
                        },
                    }
                }
                _ => false,
            },
            CassDataTypeInner::Map { typ: t, .. } => match other {
                CassDataTypeInner::Map { typ: t_other, .. } => match (t, t_other) {
                    // See https://github.com/scylladb/cpp-driver/blob/master/src/data_type.hpp#L218
                    // In cpp-driver the types are held in a vector.
                    // The logic is following:

                    // If either of vectors is empty, skip the typecheck.
                    (MapDataType::Untyped, _) => true,
                    (_, MapDataType::Untyped) => true,

                    // Otherwise, the vectors should have equal length and we perform the typecheck for subtypes.
                    (MapDataType::Key(k), MapDataType::Key(k_other)) => unsafe {
                        k.get_unchecked().typecheck_equals(k_other.get_unchecked())
                    },
                    (
                        MapDataType::KeyAndValue(k, v),
                        MapDataType::KeyAndValue(k_other, v_other),
                    ) => unsafe {
                        k.get_unchecked().typecheck_equals(k_other.get_unchecked())
                            && v.get_unchecked().typecheck_equals(v_other.get_unchecked())
                    },
                    _ => false,
                },
                _ => false,
            },
            CassDataTypeInner::Tuple(sub) => match other {
                CassDataTypeInner::Tuple(other_sub) => {
                    // If either of tuples is untyped, skip the typecheck for subtypes.
                    if sub.is_empty() || other_sub.is_empty() {
                        return true;
                    }

                    // If both are non-empty, check for subtypes equality.
                    if sub.len() != other_sub.len() {
                        return false;
                    }
                    sub.iter()
                        .zip(other_sub.iter())
                        .all(|(typ, other_typ)| unsafe {
                            typ.get_unchecked()
                                .typecheck_equals(other_typ.get_unchecked())
                        })
                }
                _ => false,
            },
            CassDataTypeInner::Vector {
                typ,
                dimensions: dims,
            } => match other {
                CassDataTypeInner::Vector {
                    typ: other_typ,
                    dimensions: other_dims,
                } => {
                    // Contrary to collections and tuples, vectors are always fully typed,
                    // so there is no untyped case to skip the typecheck for.
                    dims == other_dims
                        && unsafe {
                            typ.get_unchecked()
                                .typecheck_equals(other_typ.get_unchecked())
                        }
                }
                _ => false,
            },
            CassDataTypeInner::Custom(_) => {
                unimplemented!("cpp-rs-driver does not support custom types!")
            }
        }
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct CassDataType(UnsafeCell<CassDataTypeInner>);

/// PartialEq and Eq for test purposes.
#[cfg(test)]
impl PartialEq for CassDataType {
    fn eq(&self, other: &Self) -> bool {
        unsafe { self.get_unchecked() == other.get_unchecked() }
    }
}
#[cfg(test)]
impl Eq for CassDataType {}

unsafe impl Sync for CassDataType {}

impl CassDataType {
    pub(crate) unsafe fn get_unchecked(&self) -> &CassDataTypeInner {
        unsafe { &*self.0.get() }
    }

    #[allow(clippy::mut_from_ref)]
    pub(crate) unsafe fn get_mut_unchecked(&self) -> &mut CassDataTypeInner {
        unsafe { &mut *self.0.get() }
    }

    pub(crate) const fn new(inner: CassDataTypeInner) -> CassDataType {
        CassDataType(UnsafeCell::new(inner))
    }

    pub(crate) fn new_arced(inner: CassDataTypeInner) -> Arc<CassDataType> {
        Arc::new(CassDataType(UnsafeCell::new(inner)))
    }
}

impl From<&NativeType> for CassValueType {
    fn from(native_type: &NativeType) -> Self {
        use NativeType::*;
        match native_type {
            Ascii => CassValueType::CASS_VALUE_TYPE_ASCII,
            Boolean => CassValueType::CASS_VALUE_TYPE_BOOLEAN,
            Blob => CassValueType::CASS_VALUE_TYPE_BLOB,
            Counter => CassValueType::CASS_VALUE_TYPE_COUNTER,
            Date => CassValueType::CASS_VALUE_TYPE_DATE,
            Decimal => CassValueType::CASS_VALUE_TYPE_DECIMAL,
            Double => CassValueType::CASS_VALUE_TYPE_DOUBLE,
            Duration => CassValueType::CASS_VALUE_TYPE_DURATION,
            Float => CassValueType::CASS_VALUE_TYPE_FLOAT,
            Int => CassValueType::CASS_VALUE_TYPE_INT,
            BigInt => CassValueType::CASS_VALUE_TYPE_BIGINT,
            // Rust Driver unifies both VARCHAR and TEXT into NativeType::Text.
            // CPP Driver, in accordance to the CQL protocol, has separate types for VARCHAR and TEXT.
            // Even worse, Rust Driver even does not handle CQL TEXT correctly! It errors out on TEXT
            // type...
            // As the DBs (Cassandra and ScyllaDB) seem to send the VARCHAR type in the protocol,
            // we will assume that the NativeType::Text is actually a VARCHAR type.
            Text => CassValueType::CASS_VALUE_TYPE_VARCHAR,
            Timestamp => CassValueType::CASS_VALUE_TYPE_TIMESTAMP,
            Inet => CassValueType::CASS_VALUE_TYPE_INET,
            SmallInt => CassValueType::CASS_VALUE_TYPE_SMALL_INT,
            TinyInt => CassValueType::CASS_VALUE_TYPE_TINY_INT,
            Time => CassValueType::CASS_VALUE_TYPE_TIME,
            Timeuuid => CassValueType::CASS_VALUE_TYPE_TIMEUUID,
            Uuid => CassValueType::CASS_VALUE_TYPE_UUID,
            Varint => CassValueType::CASS_VALUE_TYPE_VARINT,

            // NativeType is non_exhaustive
            _ => CassValueType::CASS_VALUE_TYPE_UNKNOWN,
        }
    }
}

impl CassDataTypeInner {
    fn get_sub_data_type(&self, index: usize) -> Option<&Arc<CassDataType>> {
        match self {
            CassDataTypeInner::Udt(udt_data_type) => {
                udt_data_type.field_types.get(index).map(|(_, b)| b)
            }
            CassDataTypeInner::List { typ, .. } | CassDataTypeInner::Set { typ, .. } => {
                if index > 0 { None } else { typ.as_ref() }
            }
            CassDataTypeInner::Map {
                typ: MapDataType::Untyped,
                ..
            } => None,
            CassDataTypeInner::Map {
                typ: MapDataType::Key(k),
                ..
            } => (index == 0).then_some(k),
            CassDataTypeInner::Map {
                typ: MapDataType::KeyAndValue(k, v),
                ..
            } => match index {
                0 => Some(k),
                1 => Some(v),
                _ => None,
            },
            CassDataTypeInner::Tuple(v) => v.get(index),
            CassDataTypeInner::Vector { typ, .. } => (index == 0).then_some(typ),
            _ => None,
        }
    }

    fn add_sub_data_type(&mut self, sub_type: Arc<CassDataType>) -> Result<(), CassError> {
        match self {
            CassDataTypeInner::List { typ, .. } | CassDataTypeInner::Set { typ, .. } => match typ {
                Some(_) => Err(CassError::CASS_ERROR_LIB_BAD_PARAMS),
                None => {
                    *typ = Some(sub_type);
                    Ok(())
                }
            },
            CassDataTypeInner::Map {
                typ: MapDataType::KeyAndValue(_, _),
                ..
            } => Err(CassError::CASS_ERROR_LIB_BAD_PARAMS),
            CassDataTypeInner::Map {
                typ: MapDataType::Key(k),
                frozen,
            } => {
                *self = CassDataTypeInner::Map {
                    typ: MapDataType::KeyAndValue(k.clone(), sub_type),
                    frozen: *frozen,
                };
                Ok(())
            }
            CassDataTypeInner::Map {
                typ: MapDataType::Untyped,
                frozen,
            } => {
                *self = CassDataTypeInner::Map {
                    typ: MapDataType::Key(sub_type),
                    frozen: *frozen,
                };
                Ok(())
            }
            CassDataTypeInner::Tuple(types) => {
                types.push(sub_type);
                Ok(())
            }
            _ => Err(CassError::CASS_ERROR_LIB_INVALID_VALUE_TYPE),
        }
    }

    pub(crate) fn get_udt_type(&self) -> &UdtDataType {
        match self {
            CassDataTypeInner::Udt(udt) => udt,
            _ => panic!("Can get UDT out of non-UDT data type"),
        }
    }

    pub(crate) fn get_value_type(&self) -> CassValueType {
        match &self {
            CassDataTypeInner::Value(value_data_type) => *value_data_type,
            CassDataTypeInner::Udt { .. } => CassValueType::CASS_VALUE_TYPE_UDT,
            CassDataTypeInner::List { .. } => CassValueType::CASS_VALUE_TYPE_LIST,
            CassDataTypeInner::Set { .. } => CassValueType::CASS_VALUE_TYPE_SET,
            CassDataTypeInner::Map { .. } => CassValueType::CASS_VALUE_TYPE_MAP,
            CassDataTypeInner::Tuple(..) => CassValueType::CASS_VALUE_TYPE_TUPLE,
            CassDataTypeInner::Vector { .. } => CassValueType::CASS_VALUE_TYPE_VECTOR,
            CassDataTypeInner::Custom(..) => CassValueType::CASS_VALUE_TYPE_CUSTOM,
        }
    }
}

pub(crate) fn get_column_type(column_type: &ColumnType) -> CassDataType {
    use CollectionType::*;
    use ColumnType::*;
    let inner = match column_type {
        Native(native) => CassDataTypeInner::Value(native.into()),
        Collection {
            typ: List(boxed_type),
            frozen,
        } => CassDataTypeInner::List {
            typ: Some(Arc::new(get_column_type(boxed_type.as_ref()))),
            frozen: *frozen,
        },
        Collection {
            typ: Map(key, value),
            frozen,
        } => CassDataTypeInner::Map {
            typ: MapDataType::KeyAndValue(
                Arc::new(get_column_type(key.as_ref())),
                Arc::new(get_column_type(value.as_ref())),
            ),
            frozen: *frozen,
        },
        Collection {
            typ: Set(boxed_type),
            frozen,
        } => CassDataTypeInner::Set {
            typ: Some(Arc::new(get_column_type(boxed_type.as_ref()))),
            frozen: *frozen,
        },
        UserDefinedType { definition, frozen } => CassDataTypeInner::Udt(UdtDataType {
            field_types: definition
                .field_types
                .iter()
                .map(|(name, col_type)| {
                    (
                        name.clone().into_owned(),
                        Arc::new(get_column_type(col_type)),
                    )
                })
                .collect(),
            keyspace: definition.keyspace.clone().into_owned(),
            name: definition.name.clone().into_owned(),
            frozen: *frozen,
        }),
        Tuple(v) => CassDataTypeInner::Tuple(
            v.iter()
                .map(|col_type| Arc::new(get_column_type(col_type)))
                .collect(),
        ),
        Vector { typ, dimensions } => CassDataTypeInner::Vector {
            typ: Arc::new(get_column_type(typ.as_ref())),
            dimensions: *dimensions,
        },

        // ColumnType is non_exhaustive.
        _ => CassDataTypeInner::Value(CassValueType::CASS_VALUE_TYPE_UNKNOWN),
    };

    CassDataType::new(inner)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_new(
    value_type: CassValueType,
) -> CassOwnedSharedPtr<CassDataType, CMut> {
    let inner = match value_type {
        CassValueType::CASS_VALUE_TYPE_LIST => CassDataTypeInner::List {
            typ: None,
            frozen: false,
        },
        CassValueType::CASS_VALUE_TYPE_SET => CassDataTypeInner::Set {
            typ: None,
            frozen: false,
        },
        CassValueType::CASS_VALUE_TYPE_TUPLE => CassDataTypeInner::Tuple(Vec::new()),
        CassValueType::CASS_VALUE_TYPE_MAP => CassDataTypeInner::Map {
            typ: MapDataType::Untyped,
            frozen: false,
        },
        CassValueType::CASS_VALUE_TYPE_UDT => CassDataTypeInner::Udt(UdtDataType::new()),
        CassValueType::CASS_VALUE_TYPE_CUSTOM => CassDataTypeInner::Custom("".to_string()),
        // A vector cannot be created this way: both its element type and its number
        // of dimensions are part of the type. Use `cass_data_type_new_vector` instead.
        CassValueType::CASS_VALUE_TYPE_VECTOR => return ArcFFI::null(),
        CassValueType::CASS_VALUE_TYPE_UNKNOWN => return ArcFFI::null(),
        t if t < CassValueType::CASS_VALUE_TYPE_LAST_ENTRY => CassDataTypeInner::Value(t),
        _ => return ArcFFI::null(),
    };
    ArcFFI::into_ptr(CassDataType::new_arced(inner))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_new_from_existing(
    data_type: CassBorrowedSharedPtr<CassDataType, CConst>,
) -> CassOwnedSharedPtr<CassDataType, CMut> {
    let Some(data_type) = ArcFFI::as_ref(data_type) else {
        tracing::error!("Provided null data type pointer to cass_data_type_new_from_existing!");
        return ArcFFI::null();
    };

    ArcFFI::into_ptr(CassDataType::new_arced(
        unsafe { data_type.get_unchecked() }.clone(),
    ))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_new_tuple(
    item_count: size_t,
) -> CassOwnedSharedPtr<CassDataType, CMut> {
    ArcFFI::into_ptr(CassDataType::new_arced(CassDataTypeInner::Tuple(
        Vec::with_capacity(item_count as usize),
    )))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_new_udt(
    field_count: size_t,
) -> CassOwnedSharedPtr<CassDataType, CMut> {
    ArcFFI::into_ptr(CassDataType::new_arced(CassDataTypeInner::Udt(
        UdtDataType::with_capacity(field_count as usize),
    )))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_new_vector(
    element_type: CassBorrowedSharedPtr<CassDataType, CConst>,
    dimensions: cass_uint16_t,
) -> CassOwnedSharedPtr<CassDataType, CMut> {
    let Some(element_type) = ArcFFI::cloned_from_ptr(element_type) else {
        tracing::error!("Provided null element type pointer to cass_data_type_new_vector!");
        return ArcFFI::null();
    };

    // A missing subtype is a wildcard in `typecheck_equals()`. An element type
    // that does not name its own subtypes would therefore let this vector bind
    // to a column of any matching shape, and would make every setter skip the
    // element check. A collection can be built untyped, for compatibility with
    // cpp-driver. A vector has no such legacy form, so it does not get one.
    if !unsafe { element_type.get_unchecked() }.is_fully_typed() {
        tracing::error!("Provided a not fully typed element type to cass_data_type_new_vector!");
        return ArcFFI::null();
    }

    if dimensions == 0 {
        tracing::error!("Provided zero dimensions to cass_data_type_new_vector!");
        return ArcFFI::null();
    }

    ArcFFI::into_ptr(CassDataType::new_arced(CassDataTypeInner::Vector {
        typ: element_type,
        dimensions,
    }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_vector_dimensions(
    data_type: CassBorrowedSharedPtr<CassDataType, CConst>,
    dimensions: *mut cass_uint16_t,
) -> CassError {
    let Some(data_type) = ArcFFI::as_ref(data_type) else {
        tracing::error!("Provided null data type pointer to cass_data_type_vector_dimensions!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    match unsafe { data_type.get_unchecked() } {
        CassDataTypeInner::Vector { dimensions: d, .. } => {
            unsafe { std::ptr::write(dimensions, *d) };
            CassError::CASS_OK
        }
        _ => CassError::CASS_ERROR_LIB_INVALID_VALUE_TYPE,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_free(data_type: CassOwnedSharedPtr<CassDataType, CMut>) {
    ArcFFI::free(data_type);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_type(
    data_type: CassBorrowedSharedPtr<CassDataType, CConst>,
) -> CassValueType {
    let Some(data_type) = ArcFFI::as_ref(data_type) else {
        tracing::error!("Provided null data type pointer to cass_data_type_type!");
        return CassValueType::CASS_VALUE_TYPE_UNKNOWN;
    };

    unsafe { data_type.get_unchecked() }.get_value_type()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_is_frozen(
    data_type: CassBorrowedSharedPtr<CassDataType, CConst>,
) -> cass_bool_t {
    let Some(data_type) = ArcFFI::as_ref(data_type) else {
        tracing::error!("Provided null data type pointer to cass_data_type_is_frozen!");
        return cass_false;
    };

    let is_frozen = match unsafe { data_type.get_unchecked() } {
        CassDataTypeInner::Udt(udt) => udt.frozen,
        CassDataTypeInner::List { frozen, .. } => *frozen,
        CassDataTypeInner::Set { frozen, .. } => *frozen,
        CassDataTypeInner::Map { frozen, .. } => *frozen,
        _ => false,
    };

    is_frozen as cass_bool_t
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_type_name(
    data_type: CassBorrowedSharedPtr<CassDataType, CConst>,
    type_name: *mut *const c_char,
    type_name_length: *mut size_t,
) -> CassError {
    let Some(data_type) = ArcFFI::as_ref(data_type) else {
        tracing::error!("Provided null data type pointer to cass_data_type_type_name!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    match unsafe { data_type.get_unchecked() } {
        CassDataTypeInner::Udt(UdtDataType { name, .. }) => {
            unsafe { write_str_to_c(name, type_name, type_name_length) };
            CassError::CASS_OK
        }
        _ => CassError::CASS_ERROR_LIB_INVALID_VALUE_TYPE,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_set_type_name(
    data_type: CassBorrowedSharedPtr<CassDataType, CMut>,
    type_name: CassStrNulTerminated<'_>,
) -> CassError {
    let (type_name, type_name_length) = unsafe { type_name.as_len_delimited() };
    unsafe { cass_data_type_set_type_name_n(data_type, type_name, type_name_length) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_set_type_name_n(
    data_type_raw: CassBorrowedSharedPtr<CassDataType, CMut>,
    type_name: CassStrLenDelimited<'_>,
    type_name_length: CassStrLen,
) -> CassError {
    let Some(data_type) = ArcFFI::as_ref(data_type_raw) else {
        tracing::error!("Provided null data type pointer to cass_data_type_set_type_name_n!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    let type_name_string = match unsafe { type_name.to_str(type_name_length) } {
        Ok(s) => s.to_string(),
        Err(PtrToStrError::NullPointer) => {
            tracing::error!("Provided null type name pointer to cass_data_type_set_type_name(_n)!");
            return CassError::CASS_ERROR_LIB_BAD_PARAMS;
        }
        Err(PtrToStrError::InvalidUtf8(_)) => {
            tracing::error!("Provided non-UTF8 type name to cass_data_type_set_type_name(_n)!");
            return CassError::CASS_ERROR_LIB_BAD_PARAMS;
        }
    };

    match unsafe { data_type.get_mut_unchecked() } {
        CassDataTypeInner::Udt(udt_data_type) => {
            udt_data_type.name = type_name_string;
            CassError::CASS_OK
        }
        _ => CassError::CASS_ERROR_LIB_INVALID_VALUE_TYPE,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_keyspace(
    data_type: CassBorrowedSharedPtr<CassDataType, CConst>,
    keyspace: *mut *const c_char,
    keyspace_length: *mut size_t,
) -> CassError {
    let Some(data_type) = ArcFFI::as_ref(data_type) else {
        tracing::error!("Provided null data type pointer to cass_data_type_keyspace!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    match unsafe { data_type.get_unchecked() } {
        CassDataTypeInner::Udt(UdtDataType { name, .. }) => {
            unsafe { write_str_to_c(name, keyspace, keyspace_length) };
            CassError::CASS_OK
        }
        _ => CassError::CASS_ERROR_LIB_INVALID_VALUE_TYPE,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_set_keyspace(
    data_type: CassBorrowedSharedPtr<CassDataType, CMut>,
    keyspace: CassStrNulTerminated<'_>,
) -> CassError {
    let (keyspace, keyspace_length) = unsafe { keyspace.as_len_delimited() };
    unsafe { cass_data_type_set_keyspace_n(data_type, keyspace, keyspace_length) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_set_keyspace_n(
    data_type: CassBorrowedSharedPtr<CassDataType, CMut>,
    keyspace: CassStrLenDelimited<'_>,
    keyspace_length: CassStrLen,
) -> CassError {
    let Some(data_type) = ArcFFI::as_ref(data_type) else {
        tracing::error!("Provided null data type pointer to cass_data_type_set_keyspace_n!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    let keyspace_string = match unsafe { keyspace.to_str(keyspace_length) } {
        Ok(s) => s.to_string(),
        Err(PtrToStrError::NullPointer) => {
            tracing::error!("Provided null keyspace pointer to cass_data_type_set_keyspace(_n)!");
            return CassError::CASS_ERROR_LIB_BAD_PARAMS;
        }
        Err(PtrToStrError::InvalidUtf8(_)) => {
            tracing::error!("Provided non-UTF8 keyspace to cass_data_type_set_keyspace(_n)!");
            return CassError::CASS_ERROR_LIB_BAD_PARAMS;
        }
    };

    match unsafe { data_type.get_mut_unchecked() } {
        CassDataTypeInner::Udt(udt_data_type) => {
            udt_data_type.keyspace = keyspace_string;
            CassError::CASS_OK
        }
        _ => CassError::CASS_ERROR_LIB_INVALID_VALUE_TYPE,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_class_name(
    data_type: CassBorrowedSharedPtr<CassDataType, CConst>,
    class_name: *mut *const ::std::os::raw::c_char,
    class_name_length: *mut size_t,
) -> CassError {
    let Some(data_type) = ArcFFI::as_ref(data_type) else {
        tracing::error!("Provided null data type pointer to cass_data_type_class_name!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    match unsafe { data_type.get_unchecked() } {
        CassDataTypeInner::Custom(name) => {
            unsafe { write_str_to_c(name, class_name, class_name_length) };
            CassError::CASS_OK
        }
        _ => CassError::CASS_ERROR_LIB_INVALID_VALUE_TYPE,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_set_class_name(
    data_type: CassBorrowedSharedPtr<CassDataType, CMut>,
    class_name: CassStrNulTerminated<'_>,
) -> CassError {
    let (class_name, class_name_length) = unsafe { class_name.as_len_delimited() };
    unsafe { cass_data_type_set_class_name_n(data_type, class_name, class_name_length) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_set_class_name_n(
    data_type: CassBorrowedSharedPtr<CassDataType, CMut>,
    class_name: CassStrLenDelimited<'_>,
    class_name_length: CassStrLen,
) -> CassError {
    let Some(data_type) = ArcFFI::as_ref(data_type) else {
        tracing::error!("Provided null data type pointer to cass_data_type_set_class_name_n!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    let class_string = match unsafe { class_name.to_str(class_name_length) } {
        Ok(s) => s.to_string(),
        Err(PtrToStrError::NullPointer) => {
            tracing::error!(
                "Provided null class name pointer to cass_data_type_set_class_name(_n)!"
            );
            return CassError::CASS_ERROR_LIB_BAD_PARAMS;
        }
        Err(PtrToStrError::InvalidUtf8(_)) => {
            tracing::error!("Provided non-UTF8 class name to cass_data_type_set_class_name(_n)!");
            return CassError::CASS_ERROR_LIB_BAD_PARAMS;
        }
    };
    match unsafe { data_type.get_mut_unchecked() } {
        CassDataTypeInner::Custom(name) => {
            *name = class_string;
            CassError::CASS_OK
        }
        _ => CassError::CASS_ERROR_LIB_INVALID_VALUE_TYPE,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_sub_type_count(
    data_type: CassBorrowedSharedPtr<CassDataType, CConst>,
) -> size_t {
    let Some(data_type) = ArcFFI::as_ref(data_type) else {
        tracing::error!("Provided null data type pointer to cass_data_type_sub_type_count!");
        return 0;
    };

    match unsafe { data_type.get_unchecked() } {
        CassDataTypeInner::Value(..) => 0,
        CassDataTypeInner::Udt(udt_data_type) => udt_data_type.field_types.len() as size_t,
        CassDataTypeInner::List { typ, .. } | CassDataTypeInner::Set { typ, .. } => {
            typ.is_some() as size_t
        }
        CassDataTypeInner::Map { typ, .. } => match typ {
            MapDataType::Untyped => 0,
            MapDataType::Key(_) => 1,
            MapDataType::KeyAndValue(_, _) => 2,
        },
        CassDataTypeInner::Tuple(v) => v.len() as size_t,
        // A vector has exactly one sub type: the type of its elements.
        CassDataTypeInner::Vector { .. } => 1,
        CassDataTypeInner::Custom(..) => 0,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_sub_type_count(
    data_type: CassBorrowedSharedPtr<CassDataType, CConst>,
) -> size_t {
    unsafe { cass_data_type_sub_type_count(data_type) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_sub_data_type(
    data_type: CassBorrowedSharedPtr<CassDataType, CConst>,
    index: size_t,
) -> CassBorrowedSharedPtr<CassDataType, CConst> {
    let Some(data_type) = ArcFFI::as_ref(data_type) else {
        tracing::error!("Provided null data type pointer to cass_data_type_sub_data_type!");
        return ArcFFI::null();
    };

    let sub_type: Option<&Arc<CassDataType>> =
        unsafe { data_type.get_unchecked() }.get_sub_data_type(index as usize);

    match sub_type {
        None => ArcFFI::null(),
        // Semantic from cppdriver which also returns non-owning pointer
        Some(arc) => ArcFFI::as_ptr(arc),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_sub_data_type_by_name<'a>(
    data_type: CassBorrowedSharedPtr<'a, CassDataType, CConst>,
    name: CassStrNulTerminated<'_>,
) -> CassBorrowedSharedPtr<'a, CassDataType, CConst> {
    let (name, name_length) = unsafe { name.as_len_delimited() };
    unsafe { cass_data_type_sub_data_type_by_name_n(data_type, name, name_length) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_sub_data_type_by_name_n<'a>(
    data_type: CassBorrowedSharedPtr<'a, CassDataType, CConst>,
    name: CassStrLenDelimited<'_>,
    name_length: CassStrLen,
) -> CassBorrowedSharedPtr<'a, CassDataType, CConst> {
    let Some(data_type) = ArcFFI::as_ref(data_type) else {
        tracing::error!(
            "Provided null data type pointer to cass_data_type_sub_data_type_by_name_n!"
        );
        return ArcFFI::null();
    };

    let name_str = match unsafe { name.to_str(name_length) } {
        Ok(s) => s,
        Err(PtrToStrError::NullPointer) => {
            tracing::error!(
                "Provided null name pointer to cass_data_type_sub_data_type_by_name(_n)!"
            );
            return ArcFFI::null();
        }
        Err(PtrToStrError::InvalidUtf8(_)) => {
            tracing::error!("Provided non-UTF8 name to cass_data_type_sub_data_type_by_name(_n)!");
            return ArcFFI::null();
        }
    };
    match unsafe { data_type.get_unchecked() } {
        CassDataTypeInner::Udt(udt) => match udt.get_field_by_name(name_str) {
            None => ArcFFI::null(),
            Some(t) => ArcFFI::as_ptr(t),
        },
        _ => ArcFFI::null(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_sub_type_name(
    data_type: CassBorrowedSharedPtr<CassDataType, CConst>,
    index: size_t,
    name: *mut *const ::std::os::raw::c_char,
    name_length: *mut size_t,
) -> CassError {
    let Some(data_type) = ArcFFI::as_ref(data_type) else {
        tracing::error!("Provided null data type pointer to cass_data_type_sub_type_name!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    match unsafe { data_type.get_unchecked() } {
        CassDataTypeInner::Udt(udt) => match udt.field_types.get(index as usize) {
            None => CassError::CASS_ERROR_LIB_INDEX_OUT_OF_BOUNDS,
            Some((field_name, _)) => {
                unsafe { write_str_to_c(field_name, name, name_length) };
                CassError::CASS_OK
            }
        },
        _ => CassError::CASS_ERROR_LIB_INVALID_VALUE_TYPE,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_add_sub_type(
    data_type: CassBorrowedSharedPtr<CassDataType, CMut>,
    sub_data_type: CassBorrowedSharedPtr<CassDataType, CConst>,
) -> CassError {
    let Some(data_type) = ArcFFI::as_ref(data_type) else {
        tracing::error!("Provided null data type pointer to cass_data_type_add_sub_type!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };
    let Some(sub_data_type) = ArcFFI::cloned_from_ptr(sub_data_type) else {
        tracing::error!("Provided null sub data type pointer to cass_data_type_add_sub_type!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    match unsafe { data_type.get_mut_unchecked() }.add_sub_data_type(sub_data_type) {
        Ok(()) => CassError::CASS_OK,
        Err(e) => e,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_add_sub_type_by_name(
    data_type: CassBorrowedSharedPtr<CassDataType, CMut>,
    name: CassStrNulTerminated<'_>,
    sub_data_type: CassBorrowedSharedPtr<CassDataType, CConst>,
) -> CassError {
    let (name, name_length) = unsafe { name.as_len_delimited() };
    unsafe { cass_data_type_add_sub_type_by_name_n(data_type, name, name_length, sub_data_type) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_add_sub_type_by_name_n(
    data_type_raw: CassBorrowedSharedPtr<CassDataType, CMut>,
    name: CassStrLenDelimited<'_>,
    name_length: CassStrLen,
    sub_data_type_raw: CassBorrowedSharedPtr<CassDataType, CConst>,
) -> CassError {
    let Some(data_type) = ArcFFI::as_ref(data_type_raw) else {
        tracing::error!(
            "Provided null data type pointer to cass_data_type_add_sub_type_by_name_n!"
        );
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };
    let Some(sub_data_type) = ArcFFI::cloned_from_ptr(sub_data_type_raw) else {
        tracing::error!(
            "Provided null sub data type pointer to cass_data_type_add_sub_type_by_name_n!"
        );
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    let name_string = match unsafe { name.to_str(name_length) } {
        Ok(s) => s.to_string(),
        Err(PtrToStrError::NullPointer) => {
            tracing::error!(
                "Provided null name pointer to cass_data_type_add_sub_type_by_name(_n)!"
            );
            return CassError::CASS_ERROR_LIB_BAD_PARAMS;
        }
        Err(PtrToStrError::InvalidUtf8(_)) => {
            tracing::error!("Provided non-UTF8 name to cass_data_type_add_sub_type_by_name(_n)!");
            return CassError::CASS_ERROR_LIB_BAD_PARAMS;
        }
    };

    match unsafe { data_type.get_mut_unchecked() } {
        CassDataTypeInner::Udt(udt_data_type) => {
            // The Cpp Driver does not check whether field_types size
            // exceeded field_count.
            udt_data_type.field_types.push((name_string, sub_data_type));
            CassError::CASS_OK
        }
        _ => CassError::CASS_ERROR_LIB_INVALID_VALUE_TYPE,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_add_sub_value_type(
    data_type: CassBorrowedSharedPtr<CassDataType, CMut>,
    sub_value_type: CassValueType,
) -> CassError {
    let sub_data_type = CassDataType::new_arced(CassDataTypeInner::Value(sub_value_type));
    unsafe { cass_data_type_add_sub_type(data_type, ArcFFI::as_ptr(&sub_data_type)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_add_sub_value_type_by_name(
    data_type: CassBorrowedSharedPtr<CassDataType, CMut>,
    name: CassStrNulTerminated<'_>,
    sub_value_type: CassValueType,
) -> CassError {
    let sub_data_type = CassDataType::new_arced(CassDataTypeInner::Value(sub_value_type));
    unsafe { cass_data_type_add_sub_type_by_name(data_type, name, ArcFFI::as_ptr(&sub_data_type)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_data_type_add_sub_value_type_by_name_n(
    data_type: CassBorrowedSharedPtr<CassDataType, CMut>,
    name: *const c_char,
    name_length: size_t,
    sub_value_type: CassValueType,
) -> CassError {
    let sub_data_type = CassDataType::new_arced(CassDataTypeInner::Value(sub_value_type));
    unsafe {
        cass_data_type_add_sub_type_by_name_n(
            data_type,
            CassStrLenDelimited::from_raw(name),
            CassStrLen::from_raw(name_length),
            ArcFFI::as_ptr(&sub_data_type),
        )
    }
}

pub(crate) fn make_batch_type(type_: CassBatchType) -> Result<BatchType, ()> {
    match type_ {
        CassBatchType::CASS_BATCH_TYPE_LOGGED => Ok(BatchType::Logged),
        CassBatchType::CASS_BATCH_TYPE_UNLOGGED => Ok(BatchType::Unlogged),
        CassBatchType::CASS_BATCH_TYPE_COUNTER => Ok(BatchType::Counter),
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use scylla::cluster::metadata::{CollectionType, ColumnType, NativeType, UserDefinedType};

    use crate::testing::utils::assert_cass_error_eq;

    use super::*;

    /// The data type functions of a vector must reject a null pointer and an
    /// invalid argument, instead of dereferencing or panicking.
    #[test]
    fn test_vector_data_type_api_rejects_invalid_arguments() {
        unsafe {
            let element = cass_data_type_new(CassValueType::CASS_VALUE_TYPE_FLOAT);

            // A null element type.
            let vector_type = cass_data_type_new_vector(ArcFFI::null(), 2);
            assert!(ArcFFI::as_ref(vector_type.borrow().into_c_const()).is_none());

            // No dimensions at all.
            let vector_type = cass_data_type_new_vector(element.borrow().into_c_const(), 0);
            assert!(ArcFFI::as_ref(vector_type.borrow().into_c_const()).is_none());

            let mut dimensions: cass_uint16_t = 0;
            // A null data type.
            assert_eq!(
                cass_data_type_vector_dimensions(ArcFFI::null(), &raw mut dimensions),
                CassError::CASS_ERROR_LIB_BAD_PARAMS
            );
            // A data type that is not a vector.
            assert_eq!(
                cass_data_type_vector_dimensions(
                    element.borrow().into_c_const(),
                    &raw mut dimensions
                ),
                CassError::CASS_ERROR_LIB_INVALID_VALUE_TYPE
            );

            cass_data_type_free(element);
        }
    }

    /// A vector data type answers the generic sub-type questions too, and it
    /// does not typecheck equal to a type that is not a vector.
    #[test]
    fn test_vector_data_type_as_a_generic_data_type() {
        unsafe {
            let element = cass_data_type_new(CassValueType::CASS_VALUE_TYPE_FLOAT);
            let vector_type = cass_data_type_new_vector(element.borrow().into_c_const(), 3);

            assert_eq!(
                cass_data_type_sub_type_count(vector_type.borrow().into_c_const()),
                1
            );
            let sub = cass_data_type_sub_data_type(vector_type.borrow().into_c_const(), 0);
            assert_eq!(
                cass_data_type_type(sub),
                CassValueType::CASS_VALUE_TYPE_FLOAT
            );
            // A vector has exactly one sub-type.
            assert!(
                ArcFFI::as_ref(cass_data_type_sub_data_type(
                    vector_type.borrow().into_c_const(),
                    1
                ))
                .is_none()
            );

            // A vector never typechecks equal to a type that is not a vector.
            let vector = ArcFFI::as_ref(vector_type.borrow().into_c_const()).unwrap();
            let other = ArcFFI::as_ref(element.borrow().into_c_const()).unwrap();
            assert!(
                !vector
                    .get_unchecked()
                    .typecheck_equals(other.get_unchecked())
            );

            cass_data_type_free(vector_type);
            cass_data_type_free(element);
        }
    }

    /// A vector's element type has to name all of its own subtypes. A missing
    /// one cannot be treated as a wildcard, the way it is for a collection or
    /// a tuple, because a vector's elements are written according to their
    /// type.
    #[test]
    fn test_vector_data_type_requires_a_fully_typed_element() {
        unsafe {
            /// Builds a data type and returns whether a vector of it can be created.
            unsafe fn accepts(
                build: impl FnOnce() -> CassOwnedSharedPtr<CassDataType, CMut>,
            ) -> bool {
                unsafe {
                    let element_type = build();
                    let vector_type =
                        cass_data_type_new_vector(element_type.borrow().into_c_const(), 2);
                    let accepted = ArcFFI::as_ref(vector_type.borrow().into_c_const()).is_some();
                    cass_data_type_free(vector_type);
                    cass_data_type_free(element_type);
                    accepted
                }
            }

            /// Builds a data type of `value_type` and adds `sub_types` to it.
            unsafe fn with_sub_types(
                value_type: CassValueType,
                sub_types: &[CassValueType],
            ) -> CassOwnedSharedPtr<CassDataType, CMut> {
                unsafe {
                    let data_type = cass_data_type_new(value_type);
                    for sub_type in sub_types {
                        assert_cass_error_eq!(
                            cass_data_type_add_sub_value_type(data_type.borrow(), *sub_type),
                            CassError::CASS_OK
                        );
                    }
                    data_type
                }
            }

            use CassValueType as V;

            // Element types that name nothing, or not all of their subtypes.
            let wrongly_accepted: Vec<&str> = [
                ("list<?>", V::CASS_VALUE_TYPE_LIST),
                ("set<?>", V::CASS_VALUE_TYPE_SET),
                ("map<?, ?>", V::CASS_VALUE_TYPE_MAP),
            ]
            .into_iter()
            .filter(|(_, value_type)| accepts(|| with_sub_types(*value_type, &[])))
            .map(|(name, _)| name)
            .collect();
            assert!(
                wrongly_accepted.is_empty(),
                "wrongly accepted: {wrongly_accepted:?}"
            );

            // A map with only its key type is not fully typed either.
            assert!(!accepts(|| with_sub_types(
                V::CASS_VALUE_TYPE_MAP,
                &[V::CASS_VALUE_TYPE_INT]
            )));
            // Neither is an empty tuple or UDT.
            assert!(!accepts(|| cass_data_type_new_tuple(1)));
            assert!(!accepts(|| cass_data_type_new_udt(1)));
            // Nor a custom type. The driver cannot compare two custom types, so
            // a vector of one would abort the caller when it is typechecked. A
            // class name does not help: it describes nothing the driver can use.
            assert!(!accepts(|| cass_data_type_new(V::CASS_VALUE_TYPE_CUSTOM)));
            assert!(!accepts(|| {
                let custom = cass_data_type_new(V::CASS_VALUE_TYPE_CUSTOM);
                assert_cass_error_eq!(
                    cass_data_type_set_class_name(
                        custom.borrow(),
                        CassStrNulTerminated::from_cstr(c"org.example.Foo")
                    ),
                    CassError::CASS_OK
                );
                custom
            }));
            // Nor a fully typed collection of a not fully typed one.
            assert!(!accepts(|| {
                let inner = cass_data_type_new(V::CASS_VALUE_TYPE_LIST);
                let outer = cass_data_type_new(V::CASS_VALUE_TYPE_LIST);
                assert_cass_error_eq!(
                    cass_data_type_add_sub_type(outer.borrow(), inner.borrow().into_c_const()),
                    CassError::CASS_OK
                );
                cass_data_type_free(inner);
                outer
            }));

            // Fully typed element types are accepted.
            assert!(accepts(|| with_sub_types(
                V::CASS_VALUE_TYPE_LIST,
                &[V::CASS_VALUE_TYPE_INT]
            )));
            assert!(accepts(|| with_sub_types(
                V::CASS_VALUE_TYPE_MAP,
                &[V::CASS_VALUE_TYPE_INT, V::CASS_VALUE_TYPE_TEXT]
            )));
            assert!(accepts(|| {
                let tuple = cass_data_type_new_tuple(1);
                assert_cass_error_eq!(
                    cass_data_type_add_sub_value_type(tuple.borrow(), V::CASS_VALUE_TYPE_INT),
                    CassError::CASS_OK
                );
                tuple
            }));
            assert!(accepts(|| {
                let udt = cass_data_type_new_udt(1);
                let field = std::ffi::CString::new("f").unwrap();
                assert_cass_error_eq!(
                    cass_data_type_add_sub_value_type_by_name(
                        udt.borrow(),
                        CassStrNulTerminated::from_raw(field.as_ptr()),
                        V::CASS_VALUE_TYPE_INT
                    ),
                    CassError::CASS_OK
                );
                udt
            }));
            // A vector of vectors: the inner one is fully typed by construction.
            assert!(accepts(|| {
                let element = cass_data_type_new(V::CASS_VALUE_TYPE_FLOAT);
                let inner = cass_data_type_new_vector(element.borrow().into_c_const(), 2);
                cass_data_type_free(element);
                inner
            }));
        }
    }

    /// Builds the data type that a user gets from result metadata.
    fn from_metadata(column_type: ColumnType) -> CassDataType {
        get_column_type(&column_type)
    }

    /// Builds a data type the way a user would, with the C API.
    fn user_built(inner: CassDataTypeInner) -> CassDataType {
        CassDataType::new(inner)
    }

    fn value(value_type: CassValueType) -> Arc<CassDataType> {
        Arc::new(CassDataType::new(CassDataTypeInner::Value(value_type)))
    }

    fn typechecks(user: &CassDataType, metadata: &CassDataType) -> bool {
        unsafe {
            user.get_unchecked()
                .typecheck_equals(metadata.get_unchecked())
        }
    }

    /// `CASS_VALUE_TYPE_TEXT` and `CASS_VALUE_TYPE_VARCHAR` denote the same CQL
    /// type, so they have to typecheck against each other. Result metadata only
    /// ever uses the latter.
    #[test]
    fn test_text_and_varchar_typecheck_equal() {
        let text = user_built(CassDataTypeInner::Value(
            CassValueType::CASS_VALUE_TYPE_TEXT,
        ));
        let varchar = from_metadata(ColumnType::Native(NativeType::Text));

        assert!(typechecks(&text, &varchar));
        assert!(typechecks(&varchar, &text));
    }

    /// Other value types must keep comparing unequal.
    #[test]
    fn test_distinct_value_types_do_not_typecheck_equal() {
        for (t1, t2) in [
            (
                CassValueType::CASS_VALUE_TYPE_TEXT,
                CassValueType::CASS_VALUE_TYPE_ASCII,
            ),
            (
                CassValueType::CASS_VALUE_TYPE_VARCHAR,
                CassValueType::CASS_VALUE_TYPE_BLOB,
            ),
            (
                CassValueType::CASS_VALUE_TYPE_INT,
                CassValueType::CASS_VALUE_TYPE_BIGINT,
            ),
        ] {
            let d1 = user_built(CassDataTypeInner::Value(t1));
            let d2 = user_built(CassDataTypeInner::Value(t2));
            assert!(!typechecks(&d1, &d2), "{t1:?} must not equal {t2:?}");
            assert!(!typechecks(&d2, &d1), "{t2:?} must not equal {t1:?}");
        }
    }

    /// The same has to hold for a `text` nested in a container. A container
    /// built with `cass_*_new_from_data_type()` is fully typed, so - unlike an
    /// untyped one - its subtypes are really compared.
    #[test]
    fn test_text_typechecks_inside_containers() {
        let text = || value(CassValueType::CASS_VALUE_TYPE_TEXT);
        let metadata_text = || ColumnType::Native(NativeType::Text);

        let cases: Vec<(&str, CassDataType, CassDataType)> = vec![
            (
                "list<text>",
                user_built(CassDataTypeInner::List {
                    typ: Some(text()),
                    frozen: false,
                }),
                from_metadata(ColumnType::Collection {
                    frozen: false,
                    typ: CollectionType::List(Box::new(metadata_text())),
                }),
            ),
            (
                "set<text>",
                user_built(CassDataTypeInner::Set {
                    typ: Some(text()),
                    frozen: false,
                }),
                from_metadata(ColumnType::Collection {
                    frozen: false,
                    typ: CollectionType::Set(Box::new(metadata_text())),
                }),
            ),
            (
                "map<text, text>",
                user_built(CassDataTypeInner::Map {
                    typ: MapDataType::KeyAndValue(text(), text()),
                    frozen: false,
                }),
                from_metadata(ColumnType::Collection {
                    frozen: false,
                    typ: CollectionType::Map(Box::new(metadata_text()), Box::new(metadata_text())),
                }),
            ),
            (
                "tuple<text>",
                user_built(CassDataTypeInner::Tuple(vec![text()])),
                from_metadata(ColumnType::Tuple(vec![metadata_text()])),
            ),
            (
                "udt with a text field",
                user_built(CassDataTypeInner::Udt(UdtDataType {
                    field_types: vec![("field".to_owned(), text())],
                    keyspace: "ks".to_owned(),
                    name: "udt".to_owned(),
                    frozen: false,
                })),
                from_metadata(ColumnType::UserDefinedType {
                    frozen: false,
                    definition: Arc::new(UserDefinedType {
                        name: "udt".into(),
                        keyspace: "ks".into(),
                        field_types: vec![("field".into(), metadata_text())],
                    }),
                }),
            ),
        ];

        // Check every shape, so that a failure names all of the affected ones.
        let failed: Vec<&str> = cases
            .iter()
            .filter(|(_, user, metadata)| !typechecks(user, metadata))
            .map(|(name, _, _)| *name)
            .collect();
        assert!(failed.is_empty(), "failed to typecheck: {failed:?}");
    }
}
