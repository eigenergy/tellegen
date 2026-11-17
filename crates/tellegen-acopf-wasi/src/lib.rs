//! Raw WASI ABI for the development-only browser AC OPF worker.
//!
//! The shipping wasm-bindgen adapter deliberately excludes POUNCE. This small
//! reactor is built separately for `wasm32-wasip1`, loaded in its own Worker,
//! and communicates only with length-prefixed UTF-8 JSON buffers.

#![cfg(feature = "acopf")]

use std::alloc::{alloc, dealloc, Layout};
use std::panic::{catch_unwind, AssertUnwindSafe};

use tellegen::{EngineError, ErrorCode};

const PREFIX: usize = std::mem::size_of::<u32>();

fn layout_for(len: usize) -> Option<Layout> {
    Layout::from_size_align(len, 1).ok()
}

/// Allocate an input buffer for JavaScript. The same length must be supplied
/// to [`tellegen_acopf_dealloc`].
#[no_mangle]
pub extern "C" fn tellegen_acopf_alloc(len: usize) -> *mut u8 {
    match layout_for(len) {
        // SAFETY: `len > 0` and `layout_for` returned a valid layout.
        Some(layout) if len > 0 => unsafe { alloc(layout) },
        _ => std::ptr::null_mut(),
    }
}

/// Release an input buffer allocated by [`tellegen_acopf_alloc`].
///
/// # Safety
/// `ptr` and `len` must identify one live allocation from
/// [`tellegen_acopf_alloc`].
#[no_mangle]
pub unsafe extern "C" fn tellegen_acopf_dealloc(ptr: *mut u8, len: usize) {
    if ptr.is_null() {
        return;
    }
    if let Some(layout) = layout_for(len) {
        // SAFETY: guaranteed by the caller contract above.
        unsafe { dealloc(ptr, layout) };
    }
}

fn to_payload(text: &str) -> *mut u8 {
    let bytes = text.as_bytes();
    let Ok(len) = u32::try_from(bytes.len()) else {
        return std::ptr::null_mut();
    };
    let Some(layout) = layout_for(PREFIX + bytes.len()) else {
        return std::ptr::null_mut();
    };
    // SAFETY: the non-zero prefix makes this a non-zero allocation.
    let ptr = unsafe { alloc(layout) };
    if ptr.is_null() {
        return ptr;
    }
    // SAFETY: `ptr` owns the prefix and all payload bytes.
    unsafe {
        std::ptr::write_unaligned(ptr.cast::<u32>(), len);
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.add(PREFIX), bytes.len());
    }
    ptr
}

/// The error payload: `error` is the message, unchanged, and `code` is one of
/// [`ErrorCode`]'s wire names.
fn error_payload(error: &EngineError) -> *mut u8 {
    to_payload(
        &serde_json::json!({ "error": error.message, "code": error.code.as_str() }).to_string(),
    )
}

/// Release a length-prefixed result returned by [`tellegen_acopf_solve`].
///
/// # Safety
/// `ptr` must identify one live result allocation from this module.
#[no_mangle]
pub unsafe extern "C" fn tellegen_acopf_free_payload(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: every result begins with the prefix written by `to_payload`.
    let len = unsafe { std::ptr::read_unaligned(ptr.cast::<u32>()) } as usize;
    if let Some(layout) = layout_for(PREFIX + len) {
        // SAFETY: this is the same allocation and layout used by `to_payload`.
        unsafe { dealloc(ptr, layout) };
    }
}

unsafe fn input<'a>(
    ptr: *const u8,
    len: usize,
    name: &str,
    code: ErrorCode,
) -> Result<&'a str, EngineError> {
    if ptr.is_null() || len == 0 {
        return Err(EngineError::new(code, format!("{name} must not be empty")));
    }
    // SAFETY: the ABI caller promises a readable input allocation for this call.
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    std::str::from_utf8(bytes)
        .map_err(|error| EngineError::new(code, format!("{name} is not UTF-8: {error}")))
}

/// Solve one canonical PowerIO module as nonlinear AC OPF.
///
/// Returns either a `SolveOutput` JSON object (the `SolveResponse` fields plus
/// `objective_kind`) or `{ "error": "...", "code": "..." }`.
///
/// This target aborts on panic, so the `catch_unwind` below catches nothing
/// in the shipped module: a panic traps the instance and the worker replaces
/// it. The guard only takes effect in native builds, such as the tests.
/// The worker is the cancellation boundary: terminating it stops a synchronous
/// solve without leaving a reusable wasm instance in an uncertain state.
///
/// # Safety
/// Both pointer/length pairs must describe initialized readable memory owned by
/// the caller for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn tellegen_acopf_solve(
    module_ptr: *const u8,
    module_len: usize,
    request_ptr: *const u8,
    request_len: usize,
) -> *mut u8 {
    match catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: forwarded from this function's caller contract.
        let module = unsafe {
            input(
                module_ptr,
                module_len,
                "module JSON",
                ErrorCode::InvalidInput,
            )
        }?;
        // SAFETY: forwarded from this function's caller contract.
        let request = unsafe {
            input(
                request_ptr,
                request_len,
                "request JSON",
                ErrorCode::InvalidRequest,
            )
        }?;
        let value: serde_json::Value = serde_json::from_str(request)
            .map_err(|e| EngineError::new(ErrorCode::InvalidRequest, e.to_string()))?;
        if value.get("ac_dc").is_some() {
            if value.get("ac_dc") != Some(&serde_json::Value::Bool(true))
                || value.get("formulation").and_then(|v| v.as_str()) != Some("acopf")
                || value.as_object().is_none_or(|o| o.len() != 2)
            {
                return Err(EngineError::new(
                    ErrorCode::InvalidRequest,
                    "Joint AC/DC request accepts only formulation=acopf and ac_dc=true",
                ));
            }
            // The joint solve reports plain messages. Its failures are the
            // model refusing the case unless the message names another class.
            tellegen::ac_dc::solve_module_json(module, tellegen::Problem::Acopf).map_err(
                |message| {
                    let code = ErrorCode::from_message(&message).unwrap_or(ErrorCode::InvalidInput);
                    EngineError::new(code, message)
                },
            )
        } else {
            tellegen::solve_module_output_json(module, request)
        }
    })) {
        Ok(Ok(response)) => to_payload(&response),
        Ok(Err(error)) => error_payload(&error),
        Err(_) => error_payload(&EngineError::new(
            ErrorCode::Internal,
            "AC OPF solve panicked; the worker must be replaced",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_returns_structured_error() {
        // SAFETY: `(null, 0)` is deliberately accepted as an invalid empty input.
        let ptr = unsafe { tellegen_acopf_solve(std::ptr::null(), 0, std::ptr::null(), 0) };
        assert!(!ptr.is_null());
        // SAFETY: the function just returned this live payload.
        let len = unsafe { std::ptr::read_unaligned(ptr.cast::<u32>()) } as usize;
        // SAFETY: payload allocation owns PREFIX + len initialized bytes.
        let bytes = unsafe { std::slice::from_raw_parts(ptr.add(PREFIX), len) };
        let value: serde_json::Value = serde_json::from_slice(bytes).expect("error JSON");
        assert!(value["error"].as_str().unwrap().contains("module JSON"));
        assert_eq!(value["code"], "invalid_input");
        // SAFETY: release the live payload once.
        unsafe { tellegen_acopf_free_payload(ptr) };
    }
}
