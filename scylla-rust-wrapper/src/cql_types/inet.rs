use crate::argconv::*;
use crate::cass_error::CassError;
use crate::types::*;
use num_traits::FromPrimitive;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::net::IpAddr;
use std::os::raw::c_char;
use std::slice::from_raw_parts;
use std::str::FromStr;

pub use crate::cass_inet_types::CassInet;

#[repr(u8)] // address_length field in CassInet is cass_uint8_t
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone)]
pub(crate) enum CassInetLength {
    CASS_INET_V4 = 4,
    CASS_INET_V6 = 16,
}

// Need to implement manually, because of the clippy 0.1.83 lints:
// | `FromPrimitive` is not local
// | move the `impl` block outside of this constant `_IMPL_NUM_FromPrimitive_FOR_CassInetLength`
impl FromPrimitive for CassInetLength {
    fn from_i64(n: i64) -> Option<Self> {
        match n {
            4 => Some(Self::CASS_INET_V4),
            16 => Some(Self::CASS_INET_V6),
            _ => None,
        }
    }

    fn from_u64(n: u64) -> Option<Self> {
        match n {
            4 => Some(Self::CASS_INET_V4),
            16 => Some(Self::CASS_INET_V6),
            _ => None,
        }
    }
}

unsafe fn cass_inet_init(address: *const cass_uint8_t, address_length: CassInetLength) -> CassInet {
    let mut array = [0; 16];
    let length = address_length as usize;
    array[0..length].clone_from_slice(unsafe { from_raw_parts(address, length) });

    CassInet {
        address: array,
        address_length: address_length as u8,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_inet_init_v4(address: *const cass_uint8_t) -> CassInet {
    unsafe { cass_inet_init(address, CassInetLength::CASS_INET_V4) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_inet_init_v6(address: *const cass_uint8_t) -> CassInet {
    unsafe { cass_inet_init(address, CassInetLength::CASS_INET_V6) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_inet_string(inet: CassInet, output: *mut c_char) {
    let ip_addr: IpAddr = match inet.try_into() {
        Ok(v) => v,
        Err(_) => return, // Behaviour of cppdriver.
    };

    let string_representation = ip_addr.to_string();
    unsafe {
        std::ptr::copy_nonoverlapping(
            string_representation.as_ptr(),
            output as *mut u8,
            string_representation.len(),
        )
    };

    // Null-terminate
    let null_byte = unsafe { output.add(string_representation.len()) as *mut c_char };
    unsafe { *null_byte = 0 };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_inet_from_string(
    input: *const c_char,
    inet: *mut CassInet,
) -> CassError {
    unsafe { cass_inet_from_string_n(input, strlen(input), inet) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_inet_from_string_n(
    input_raw: *const c_char,
    input_length: size_t,
    inet_raw: *mut CassInet,
) -> CassError {
    let Some(input_str) = (unsafe { ptr_to_cstr_n(input_raw, input_length) }) else {
        tracing::error!("Provided null input string to cass_inet_from_string(_n)!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    let Ok(ip_addr) = IpAddr::from_str(input_str) else {
        tracing::error!(
            "Provided input string which does not represent a valid IP address to cass_inet_from_string(_n)!"
        );
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    unsafe { std::ptr::write(inet_raw, ip_addr.into()) };

    CassError::CASS_OK
}

impl TryFrom<CassInet> for IpAddr {
    type Error = ();
    fn try_from(inet: CassInet) -> Result<Self, Self::Error> {
        match FromPrimitive::from_u8(inet.address_length) {
            Some(CassInetLength::CASS_INET_V4) => {
                let addr_bytes: [cass_uint8_t; CassInetLength::CASS_INET_V4 as usize] = inet
                    .address[0..(CassInetLength::CASS_INET_V4 as usize)]
                    .try_into()
                    .unwrap();
                Ok(IpAddr::V4(addr_bytes.into()))
            }
            Some(CassInetLength::CASS_INET_V6) => {
                let addr_bytes: [cass_uint8_t; CassInetLength::CASS_INET_V6 as usize] = inet
                    .address[0..(CassInetLength::CASS_INET_V6 as usize)]
                    .try_into()
                    .unwrap();
                Ok(IpAddr::V6(addr_bytes.into()))
            }
            None => Err(()),
        }
    }
}

impl From<IpAddr> for CassInet {
    fn from(ip_addr: IpAddr) -> Self {
        match ip_addr {
            IpAddr::V4(v4_addr) => {
                let mut address = [0; 16];
                address[0..(CassInetLength::CASS_INET_V4 as usize)]
                    .copy_from_slice(&v4_addr.octets());

                CassInet {
                    address,
                    address_length: CassInetLength::CASS_INET_V4 as u8,
                }
            }
            IpAddr::V6(v6_addr) => {
                let mut address = [0; 16];
                address[0..(CassInetLength::CASS_INET_V6 as usize)]
                    .copy_from_slice(&v6_addr.octets());

                CassInet {
                    address,
                    address_length: CassInetLength::CASS_INET_V6 as u8,
                }
            }
        }
    }
}
