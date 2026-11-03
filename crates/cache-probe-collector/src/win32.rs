//! The only allowed unsafe capsule; compiled only by explicit Windows feature.
//! No API here installs or enumerates devices. No Windows device call is used by
//! tests or CI: the enabled binary is compiled/linked, never run in live mode.
//!
//! Signatures/flags verified against Microsoft Learn on 2026-11-03:
//! https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew
//! https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-deviceiocontrol
//! https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfiletype
//! https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptgenrandom
use crate::capture::{CaptureClock, ProbeTransport, SessionError, SessionSource, TransportError};
use cache_probe_core::Request;
use std::{
    ffi::c_void,
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle},
    ptr,
    time::{Duration, Instant},
};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateFileW(
        name: *const u16,
        access: u32,
        sharing: u32,
        security: *const c_void,
        disposition: u32,
        flags: u32,
        template: RawHandle,
    ) -> RawHandle;
    fn DeviceIoControl(
        device: RawHandle,
        ioctl: u32,
        input: *mut c_void,
        input_bytes: u32,
        output: *mut c_void,
        output_bytes: u32,
        returned: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn GetFileType(handle: RawHandle) -> u32;
}

#[link(name = "bcrypt")]
unsafe extern "system" {
    fn BCryptGenRandom(algorithm: *mut c_void, buffer: *mut u8, length: u32, flags: u32) -> i32;
}

pub fn stdout_is_pipe() -> bool {
    let stdout = io::stdout();
    let handle = stdout.as_raw_handle();
    if handle.is_null() || handle == (-1isize) as RawHandle {
        return false;
    }
    // SAFETY: borrowed process-owned stdout stays alive across this non-owning
    // query. Unknown, failed, character and disk cases all fail closed.
    unsafe { GetFileType(handle) == 3 }
}

pub struct SystemRng;
impl SessionSource for SystemRng {
    fn draw(&mut self) -> Result<u64, SessionError> {
        let mut bytes = [0u8; 8];
        // SAFETY: system-preferred RNG requires null algorithm, flag2; this
        // synchronous call can write exactly the live exclusive eight-byte array.
        let status = unsafe { BCryptGenRandom(ptr::null_mut(), bytes.as_mut_ptr(), 8, 2) };
        if status < 0 {
            return Err(SessionError);
        }
        Ok(u64::from_le_bytes(bytes))
    }
}

pub struct Device {
    handle: OwnedHandle,
}
impl Device {
    pub fn open() -> Result<Self, TransportError> {
        let name: Vec<u16> = r"\\.\WinSsdHddCacheProbeP04"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: fixed NUL-terminated local string; no optional pointers are
        // supplied; OPEN_EXISTING, generic read|write, no OVERLAPPED or inheritance.
        let raw = unsafe {
            CreateFileW(
                name.as_ptr(),
                0xc000_0000,
                3,
                ptr::null(),
                3,
                0,
                ptr::null_mut(),
            )
        };
        if raw.is_null() || raw == (-1isize) as RawHandle {
            return Err(TransportError);
        }
        // SAFETY: CreateFileW returned a fresh owned valid handle. Exactly this
        // OwnedHandle closes it; every exchange only borrows it synchronously.
        let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
        Ok(Self { handle })
    }
}

impl ProbeTransport for Device {
    fn exchange(&mut self, request: Request) -> Result<Vec<u8>, TransportError> {
        let mut input = request.to_bytes();
        let mut output = vec![0u8; request.output_capacity()];
        let mut returned = 0u32;
        // SAFETY: live owned synchronous CDO handle; input48 and output144/1680
        // are separate initialized allocations borrowed exclusively for the call.
        // No overlapped request or pointer outlives this stack frame. Returned
        // bytes are checked before slicing, and failed calls are never retried.
        let success = unsafe {
            DeviceIoControl(
                self.handle.as_raw_handle(),
                request.ioctl(),
                input.as_mut_ptr().cast(),
                input.len() as u32,
                output.as_mut_ptr().cast(),
                output.len() as u32,
                &mut returned,
                ptr::null_mut(),
            )
        };
        if success == 0 || returned as usize > output.len() {
            return Err(TransportError);
        }
        output.truncate(returned as usize);
        Ok(output)
    }
}

pub struct Clock {
    start: Instant,
}
impl Clock {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}
impl CaptureClock for Clock {
    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
    fn wait_ms(&mut self, milliseconds: u32) {
        std::thread::sleep(Duration::from_millis(u64::from(milliseconds)));
    }
}
