use crate::{encode_message, IpcEnvelope, IpcPayload, IpcRequest};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Verifies session token constant-time comparison.
///
/// # Safety
/// Pointers must be valid null-terminated C strings or null.
#[no_mangle]
pub unsafe extern "C" fn aether_verify_token(
    expected: *const c_char,
    provided: *const c_char,
) -> bool {
    if expected.is_null() || provided.is_null() {
        return false;
    }
    let exp = match CStr::from_ptr(expected).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };
    let prov = match CStr::from_ptr(provided).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let exp_b = exp.as_bytes();
    let prov_b = prov.as_bytes();
    if exp_b.len() != prov_b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in exp_b.iter().zip(prov_b.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

#[no_mangle]
pub extern "C" fn aether_generate_session_token() -> *mut c_char {
    let token = format!("aether-sess-{}", uuid::Uuid::new_v4().simple());
    match CString::new(token) {
        Ok(c_str) => c_str.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Deallocates CString returned by Rust.
///
/// # Safety
/// Pointer must have been allocated by `CString::into_raw` or null.
#[no_mangle]
pub unsafe extern "C" fn aether_free_string(s: *mut c_char) {
    if !s.is_null() {
        let _ = CString::from_raw(s);
    }
}

/// Formats wire-ready JSON envelope.
///
/// # Safety
/// Pointers must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn aether_format_envelope(
    action: *const c_char,
    token: *const c_char,
    params_json: *const c_char,
) -> *mut c_char {
    if action.is_null() {
        return std::ptr::null_mut();
    }
    let action_str = match CStr::from_ptr(action).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let token_opt = if !token.is_null() {
        CStr::from_ptr(token).to_str().ok().map(|s| s.to_string())
    } else {
        None
    };

    let req = match action_str {
        "Ping" => IpcRequest::Ping,
        "GetStatus" => IpcRequest::GetStatus,
        "RunDoctorCheck" => IpcRequest::RunDoctorCheck,
        _ => {
            let params = if !params_json.is_null() {
                CStr::from_ptr(params_json).to_str().unwrap_or("{}")
            } else {
                "{}"
            };
            IpcRequest::FullTextSearch {
                query: params.to_string(),
                limit: Some(10),
            }
        }
    };

    let envelope = IpcEnvelope {
        id: format!("ios-{}", uuid::Uuid::new_v4().simple()),
        session_token: token_opt,
        payload: IpcPayload::Request(req),
    };

    match encode_message(&envelope) {
        Ok(bytes) => match CString::new(bytes) {
            Ok(c_str) => c_str.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        Err(_) => std::ptr::null_mut(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffi_token_verification() {
        let t1 = CString::new("aether-sess-12345").unwrap();
        let t2 = CString::new("aether-sess-12345").unwrap();
        let t3 = CString::new("aether-sess-wrong").unwrap();

        unsafe {
            assert!(aether_verify_token(t1.as_ptr(), t2.as_ptr()));
            assert!(!aether_verify_token(t1.as_ptr(), t3.as_ptr()));
            assert!(!aether_verify_token(std::ptr::null(), t1.as_ptr()));
        }
    }

    #[test]
    fn test_ffi_generate_and_free_token() {
        unsafe {
            let ptr = aether_generate_session_token();
            assert!(!ptr.is_null());
            let s = CStr::from_ptr(ptr).to_str().unwrap();
            assert!(s.starts_with("aether-sess-"));
            aether_free_string(ptr);
        }
    }
}
