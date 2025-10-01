use crate::types::size_t;
use std::cmp::min;
use std::ffi::CStr;
use std::marker::PhantomData;
use std::os::raw::c_char;
use std::ptr::NonNull;
use std::sync::{Arc, Weak};

pub unsafe fn ptr_to_cstr(ptr: *const c_char) -> Option<&'static str> {
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

pub unsafe fn ptr_to_cstr_n(ptr: *const c_char, size: size_t) -> Option<&'static str> {
    if ptr.is_null() {
        return None;
    }
    std::str::from_utf8(unsafe { std::slice::from_raw_parts(ptr as *const u8, size as usize) }).ok()
}

pub(crate) unsafe fn arr_to_cstr<const N: usize>(arr: &[c_char]) -> Option<&'static str> {
    let null_char = '\0' as c_char;
    let end_index = arr[..N].iter().position(|c| c == &null_char).unwrap_or(N);
    unsafe { ptr_to_cstr_n(arr.as_ptr(), end_index as size_t) }
}

pub(crate) fn str_to_arr<const N: usize>(s: &str) -> [c_char; N] {
    let mut result = ['\0' as c_char; N];

    // Max length must be null-terminated
    let mut max_len = min(N - 1, s.len());

    while !s.is_char_boundary(max_len) {
        max_len -= 1;
    }

    for (i, c) in s.as_bytes().iter().enumerate().take(max_len) {
        result[i] = *c as c_char;
    }

    result
}

pub(crate) unsafe fn write_str_to_c(s: &str, c_str: *mut *const c_char, c_strlen: *mut size_t) {
    unsafe {
        *c_str = s.as_ptr() as *const c_char;
        *c_strlen = s.len() as u64;
    }
}

pub(crate) unsafe fn strlen(ptr: *const c_char) -> size_t {
    if ptr.is_null() {
        return 0;
    }
    unsafe { libc::strlen(ptr) as size_t }
}

#[cfg(test)]
pub(crate) fn str_to_c_str_n(s: &str) -> (*const c_char, size_t) {
    let mut c_str = std::ptr::null();
    let mut c_strlen = size_t::default();

    // SAFETY: The pointers that are passed to `write_str_to_c` are compile-checked references.
    unsafe { write_str_to_c(s, &mut c_str, &mut c_strlen) };

    (c_str, c_strlen)
}

#[cfg(test)]
macro_rules! make_c_str {
    ($str:literal) => {
        concat!($str, "\0").as_ptr() as *const c_char
    };
}

#[cfg(test)]
pub(crate) use make_c_str;

mod sealed {
    // This is a sealed trait - its whole purpose is to be unnameable.
    // This means we need to disable the check.
    #[expect(unnameable_types)]
    pub trait Sealed {}
}

/// A trait representing ownership (i.e. Rust mutability) of the pointer.
///
/// Pointer can either be [`Exclusive`] or [`Shared`].
///
/// ## Shared pointers
/// Shared pointers can only be converted to **immutable** Rust referential types.
/// There is no way to obtain a mutable reference from such pointer.
///
/// In some cases, we need to be able to mutate the data behind a shared pointer.
/// There is an example of such use case - namely [`crate::cql_types::data_type::CassDataType`].
/// argconv API does not provide a way to mutate such pointer - one can only convert the pointer
/// to [`Arc`] or &. It is the API user's responsibility to implement sound interior mutability
/// pattern in such case. This is what we currently do - CassDataType wraps CassDataTypeInner
/// inside an `UnsafeCell` to implement interior mutability for the type.
/// Other example is [`crate::future::CassFuture`] which uses Mutex.
///
/// ## Exclusive pointers
/// Exclusive pointers can be converted to both immutable and mutable Rust referential types.
pub trait Ownership: sealed::Sealed {}

/// Represents shared (immutable) pointer.
pub struct Shared;
impl sealed::Sealed for Shared {}
impl Ownership for Shared {}

/// Represents exclusive (mutable) pointer.
pub struct Exclusive;
impl sealed::Sealed for Exclusive {}
impl Ownership for Exclusive {}

/// A trait representing mutability of the pointer in C semantics.
///
/// Pointer can be either [`CConst`] or [`CMut`].
///
/// The semantics of this property are very simple: it tells whether
/// the pointer is const or not on the C side. If the pointer is `T*`, then
/// corresponding [`CassPtr`] is [`CMut`], and if the pointer is `const T*`, then
/// corresponding [`CassPtr`] is [`CConst`].
///
/// ## Why [`Ownership`] property is not enough?
/// This is because mutability semantics differ between C and Rust.
/// - In C/C++: mut = logically mutable, const = logically immutable
/// - In Rust: mut = exclusive ownership (e.g. Box, &mut), const = shared ownership (e.g. Arc, &)
///
/// In cpp-driver API there are some types that are both shared and mutable. Take for example `CassDataType`.
/// From rust perspective, the pointer is always [`Shared`], because it is shared behind an [`Arc`].
/// But there are some methods, e.g:
/// ```rust,ignore
/// cass_data_type_add_sub_type(CassDataType* data_type, const CassDataType* sub_data_type);
/// ```
/// which accept a mutable pointer. Such method is implemented by employing interior mutability pattern (mutating a shared object).
/// If we did not have [`CMutability`] property defined, we would not be able to
/// distinguish between `CassDataType*` and `const CassDataType*`. In such case, user would be able to obtain
/// `const CassDataType*` pointer (e.g. from `cass_tuple_data_type`) and pass it to the method that expects `CassDataType*`.
///
/// This property comes useful for implementing safe Rust unit tests where we play a role of C API user.
pub trait CMutability: sealed::Sealed {}

/// Represents const C pointer.
pub struct CConst;
impl sealed::Sealed for CConst {}
impl CMutability for CConst {}

/// Represents mutable (non-const) C pointer.
pub struct CMut;
impl sealed::Sealed for CMut {}
impl CMutability for CMut {}

/// Represents additional properties of the pointer.
pub trait Properties: sealed::Sealed {
    type Onwership: Ownership;
    type CMutability: CMutability;
}

impl<O: Ownership, CM: CMutability> sealed::Sealed for (O, CM) {}
impl<O: Ownership, CM: CMutability> Properties for (O, CM) {
    type Onwership = O;
    type CMutability = CM;
}

/// Represents a valid non-dangling pointer.
///
/// ## Safety and validity guarantees
/// Apart from trivial constructors such as [`CassPtr::null()`] and [`CassPtr::null_mut()`], there
/// is only one way to construct a [`CassPtr`] instance - from raw pointer via [`CassPtr::from_raw()`].
/// This constructor is `unsafe`. It is user's responsibility to ensure that the raw pointer
/// provided to the constructor is **valid**. In other words, the pointer comes from some valid
/// allocation, or from some valid reference.
///
/// ## Generic lifetime and aliasing guarantees
/// We distinguish two types of pointers: shared ([`Shared`]) and exclusive ([`Exclusive`]).
/// Shared pointers can be converted to immutable (&) references, while exclusive pointers
/// can be converted to either immutable (&) or mutable (&mut) reference. User needs to pick
/// the correct mutability property of the pointer during construction. This is yet another
/// reason why [`CassPtr::from_raw`] is `unsafe`.
///
/// Pointer is parameterized by the lifetime. Thanks to that, we can tell whether the pointer
/// **owns** or **borrows** the pointee. Once again, user is responsible for "picking"
/// the correct lifetime when creating the pointer. For example, when raw pointer
/// comes from [`Box::into_raw()`], user could create a [`CassPtr<'static, T, (Exclusive,)>`].
/// `'static` lifetime represents that user is the exclusive **owner** of the pointee, and
/// is responsible for freeing the memory (e.g. via [`Box::from_raw()`]).
/// On the other hand, when pointer is created from some immutable reference `&'a T`,
/// the correct choice of CassPtr would be [`CassPtr<'a, T, (Shared,)>`]. It means that
/// holder of the created pointer **borrows** the pointee (with some lifetime `'a`
/// inherited from the immutable borrow `&'a T`).
///
/// Both [`CassPtr::into_ref()`] and [`CassPtr::into_mut_ref()`] consume the pointer.
/// At first glance, it seems impossible to obtain multiple immutable reference from one pointer.
/// This is why pointer reborrowing mechanism is introduced. There are two methods: [`CassPtr::borrow()`]
/// and [`CassPtr::borrow_mut()`]. Both of them cooperate with borrow checker and enforce
/// aliasing XOR mutability principle at compile time.
///
/// ## Safe conversions to referential types
/// Thanks to the above guarantees, conversions to referential types are **safe**.
/// See methods [`CassPtr::into_ref()`] and [`CassPtr::into_mut_ref()`].
///
/// ## Memory layout
/// We use repr(transparent), so the struct has the same layout as underlying [`Option<NonNull<T>>`].
/// Thanks to <https://doc.rust-lang.org/std/option/#representation optimization>,
/// we are guaranteed, that for `T: Sized`, our struct has the same layout
/// and function call ABI as simply [`NonNull<T>`].
#[repr(transparent)]
pub struct CassPtr<'a, T: Sized, P: Properties> {
    ptr: Option<NonNull<T>>,
    _phantom: PhantomData<&'a P>,
}

/// Owned shared pointer.
/// Can be used for pointers with shared ownership - e.g. pointers coming from [`Arc`] allocation.
pub type CassOwnedSharedPtr<T, CM> = CassPtr<'static, T, (Shared, CM)>;

/// Borrowed shared pointer.
/// Can be used for pointers created from some immutable reference.
pub type CassBorrowedSharedPtr<'a, T, CM> = CassPtr<'a, T, (Shared, CM)>;

/// Owned exclusive pointer.
/// Can be used for pointers with exclusive ownership - e.g. pointers coming from [`Box`] allocation.
pub type CassOwnedExclusivePtr<T, CM> = CassPtr<'static, T, (Exclusive, CM)>;

/// Borrowed exclusive pointer.
/// This can be for example obtained from mutable reborrow of some [`CassOwnedExclusivePtr`].
pub type CassBorrowedExclusivePtr<'a, T, CM> = CassPtr<'a, T, (Exclusive, CM)>;

/// Utility method for tests. Useful when some method returns `T*`,
/// and then another method accepts `const T*`.
impl<'a, T: Sized, P: Properties> CassPtr<'a, T, P> {
    pub fn into_c_const(self) -> CassPtr<'a, T, (P::Onwership, CConst)> {
        CassPtr {
            ptr: self.ptr,
            _phantom: PhantomData,
        }
    }
}

/// Pointer constructors.
impl<T: Sized, P: Properties> CassPtr<'_, T, P> {
    pub fn null() -> Self {
        CassPtr {
            ptr: None,
            _phantom: PhantomData,
        }
    }

    pub fn is_null(&self) -> bool {
        self.ptr.is_none()
    }

    /// Constructs [`CassPtr`] from raw pointer.
    ///
    /// ## Safety
    /// User needs to ensure that the pointer is **valid**.
    /// User is also responsible for picking correct ownership property and lifetime
    /// of the created pointer. For more information, see the documentation of [`CassPtr`].
    unsafe fn from_raw(raw: *const T) -> Self {
        CassPtr {
            ptr: NonNull::new(raw as *mut T),
            _phantom: PhantomData,
        }
    }
}

/// Conversion to raw pointer.
impl<T: Sized, P: Properties> CassPtr<'_, T, P> {
    fn to_raw(&self) -> Option<*mut T> {
        self.ptr.map(|ptr| ptr.as_ptr())
    }
}

/// Constructors for to exclusive pointers.
impl<T: Sized> CassPtr<'_, T, (Exclusive, CMut)> {
    pub(crate) fn null_mut() -> Self {
        CassPtr {
            ptr: None,
            _phantom: PhantomData,
        }
    }
}

impl<'a, T: Sized, P: Properties> CassPtr<'a, T, P> {
    /// Converts a pointer to an optional valid reference.
    /// The reference inherits the lifetime of the pointer.
    fn into_ref(self) -> Option<&'a T> {
        // SAFETY: Thanks to the validity and aliasing ^ mutability guarantees,
        // we can safely convert the pointer to valid immutable reference with
        // correct lifetime.
        unsafe { self.ptr.map(|p| p.as_ref()) }
    }
}

impl<'a, T: Sized, CM: CMutability> CassPtr<'a, T, (Exclusive, CM)> {
    /// Converts a pointer to an optional valid mutable reference.
    /// The reference inherits the lifetime of the pointer.
    fn into_mut_ref(self) -> Option<&'a mut T> {
        // SAFETY: Thanks to the validity and aliasing ^ mutability guarantees,
        // we can safely convert the pointer to valid mutable (and exclusive) reference with
        // correct lifetime.
        unsafe { self.ptr.map(|mut p| p.as_mut()) }
    }
}

impl<T: Sized, P: Properties> CassPtr<'_, T, P> {
    /// Immutably reborrows the pointer.
    /// Resulting pointer inherits the lifetime from the immutable borrow
    /// of original pointer.
    #[allow(clippy::needless_lifetimes)]
    pub fn borrow<'a>(&'a self) -> CassPtr<'a, T, (Shared, P::CMutability)> {
        CassPtr {
            ptr: self.ptr,
            _phantom: PhantomData,
        }
    }
}

impl<T: Sized> CassPtr<'_, T, (Exclusive, CMut)> {
    /// Mutably reborrows the pointer.
    /// Resulting pointer inherits the lifetime from the mutable borrow
    /// of original pointer. Since the method accepts a mutable reference
    /// to the original pointer, we enforce aliasing ^ mutability principle at compile time.
    #[allow(clippy::needless_lifetimes)]
    pub fn borrow_mut<'a>(&'a mut self) -> CassPtr<'a, T, (Exclusive, CMut)> {
        CassPtr {
            ptr: self.ptr,
            _phantom: PhantomData,
        }
    }
}

mod origin_sealed {
    // This is a sealed trait - its whole purpose is to be unnameable.
    // This means we need to disable the check.
    #[expect(unnameable_types)]
    pub trait FromBoxSealed {}

    // This is a sealed trait - its whole purpose is to be unnameable.
    // This means we need to disable the check.
    #[expect(unnameable_types)]
    pub trait FromArcSealed {}

    // This is a sealed trait - its whole purpose is to be unnameable.
    // This means we need to disable the check.
    #[expect(unnameable_types)]
    pub trait FromRefSealed {}
}

/// Defines a pointer manipulation API for non-shared heap-allocated data.
///
/// Implement this trait for types that are allocated by the driver via [`Box::new`],
/// and then returned to the user as a pointer. The user is responsible for freeing
/// the memory associated with the pointer using corresponding driver's API function.
pub trait BoxFFI: Sized + origin_sealed::FromBoxSealed {
    /// Consumes the Box and returns a pointer with exclusive ownership.
    /// The pointer needs to be freed. See [`BoxFFI::free()`].
    fn into_ptr<CM: CMutability>(self: Box<Self>) -> CassPtr<'static, Self, (Exclusive, CM)> {
        #[allow(clippy::disallowed_methods)]
        let ptr = Box::into_raw(self);

        // SAFETY:
        // 1. validity guarantee - pointer is obviously valid. It comes from box allocation.
        // 2. pointer's lifetime - we choose 'static lifetime. It is ok, because holder of the
        //    pointer becomes the owner of pointee. He is responsible for freeing the memory
        //    via BoxFFI::free() - which accepts 'static pointer. User is not able to obtain
        //    another pointer with 'static lifetime pointing to the same memory.
        // 3. ownership - user becomes an exclusive owner of the pointee. Thus, it's ok
        //    for the pointer to be `Exclusive`.
        unsafe { CassPtr::from_raw(ptr) }
    }

    /// Consumes the pointer with exclusive ownership back to the Box.
    fn from_ptr<CM: CMutability>(
        ptr: CassPtr<'static, Self, (Exclusive, CM)>,
    ) -> Option<Box<Self>> {
        // SAFETY:
        // The only way to obtain an owned pointer (with 'static lifetime) is BoxFFI::into_ptr().
        // It creates a pointer based on Box allocation. It is thus safe to convert the pointer
        // back to owned `Box`.
        unsafe {
            ptr.to_raw().map(|p| {
                #[allow(clippy::disallowed_methods)]
                Box::from_raw(p)
            })
        }
    }

    /// Creates a reference from an exclusive pointer.
    /// Reference inherits the lifetime of the pointer's borrow.
    #[allow(clippy::needless_lifetimes)]
    fn as_ref<'a, O: Ownership, CM: CMutability>(
        ptr: CassPtr<'a, Self, (O, CM)>,
    ) -> Option<&'a Self> {
        ptr.into_ref()
    }

    /// Creates a mutable from an exlusive pointer.
    /// Reference inherits the lifetime of the pointer's mutable borrow.
    #[allow(clippy::needless_lifetimes)]
    fn as_mut_ref<'a, CM: CMutability>(
        ptr: CassPtr<'a, Self, (Exclusive, CM)>,
    ) -> Option<&'a mut Self> {
        ptr.into_mut_ref()
    }

    /// Frees the pointee.
    fn free<CM: CMutability>(ptr: CassPtr<'static, Self, (Exclusive, CM)>) {
        std::mem::drop(BoxFFI::from_ptr(ptr));
    }

    // Currently used only in tests.
    #[allow(dead_code)]
    fn null<'a, CM: CMutability>() -> CassPtr<'a, Self, (Shared, CConst)> {
        CassPtr::null()
    }

    fn null_mut<'a>() -> CassPtr<'a, Self, (Exclusive, CMut)> {
        CassPtr::null_mut()
    }
}

/// Defines a pointer manipulation API for shared heap-allocated data.
///
/// Implement this trait for types that require a shared ownership of data.
/// The data should be allocated via [`Arc::new`], and then returned to the user as a pointer.
/// The user is responsible for freeing the memory associated
/// with the pointer using corresponding driver's API function.
pub trait ArcFFI: Sized + origin_sealed::FromArcSealed {
    /// Creates a pointer from a valid reference to Arc-allocated data.
    /// Holder of the pointer borrows the pointee.
    #[allow(clippy::needless_lifetimes)]
    fn as_ptr<'a, CM: CMutability>(self: &'a Arc<Self>) -> CassPtr<'a, Self, (Shared, CM)> {
        #[allow(clippy::disallowed_methods)]
        let ptr = Arc::as_ptr(self);

        // SAFETY:
        // 1. validity guarantee - pointer is valid, since it's obtained from Arc allocation.
        // 2. pointer's lifetime - pointer inherits the lifetime of provided Arc's borrow.
        //    What's important is that the returned pointer borrows the data, and is not the
        //    shared owner. Thus, user cannot call ArcFFI::free() on such pointer.
        // 3. ownership - we always create a `Shared` pointer.
        unsafe { CassPtr::from_raw(ptr) }
    }

    /// Creates a pointer from a valid Arc allocation.
    fn into_ptr<CM: CMutability>(self: Arc<Self>) -> CassPtr<'static, Self, (Shared, CM)> {
        #[allow(clippy::disallowed_methods)]
        let ptr = Arc::into_raw(self);

        // SAFETY:
        // 1. validity guarantee - pointer is valid, since it's obtained from Arc allocation
        // 2. pointer's lifetime - returned pointer has a 'static lifetime. It is a shared
        //    owner of the pointee. User has to decrement the RC of the pointer (and potentially free the memory)
        //    via ArcFFI::free().
        // 3. ownership - we always create a `Shared` pointer.
        unsafe { CassPtr::from_raw(ptr) }
    }

    /// Converts shared owned pointer back to owned Arc.
    fn from_ptr<CM: CMutability>(ptr: CassPtr<'static, Self, (Shared, CM)>) -> Option<Arc<Self>> {
        // SAFETY:
        // The only way to obtain a pointer with shared ownership ('static lifetime) is
        // ArcFFI::into_ptr(). It converts an owned Arc into the pointer. It is thus safe
        // to convert such pointer back to owned Arc.
        unsafe {
            ptr.to_raw().map(|p| {
                #[allow(clippy::disallowed_methods)]
                Arc::from_raw(p)
            })
        }
    }

    /// Increases the reference count of the pointer, and returns an owned Arc.
    fn cloned_from_ptr<CM: CMutability>(ptr: CassPtr<'_, Self, (Shared, CM)>) -> Option<Arc<Self>> {
        // SAFETY:
        // All pointers created via ArcFFI API are originated from Arc allocation.
        // It is thus safe to increase the reference count of the pointer, and convert
        // it to Arc. Because of the borrow-checker, it is not possible for the user
        // to provide a pointer that points to already deallocated memory.
        unsafe {
            ptr.to_raw().map(|p| {
                #[allow(clippy::disallowed_methods)]
                Arc::increment_strong_count(p);
                #[allow(clippy::disallowed_methods)]
                Arc::from_raw(p)
            })
        }
    }

    /// Converts a shared borrowed pointer to reference.
    /// The reference inherits the lifetime of pointer's borrow.
    #[allow(clippy::needless_lifetimes)]
    fn as_ref<'a, CM: CMutability>(ptr: CassPtr<'a, Self, (Shared, CM)>) -> Option<&'a Self> {
        ptr.into_ref()
    }

    /// Decreases the reference count (and potentially frees) of the owned pointer.
    fn free<CM: CMutability>(ptr: CassPtr<'static, Self, (Shared, CM)>) {
        std::mem::drop(ArcFFI::from_ptr(ptr));
    }

    fn null<'a, CM: CMutability>() -> CassPtr<'a, Self, (Shared, CM)> {
        CassPtr::null()
    }

    fn is_null<CM: CMutability>(ptr: &CassPtr<'_, Self, (Shared, CM)>) -> bool {
        ptr.is_null()
    }
}

/// Defines a pointer manipulation API for data owned by some other object.
///
/// Implement this trait for the types that do not need to be freed (directly) by the user.
/// The lifetime of the data is bound to some other object owning it.
///
/// For example: lifetime of CassRow is bound by the lifetime of CassResult.
/// There is no API function that frees the CassRow. It should be automatically
/// freed when user calls cass_result_free.
pub trait RefFFI: Sized + origin_sealed::FromRefSealed {
    /// Creates a borrowed pointer from a valid reference.
    #[allow(clippy::needless_lifetimes)]
    fn as_ptr<'a, CM: CMutability>(&'a self) -> CassPtr<'a, Self, (Shared, CM)> {
        // SAFETY:
        // 1. validity guarantee - pointer is valid, since it's obtained a valid reference.
        // 2. pointer's lifetime - pointer inherits the lifetime of provided reference's borrow.
        // 3. ownerhsip - we always create a `Shared` pointer.
        unsafe { CassPtr::from_raw(self) }
    }

    /// Creates a borrowed pointer from a weak reference.
    ///
    /// ## SAFETY
    /// User needs to ensure that the pointee is not freed when pointer is being
    /// dereferenced.
    ///
    /// ## Why this method is unsafe? - Example
    /// ```
    /// # use scylla_cpp_driver::argconv::{CConst, CassBorrowedSharedPtr};
    /// # use scylla_cpp_driver::argconv::{FFI, FromRef, RefFFI};
    /// # use std::sync::{Arc, Weak};
    ///
    /// struct Foo;
    /// impl FFI for Foo {
    ///     type Origin = FromRef;
    /// }
    ///
    /// let arc = Arc::new(Foo);
    /// let weak = Arc::downgrade(&arc);
    /// let ptr: CassBorrowedSharedPtr<Foo, CConst> = unsafe { RefFFI::weak_as_ptr(&weak) };
    /// std::mem::drop(arc);
    ///
    /// // The ptr is now dangling. The user can "safely" dereference it using RefFFI API.
    ///
    /// ```
    #[allow(clippy::needless_lifetimes)]
    unsafe fn weak_as_ptr<'a, CM: CMutability>(
        w: &'a Weak<Self>,
    ) -> CassPtr<'a, Self, (Shared, CM)> {
        match w.upgrade() {
            Some(a) => {
                #[allow(clippy::disallowed_methods)]
                let ptr = Arc::as_ptr(&a);
                unsafe { CassPtr::from_raw(ptr) }
            }
            None => CassPtr::null(),
        }
    }

    /// Converts a borrowed pointer to reference.
    /// The reference inherits the lifetime of pointer's borrow.
    #[allow(clippy::needless_lifetimes)]
    fn as_ref<'a, CM: CMutability>(ptr: CassPtr<'a, Self, (Shared, CM)>) -> Option<&'a Self> {
        ptr.into_ref()
    }

    fn null<'a, CM: CMutability>() -> CassPtr<'a, Self, (Shared, CM)> {
        CassPtr::null()
    }

    fn is_null<CM: CMutability>(ptr: &CassPtr<'_, Self, (Shared, CM)>) -> bool {
        ptr.is_null()
    }
}

/// This trait should be implemented for types that are passed between
/// C and Rust API. We currently distinguish 3 kinds of implementors,
/// wrt. the origin of the pointer. The implementor should pick one of the 3 ownership
/// kinds as the associated type:
/// - [`FromBox`]
/// - [`FromArc`]
/// - [`FromRef`]
#[allow(clippy::upper_case_acronyms)]
pub trait FFI {
    type Origin;
}

/// Represents types with an exclusive ownership.
///
/// Use this associated type for implementors that require:
/// - owned exclusive pointer manipulation via [`BoxFFI`]
/// - exclusive ownership of the corresponding object
/// - potential mutability of the corresponding object
/// - manual memory freeing
///
/// C API user should be responsible for freeing associated memory manually
/// via corresponding API call.
///
/// An example of such implementor would be [`CassCluster`](crate::cluster::CassCluster):
/// - it is allocated on the heap via [`Box::new`]
/// - user is the exclusive owner of the CassCluster object
/// - there is no API to increase a reference count of CassCluster object
/// - CassCluster is mutable via some API methods (`cass_cluster_set_*`)
/// - user is responsible for freeing the associated memory (`cass_cluster_free`)
pub struct FromBox;
impl<T> origin_sealed::FromBoxSealed for T where T: FFI<Origin = FromBox> {}
impl<T> BoxFFI for T where T: FFI<Origin = FromBox> {}

/// Represents types with a shared ownership.
///
/// Use this associated type for implementors that require:
/// - pointer with shared ownership manipulation via [`ArcFFI`]
/// - shared ownership of the corresponding object
/// - manual memory freeing
///
/// C API user should be responsible for freeing (decreasing reference count of)
/// associated memory manually via corresponding API call.
///
/// An example of such implementor would be [`CassDataType`](crate::cql_types::data_type::CassDataType):
/// - it is allocated on the heap via [`Arc::new`]
/// - there are multiple owners of the shared CassDataType object
/// - some API functions require to increase a reference count of the object
/// - user is responsible for freeing (decreasing RC of) the associated memory (`cass_data_type_free`)
pub struct FromArc;
impl<T> origin_sealed::FromArcSealed for T where T: FFI<Origin = FromArc> {}
impl<T> ArcFFI for T where T: FFI<Origin = FromArc> {}

/// Represents borrowed types.
///
/// Use this associated type for implementors that do not require any assumptions
/// about the pointer type (apart from validity).
/// The implementation will enable [`CassBorrowedPtr`] manipulation via [`RefFFI`]
///
/// C API user is not responsible for freeing associated memory manually. The memory
/// should be freed automatically, when the owner is being dropped.
///
/// An example of such implementor would be [`CassRow`](crate::query_result::CassRow):
/// - its lifetime is tied to the lifetime of CassResult
/// - user only "borrows" the pointer - he is not responsible for freeing the memory
pub struct FromRef;
impl<T> origin_sealed::FromRefSealed for T where T: FFI<Origin = FromRef> {}
impl<T> RefFFI for T where T: FFI<Origin = FromRef> {}

/// ```compile_fail,E0499
/// # use scylla_cpp_driver::argconv::{CassOwnedExclusivePtr, CassBorrowedExclusivePtr, CMut};
/// # use scylla_cpp_driver::argconv::{FFI, BoxFFI, FromBox};
/// struct Foo;
/// impl FFI for Foo {
///     type Origin = FromBox;
/// }
///
/// let mut ptr: CassOwnedExclusivePtr<Foo, CMut> = BoxFFI::into_ptr(Box::new(Foo));
/// let borrowed_mut_ptr1: CassBorrowedExclusivePtr<Foo, CMut> = ptr.borrow_mut();
/// let borrowed_mut_ptr2: CassBorrowedExclusivePtr<Foo, CMut> = ptr.borrow_mut();
/// let mutref1 = BoxFFI::as_mut_ref(borrowed_mut_ptr2);
/// let mutref2 = BoxFFI::as_mut_ref(borrowed_mut_ptr1);
/// ```
fn _test_box_ffi_cannot_have_two_mutable_references() {}

/// ```compile_fail,E0502
/// # use scylla_cpp_driver::argconv::{CassOwnedExclusivePtr, CassBorrowedSharedPtr, CassBorrowedExclusivePtr, CConst, CMut};
/// # use scylla_cpp_driver::argconv::{FFI, BoxFFI, FromBox};
/// struct Foo;
/// impl FFI for Foo {
///     type Origin = FromBox;
/// }
///
/// let mut ptr: CassOwnedExclusivePtr<Foo, CMut> = BoxFFI::into_ptr(Box::new(Foo));
/// let borrowed_mut_ptr: CassBorrowedExclusivePtr<Foo, CMut> = ptr.borrow_mut();
/// let borrowed_ptr: CassBorrowedSharedPtr<Foo, CMut> = ptr.borrow();
/// let immref = BoxFFI::as_ref(borrowed_ptr);
/// let mutref = BoxFFI::as_mut_ref(borrowed_mut_ptr);
/// ```
fn _test_box_ffi_cannot_have_mutable_and_immutable_references_at_the_same_time() {}

/// ```compile_fail,E0505
/// # use scylla_cpp_driver::argconv::{CassOwnedExclusivePtr, CassBorrowedSharedPtr, CMut};
/// # use scylla_cpp_driver::argconv::{FFI, BoxFFI, FromBox};
/// struct Foo;
/// impl FFI for Foo {
///     type Origin = FromBox;
/// }
///
/// let ptr: CassOwnedExclusivePtr<Foo, CMut> = BoxFFI::into_ptr(Box::new(Foo));
/// let borrowed_ptr: CassBorrowedSharedPtr<Foo, CMut> = ptr.borrow();
/// BoxFFI::free(ptr);
/// let immref = BoxFFI::as_ref(borrowed_ptr);
/// ```
fn _test_box_ffi_cannot_free_while_having_borrowed_pointer() {}

/// ```compile_fail,E0505
/// # use scylla_cpp_driver::argconv::{CassOwnedSharedPtr, CassBorrowedSharedPtr, CConst};
/// # use scylla_cpp_driver::argconv::{FFI, ArcFFI, FromArc};
/// # use std::sync::Arc;
/// struct Foo;
/// impl FFI for Foo {
///     type Origin = FromArc;
/// }
///
/// let ptr: CassOwnedSharedPtr<Foo, CConst> = ArcFFI::into_ptr(Arc::new(Foo));
/// let borrowed_ptr: CassBorrowedSharedPtr<Foo, CConst> = ptr.borrow();
/// ArcFFI::free(ptr);
/// let immref = ArcFFI::cloned_from_ptr(borrowed_ptr);
/// ```
fn _test_arc_ffi_cannot_clone_after_free() {}

/// ```compile_fail,E0505
/// # use scylla_cpp_driver::argconv::{CassBorrowedSharedPtr, CConst};
/// # use scylla_cpp_driver::argconv::{FFI, ArcFFI, FromArc};
/// # use std::sync::Arc;
/// struct Foo;
/// impl FFI for Foo {
///     type Origin = FromArc;
/// }
///
/// let arc = Arc::new(Foo);
/// let borrowed_ptr: CassBorrowedSharedPtr<Foo, CConst> = ArcFFI::as_ptr(&arc);
/// std::mem::drop(arc);
/// let immref = ArcFFI::cloned_from_ptr(borrowed_ptr);
/// ```
fn _test_arc_ffi_cannot_dereference_borrowed_after_drop() {}
