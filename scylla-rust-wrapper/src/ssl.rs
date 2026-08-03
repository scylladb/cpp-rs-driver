use crate::argconv::{
    ArcFFI, CMut, CassBorrowedSharedPtr, CassOwnedSharedPtr, CassStrLen, CassStrLenDelimited,
    CassStrNulTerminated, FFI, FromArc,
};
use crate::cass_error::CassError;
use crate::cass_ssl_types::CassSslVerifyFlags;
use crate::types::size_t;
use libc::{c_int, strlen};
use openssl::ssl::SslVerifyMode;
use openssl_sys::{
    BIO, BIO_free_all, BIO_new_mem_buf, EVP_PKEY_free, PEM_read_bio_PrivateKey, PEM_read_bio_X509,
    SSL_CTX, SSL_CTX_add_extra_chain_cert, SSL_CTX_free, SSL_CTX_new, SSL_CTX_set_cert_store,
    SSL_CTX_set_verify, SSL_CTX_use_PrivateKey, SSL_CTX_use_certificate, TLS_method, X509_STORE,
    X509_STORE_CTX, X509_STORE_CTX_get_error, X509_STORE_CTX_set_error, X509_STORE_add_cert,
    X509_STORE_new, X509_V_ERR_EMAIL_MISMATCH, X509_V_ERR_HOSTNAME_MISMATCH,
    X509_V_ERR_IP_ADDRESS_MISMATCH, X509_V_OK, X509_free,
};
use std::convert::TryInto;
use std::os::raw::c_char;
use std::os::raw::c_void;
use std::sync::Arc;

pub struct CassSsl {
    pub(crate) ssl_context: *mut SSL_CTX,
    pub(crate) trusted_store: *mut X509_STORE,
}

impl FFI for CassSsl {
    type Origin = FromArc;
}

/// Type of the verification callback accepted by `SSL_CTX_set_verify()`.
type VerifyCallback = Option<extern "C" fn(c_int, *mut X509_STORE_CTX) -> c_int>;

/// All bits recognised by [`cass_ssl_set_verify_flags`].
const CASS_SSL_VERIFY_KNOWN_FLAGS: i32 = (CassSslVerifyFlags::CASS_SSL_VERIFY_PEER_CERT.0
    | CassSslVerifyFlags::CASS_SSL_VERIFY_PEER_IDENTITY.0
    | CassSslVerifyFlags::CASS_SSL_VERIFY_PEER_IDENTITY_DNS.0)
    as i32;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_ssl_new() -> CassOwnedSharedPtr<CassSsl, CMut> {
    openssl_sys::init();
    unsafe { cass_ssl_new_no_lib_init() }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_ssl_new_no_lib_init() -> CassOwnedSharedPtr<CassSsl, CMut> {
    let ssl_context: *mut SSL_CTX = unsafe { SSL_CTX_new(TLS_method()) };
    let trusted_store: *mut X509_STORE = unsafe { X509_STORE_new() };

    unsafe {
        SSL_CTX_set_cert_store(ssl_context, trusted_store);
        SSL_CTX_set_verify(
            ssl_context,
            SslVerifyMode::PEER.bits(),
            Some(verify_peer_cert_callback),
        );
    }

    let ssl = CassSsl {
        ssl_context,
        trusted_store,
    };

    ArcFFI::into_ptr(Arc::new(ssl))
}

// This is required for the type system to impl Send + Sync for Arc<CassSsl>.
// Otherwise, clippy complains about using Arc where Rc would do.
// In our case, though, we need to use Arc because we potentially do share
// the Arc between threads, so employing Rc here would lead to races.
unsafe impl Send for CassSsl {}
unsafe impl Sync for CassSsl {}

impl Drop for CassSsl {
    fn drop(&mut self) {
        unsafe {
            SSL_CTX_free(self.ssl_context);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_ssl_free(ssl: CassOwnedSharedPtr<CassSsl, CMut>) {
    ArcFFI::free(ssl);
}

/// Verification callback used to implement [`CASS_SSL_VERIFY_PEER_CERT`],
/// i.e. chain-only verification.
///
/// The Rust driver unconditionally pins the expected peer identity to the
/// node's IP address (`ssl.param_mut().set_ip(node_address.ip())`) before every
/// handshake, so OpenSSL always checks the peer identity on top of the chain.
/// There is no way to undo that from the `SSL_CTX` we hand over. Instead, we
/// install this callback, which lets the chain validation proceed as usual but
/// tolerates the identity mismatch errors, leaving `CASS_SSL_VERIFY_PEER_CERT`
/// with exactly the semantics the C API promises: "certificate is present and
/// valid", without requiring it to match the peer's address.
///
/// Declared as a safe `extern "C" fn`, because that is the callback type
/// `SSL_CTX_set_verify()` expects; `x509_ctx` is only ever supplied by OpenSSL.
extern "C" fn verify_peer_cert_callback(
    preverify_ok: c_int,
    x509_ctx: *mut X509_STORE_CTX,
) -> c_int {
    // Anything that already passed is accepted as-is.
    if preverify_ok == 1 {
        return 1;
    }

    let error = unsafe { X509_STORE_CTX_get_error(x509_ctx) };
    match error {
        X509_V_ERR_HOSTNAME_MISMATCH
        | X509_V_ERR_EMAIL_MISMATCH
        | X509_V_ERR_IP_ADDRESS_MISMATCH => {
            // Reset the error, so that the handshake does not merely continue
            // but `SSL_get_verify_result()` also reports success afterwards.
            unsafe { X509_STORE_CTX_set_error(x509_ctx, X509_V_OK) };
            1
        }
        // Every other failure - an untrusted issuer, an expired certificate,
        // a broken chain - is still fatal.
        _ => 0,
    }
}

unsafe extern "C" fn pem_password_callback(
    buf: *mut c_char,
    size: c_int,
    _rwflag: c_int,
    u: *mut c_void,
) -> c_int {
    if u.is_null() {
        return 0;
    }

    let len = unsafe { strlen(u as *const c_char) };
    if len == 0 {
        return 0;
    }

    let mut to_copy = size;
    if len < to_copy.try_into().unwrap() {
        to_copy = len as c_int;
    }

    // Same as: memcpy(buf, u, to_copy);
    unsafe { std::ptr::copy_nonoverlapping(u as *const c_char, buf, to_copy as usize) };

    len as c_int
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_ssl_add_trusted_cert(
    ssl: CassBorrowedSharedPtr<CassSsl, CMut>,
    cert: CassStrNulTerminated<'_>,
) -> CassError {
    let (cert, cert_length) = unsafe { cert.as_len_delimited() };
    unsafe { cass_ssl_add_trusted_cert_n(ssl, cert, cert_length) }
}

// These functions accept PEM-encoded data (ASCII text format), not binary DER.
// Both the cpp-driver API documentation and the implementation (PEM_read_bio_X509,
// PEM_read_bio_PrivateKey) enforce PEM-only input. Since ASCII is a subset of
// UTF-8, using CassStrLenDelimited (which validates UTF-8) is safe and appropriate.

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_ssl_add_trusted_cert_n(
    ssl: CassBorrowedSharedPtr<CassSsl, CMut>,
    cert: CassStrLenDelimited<'_>,
    cert_length: CassStrLen,
) -> CassError {
    let Some(ssl) = ArcFFI::cloned_from_ptr(ssl) else {
        tracing::error!("Provided null ssl pointer to cass_ssl_add_trusted_cert_n!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    let cert_str = match unsafe { cert.to_str(cert_length) } {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Invalid PEM certificate data: {e}");
            return CassError::CASS_ERROR_SSL_INVALID_CERT;
        }
    };

    let bio = unsafe {
        BIO_new_mem_buf(
            cert_str.as_ptr() as *const c_void,
            cert_str.len().try_into().unwrap(),
        )
    };

    if bio.is_null() {
        return CassError::CASS_ERROR_SSL_INVALID_CERT;
    }

    let x509 = unsafe {
        PEM_read_bio_X509(
            bio,
            std::ptr::null_mut(),
            Some(pem_password_callback),
            std::ptr::null_mut(),
        )
    };

    unsafe { BIO_free_all(bio) };

    if x509.is_null() {
        return CassError::CASS_ERROR_SSL_INVALID_CERT;
    }

    unsafe {
        X509_STORE_add_cert(ssl.trusted_store, x509);
        X509_free(x509);
    }

    CassError::CASS_OK
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_ssl_set_verify_flags(
    ssl: CassBorrowedSharedPtr<CassSsl, CMut>,
    flags: i32,
) {
    let Some(ssl) = ArcFFI::cloned_from_ptr(ssl) else {
        tracing::error!("Provided null ssl pointer to cass_ssl_set_verify_flags!");
        return;
    };

    // `CassSslVerifyFlags` is a bitmask: the values are disjoint bits, meant to
    // be combined, e.g. `CASS_SSL_VERIFY_PEER_CERT | CASS_SSL_VERIFY_PEER_IDENTITY`.
    // Matching on the value as a whole would reject every such combination.
    //
    // Bits outside the mask are still unrecognised, so - as before - they make
    // the whole value unhonourable and we fall back to the strictest setting.
    let flags = if flags & !CASS_SSL_VERIFY_KNOWN_FLAGS != 0 {
        tracing::error!(
            "Provided unknown CASS_SSL_VERIFY flags: {flags:#x}. \
             Enforcing the strictest verification (peer certificate and peer identity) instead."
        );
        CassSslVerifyFlags::CASS_SSL_VERIFY_PEER_IDENTITY.0 as i32
    } else {
        flags
    };

    if flags & CassSslVerifyFlags::CASS_SSL_VERIFY_PEER_IDENTITY_DNS.0 as i32 != 0 {
        tracing::warn!(
            "The CASS_SSL_VERIFY_PEER_IDENTITY_DNS is not supported, CASS_SSL_VERIFY_PEER_IDENTITY is set in SSL context instead."
        );
    }

    // Verifying the peer's identity implies verifying its certificate, so any
    // recognised bit turns peer verification on; `CASS_SSL_VERIFY_NONE` is
    // their absence.
    let verify_identity = flags
        & (CassSslVerifyFlags::CASS_SSL_VERIFY_PEER_IDENTITY.0 as i32
            | CassSslVerifyFlags::CASS_SSL_VERIFY_PEER_IDENTITY_DNS.0 as i32)
        != 0;
    let verify_peer =
        flags & CASS_SSL_VERIFY_KNOWN_FLAGS != CassSslVerifyFlags::CASS_SSL_VERIFY_NONE.0 as i32;

    let (mode, callback): (SslVerifyMode, VerifyCallback) = match (verify_peer, verify_identity) {
        // Verifying the peer's identity implies verifying its certificate.
        (_, true) => {
            // Rust Driver verifies identity by default (and provides no lever to turn this verification off)
            // by expecting particular IP address to be present in the SAN field.
            // This means that once we enable SslVerifyMode::PEER, we get certificate + identity verification.
            (SslVerifyMode::PEER, None)
        }
        (true, false) => {
            // Chain-only verification. The driver pins the peer's identity for us,
            // so the mismatches that pinning produces have to be tolerated.
            (SslVerifyMode::PEER, Some(verify_peer_cert_callback))
        }
        (false, false) => (SslVerifyMode::NONE, None),
    };

    unsafe { SSL_CTX_set_verify(ssl.ssl_context, mode.bits(), callback) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_ssl_set_cert(
    ssl: CassBorrowedSharedPtr<CassSsl, CMut>,
    cert: CassStrNulTerminated<'_>,
) -> CassError {
    let (cert, cert_length) = unsafe { cert.as_len_delimited() };
    unsafe { cass_ssl_set_cert_n(ssl, cert, cert_length) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_ssl_set_cert_n(
    ssl: CassBorrowedSharedPtr<CassSsl, CMut>,
    cert: CassStrLenDelimited<'_>,
    cert_length: CassStrLen,
) -> CassError {
    let Some(ssl) = ArcFFI::cloned_from_ptr(ssl) else {
        tracing::error!("Provided null ssl pointer to cass_ssl_set_cert_n!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    let cert_str = match unsafe { cert.to_str(cert_length) } {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Invalid PEM certificate data: {e}");
            return CassError::CASS_ERROR_SSL_INVALID_CERT;
        }
    };

    let bio = unsafe {
        BIO_new_mem_buf(
            cert_str.as_ptr() as *const c_void,
            cert_str.len().try_into().unwrap(),
        )
    };

    if bio.is_null() {
        return CassError::CASS_ERROR_SSL_INVALID_CERT;
    }

    let rc = unsafe { SSL_CTX_use_certificate_chain_bio(ssl.ssl_context, bio) };
    unsafe { BIO_free_all(bio) };

    if rc == 0 {
        return CassError::CASS_ERROR_SSL_INVALID_CERT;
    }

    CassError::CASS_OK
}

#[allow(non_snake_case)]
unsafe extern "C" fn SSL_CTX_use_certificate_chain_bio(
    ssl_context: *mut SSL_CTX,
    bio: *mut BIO,
) -> c_int {
    let mut ret = 0;
    let x = unsafe {
        PEM_read_bio_X509(
            bio,
            std::ptr::null_mut(),
            Some(pem_password_callback),
            std::ptr::null_mut(),
        )
    };

    if x.is_null() {
        return ret;
    }

    ret = unsafe { SSL_CTX_use_certificate(ssl_context, x) };

    if ret != 1 {
        loop {
            let ca = unsafe {
                PEM_read_bio_X509(
                    bio,
                    std::ptr::null_mut(),
                    Some(pem_password_callback),
                    std::ptr::null_mut(),
                )
            };

            if ca.is_null() {
                ret = 0;
                break;
            }

            let r = unsafe { SSL_CTX_add_extra_chain_cert(ssl_context, ca) };
            if r == 0 {
                unsafe { X509_free(ca) };
                ret = 0;
                break;
            }
        }
    }

    if !x.is_null() {
        unsafe { X509_free(x) }
    };

    ret
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_ssl_set_private_key(
    ssl: CassBorrowedSharedPtr<CassSsl, CMut>,
    key: CassStrNulTerminated<'_>,
    password: *mut c_char,
) -> CassError {
    if password.is_null() {
        return CassError::CASS_ERROR_SSL_INVALID_PRIVATE_KEY;
    }

    let (key, key_length) = unsafe { key.as_len_delimited() };
    unsafe { cass_ssl_set_private_key_n(ssl, key, key_length, password, 0) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_ssl_set_private_key_n(
    ssl: CassBorrowedSharedPtr<CassSsl, CMut>,
    key: CassStrLenDelimited<'_>,
    key_length: CassStrLen,
    // Password is passed as opaque userdata to OpenSSL's PEM password callback,
    // not used as a string by this function directly. Left as raw pointer.
    password: *mut c_char,
    _password_length: size_t,
) -> CassError {
    let Some(ssl) = ArcFFI::cloned_from_ptr(ssl) else {
        tracing::error!("Provided null ssl pointer to cass_ssl_set_private_key_n!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    let key_str = match unsafe { key.to_str(key_length) } {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Invalid PEM private key data: {e}");
            return CassError::CASS_ERROR_SSL_INVALID_PRIVATE_KEY;
        }
    };

    let bio = unsafe {
        BIO_new_mem_buf(
            key_str.as_ptr() as *const c_void,
            key_str.len().try_into().unwrap(),
        )
    };

    if bio.is_null() {
        return CassError::CASS_ERROR_SSL_INVALID_CERT;
    }

    let pkey = unsafe {
        PEM_read_bio_PrivateKey(
            bio,
            std::ptr::null_mut(),
            Some(pem_password_callback),
            password as *mut c_void,
        )
    };

    unsafe { BIO_free_all(bio) };

    if pkey.is_null() {
        return CassError::CASS_ERROR_SSL_INVALID_PRIVATE_KEY;
    }

    unsafe {
        SSL_CTX_use_PrivateKey(ssl.ssl_context, pkey);
        EVP_PKEY_free(pkey);
    }

    CassError::CASS_OK
}

#[cfg(test)]
mod tests {
    use openssl_sys::SSL_CTX_get_verify_mode;

    use super::*;

    /// Reads the verification mode currently configured in the `SSL_CTX`.
    fn verify_mode(ssl: &CassBorrowedSharedPtr<'_, CassSsl, CMut>) -> c_int {
        let ssl = ArcFFI::as_ref(ssl.borrow()).unwrap();
        unsafe { SSL_CTX_get_verify_mode(ssl.ssl_context) }
    }

    #[test]
    fn verify_flags_are_a_bitmask() {
        unsafe {
            let ssl = cass_ssl_new();

            cass_ssl_set_verify_flags(
                ssl.borrow(),
                CassSslVerifyFlags::CASS_SSL_VERIFY_NONE.0 as i32,
            );
            assert_eq!(verify_mode(&ssl.borrow()), SslVerifyMode::NONE.bits());

            // Each flag on its own enables peer verification...
            for flags in [
                CassSslVerifyFlags::CASS_SSL_VERIFY_PEER_CERT,
                CassSslVerifyFlags::CASS_SSL_VERIFY_PEER_IDENTITY,
                CassSslVerifyFlags::CASS_SSL_VERIFY_PEER_IDENTITY_DNS,
            ]
            .map(|flag| flag.0 as i32)
            {
                cass_ssl_set_verify_flags(
                    ssl.borrow(),
                    CassSslVerifyFlags::CASS_SSL_VERIFY_NONE.0 as i32,
                );
                cass_ssl_set_verify_flags(ssl.borrow(), flags);
                assert_eq!(verify_mode(&ssl.borrow()), SslVerifyMode::PEER.bits());
            }

            // ...and so does any combination of them, as documented in the
            // TLS guide and accepted by the C/C++ driver.
            for flags in [
                CassSslVerifyFlags::CASS_SSL_VERIFY_PEER_CERT.0 as i32
                    | CassSslVerifyFlags::CASS_SSL_VERIFY_PEER_IDENTITY.0 as i32,
                CassSslVerifyFlags::CASS_SSL_VERIFY_PEER_CERT.0 as i32
                    | CassSslVerifyFlags::CASS_SSL_VERIFY_PEER_IDENTITY_DNS.0 as i32,
                CASS_SSL_VERIFY_KNOWN_FLAGS,
            ] {
                cass_ssl_set_verify_flags(
                    ssl.borrow(),
                    CassSslVerifyFlags::CASS_SSL_VERIFY_NONE.0 as i32,
                );
                cass_ssl_set_verify_flags(ssl.borrow(), flags);
                assert_eq!(verify_mode(&ssl.borrow()), SslVerifyMode::PEER.bits());
            }

            cass_ssl_free(ssl);
        }
    }

    #[test]
    fn unknown_verify_flags_enforce_strictest_verification() {
        unsafe {
            let ssl = cass_ssl_new();

            // An unrecognised bit makes the whole value unhonourable, whether it
            // comes on its own or smuggled in next to known flags. Verification
            // must then be turned on, and in particular a previously requested
            // `CASS_SSL_VERIFY_NONE` must not be left in effect.
            for flags in [
                0x08,
                -1,
                CassSslVerifyFlags::CASS_SSL_VERIFY_PEER_IDENTITY.0 as i32 | 0x10,
            ] {
                cass_ssl_set_verify_flags(
                    ssl.borrow(),
                    CassSslVerifyFlags::CASS_SSL_VERIFY_NONE.0 as i32,
                );
                assert_eq!(verify_mode(&ssl.borrow()), SslVerifyMode::NONE.bits());

                cass_ssl_set_verify_flags(ssl.borrow(), flags);
                assert_eq!(
                    verify_mode(&ssl.borrow()),
                    SslVerifyMode::PEER.bits(),
                    "unknown flags {flags:#x} did not enable peer verification"
                );
            }

            cass_ssl_free(ssl);
        }
    }
}
