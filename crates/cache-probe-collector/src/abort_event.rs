//! AMD64 ABORT-1 capsule. No device APIs are linked here.
//! Unknown handles are queried only: never waited, adopted or closed.
//! NtQueryObject is resolved as documented by Microsoft Learn (2026-11-19).
use std::{
    ffi::c_void,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle},
};
static ADOPTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub const ENVIRONMENT: &str = "P04_ABORT_EVENT_V1";
pub const SYNCHRONIZE: u32 = 0x0010_0000;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventError {
    Invalid,
    Unavailable,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> RawHandle;
    fn PssCaptureSnapshot(
        process: RawHandle,
        flags: u32,
        context_flags: u32,
        snapshot: *mut RawHandle,
    ) -> u32;
    fn PssWalkMarkerCreate(allocator: *const c_void, marker: *mut RawHandle) -> u32;
    fn PssWalkSnapshot(
        snapshot: RawHandle,
        class: u32,
        marker: RawHandle,
        buffer: *mut c_void,
        bytes: u32,
    ) -> u32;
    fn PssWalkMarkerFree(marker: RawHandle) -> u32;
    fn PssFreeSnapshot(process: RawHandle, snapshot: RawHandle) -> u32;
    fn GetFileType(handle: RawHandle) -> u32;
    fn GetHandleInformation(handle: RawHandle, flags: *mut u32) -> i32;
    fn GetModuleHandleW(name: *const u16) -> RawHandle;
    fn GetProcAddress(module: RawHandle, name: *const u8) -> *mut c_void;
    fn SetHandleInformation(handle: RawHandle, mask: u32, flags: u32) -> i32;
    fn WaitForSingleObject(handle: RawHandle, milliseconds: u32) -> u32;
}
type Query = unsafe extern "system" fn(RawHandle, u32, *mut c_void, u32, *mut u32) -> i32;
#[repr(C)]
#[derive(Default)]
struct Basic {
    attributes: u32,
    access: u32,
    handles: u32,
    pointers: u32,
    reserved: [u32; 10],
}
#[repr(C)]
struct Unicode {
    length: u16,
    maximum: u16,
    buffer: *const u16,
}
pub fn stdout_is_pipe() -> bool {
    // SAFETY: borrowed process stdout stays live throughout nonowning query.
    unsafe { GetFileType(std::io::stdout().as_raw_handle()) == 3 }
}
pub struct AbortEvent {
    handle: OwnedHandle,
    latched: bool,
}
impl AbortEvent {
    /// Validate and adopt the event exclusively transferred at process startup.
    ///
    /// # Safety
    /// The caller establishes trusted startup ownership: every handle passing
    /// validation is exclusively transferred, with no Rust owner or concurrent
    /// close/reuse/environment mutation. Type, rights and flags cannot establish
    /// ownership or origin. Rejected inputs are never adopted, waited or closed.
    pub unsafe fn from_environment() -> Result<Self, EventError> {
        let value = std::env::var_os(ENVIRONMENT).ok_or(EventError::Invalid)?;
        let text = value.to_str().ok_or(EventError::Invalid)?;
        let raw = parse(text)? as RawHandle;
        // The host may permanently enable StrictHandleChecks. Enumerate only
        // our own handle values before any query of untrusted ambient input.
        if !present_handle(raw)? {
            return Err(EventError::Invalid);
        }
        // SAFETY: no ownership/wait until OS type and exact rights are verified.
        unsafe {
            validate(raw)?;
        }
        let mut flags = 0;
        if unsafe { GetHandleInformation(raw, &mut flags) } == 0 {
            return Err(EventError::Unavailable);
        }
        // Exactly the trusted startup inheritance flag; never a local decoy.
        if flags != 1 {
            return Err(EventError::Invalid);
        }
        if ADOPTED
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            )
            .is_err()
        {
            return Err(EventError::Invalid);
        }
        // SAFETY: the launcher transferred this now-validated child handle exactly
        // once. Clearing inheritance precedes any device open; Drop closes once.
        let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
        if unsafe { SetHandleInformation(handle.as_raw_handle(), 1, 0) } == 0 {
            return Err(EventError::Unavailable);
        }
        Ok(Self {
            handle,
            latched: false,
        })
    }
    pub fn poll(&mut self) -> Result<bool, EventError> {
        self.wait(0)
    }
    pub fn wait(&mut self, milliseconds: u32) -> Result<bool, EventError> {
        if self.latched {
            return Ok(true);
        }
        #[cfg(feature = "synthetic-ipc")]
        SYNTHETIC_WAIT_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // SAFETY: sole owned validated Event remains live during synchronous wait.
        match unsafe { WaitForSingleObject(self.handle.as_raw_handle(), milliseconds) } {
            0 => {
                self.latched = true;
                Ok(true)
            }
            258 => Ok(false),
            _ => Err(EventError::Unavailable),
        }
    }
    #[cfg(feature = "synthetic-ipc")]
    #[allow(dead_code)] // Only the separate synthetic worker borrows this view.
    pub fn raw_for_synthetic_check(&self) -> RawHandle {
        self.handle.as_raw_handle()
    }
}
#[cfg(feature = "synthetic-ipc")]
static SYNTHETIC_WAIT_CALLS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
#[cfg(feature = "synthetic-ipc")]
#[allow(dead_code)]
pub fn synthetic_ownership_observations() -> (bool, usize) {
    (
        ADOPTED.load(std::sync::atomic::Ordering::Acquire),
        SYNTHETIC_WAIT_CALLS.load(std::sync::atomic::Ordering::Relaxed),
    )
}
#[cfg(feature = "synthetic-ipc")]
#[allow(dead_code)]
pub fn synthetic_event_properties(handle: &OwnedHandle) -> Result<(), EventError> {
    // SAFETY: a live borrowed owner pins this known-valid handle for the query;
    // this helper never adopts, waits, changes flags or closes it.
    unsafe { validate(handle.as_raw_handle()) }
}
pub fn parse(text: &str) -> Result<usize, EventError> {
    if text.len() != 16
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(EventError::Invalid);
    }
    let value = u64::from_str_radix(text, 16).map_err(|_| EventError::Invalid)?;
    // Excludes null, pseudo handles and non-AMD64 handle representations.
    if value == 0 || value > isize::MAX as u64 || usize::BITS != 64 {
        return Err(EventError::Invalid);
    }
    Ok(value as usize)
}
fn query_function() -> Result<Query, EventError> {
    let name: Vec<u16> = "ntdll.dll\0".encode_utf16().collect();
    let module = unsafe { GetModuleHandleW(name.as_ptr()) };
    if module.is_null() {
        return Err(EventError::Unavailable);
    }
    let address = unsafe { GetProcAddress(module, c"NtQueryObject".as_ptr().cast()) };
    if address.is_null() {
        return Err(EventError::Unavailable);
    }
    Ok(unsafe { std::mem::transmute::<*mut c_void, Query>(address) })
}
unsafe fn validate(raw: RawHandle) -> Result<(), EventError> {
    unsafe { validate_properties(raw, query_function()?) }
}
#[cfg(feature = "synthetic-ipc")]
#[allow(dead_code, clippy::not_unsafe_ptr_arg_deref)]
pub fn invalid_handle_for_synthetic_check(raw: RawHandle) -> bool {
    matches!(present_handle(raw), Ok(false))
}
#[cfg(feature = "synthetic-ipc")]
#[allow(dead_code, clippy::not_unsafe_ptr_arg_deref)]
pub fn excluded_alias_for_synthetic_check(candidate: RawHandle, event: RawHandle) -> bool {
    match present_handle(candidate) {
        Ok(false) => true,
        Ok(true) => {
            let name: Vec<u16> = "kernelbase.dll\0".encode_utf16().collect();
            let module = unsafe { GetModuleHandleW(name.as_ptr()) };
            if module.is_null() {
                return false;
            }
            let address =
                unsafe { GetProcAddress(module, c"CompareObjectHandles".as_ptr().cast()) };
            if address.is_null() {
                return false;
            }
            let compare: unsafe extern "system" fn(RawHandle, RawHandle) -> i32 =
                unsafe { std::mem::transmute(address) };
            unsafe { compare(candidate, event) == 0 }
        }
        Err(_) => false,
    }
}

// PSS_HANDLE_ENTRY AMD64 prefix and the largest (Thread) type-specific union.
// We read only Handle. No name, object pointer or other process is inspected.
#[repr(C)]
struct HandleEntry {
    handle: RawHandle,
    flags: u32,
    object_type: u32,
    capture: [u32; 2],
    attributes: u32,
    access: u32,
    handles: u32,
    pointers: u32,
    paged: u32,
    nonpaged: u32,
    creation: [u32; 2],
    type_length: u16,
    type_name: *const u16,
    name_length: u16,
    name: *const u16,
    type_information: [usize; 6],
}
const _: () = assert!(size_of::<HandleEntry>() == 136);
struct HandleSnapshot {
    snapshot: RawHandle,
    marker: RawHandle,
}
impl HandleSnapshot {
    fn close(mut self) -> Result<(), EventError> {
        let marker = unsafe { PssWalkMarkerFree(self.marker) };
        self.marker = std::ptr::null_mut();
        let snapshot = unsafe { PssFreeSnapshot(GetCurrentProcess(), self.snapshot) };
        self.snapshot = std::ptr::null_mut();
        if marker == 0 && snapshot == 0 {
            Ok(())
        } else {
            Err(EventError::Unavailable)
        }
    }
}
impl Drop for HandleSnapshot {
    fn drop(&mut self) {
        unsafe {
            if !self.marker.is_null() {
                PssWalkMarkerFree(self.marker);
            }
            if !self.snapshot.is_null() {
                PssFreeSnapshot(GetCurrentProcess(), self.snapshot);
            }
        }
    }
}
fn present_handle(raw: RawHandle) -> Result<bool, EventError> {
    let mut snapshot = std::ptr::null_mut();
    // SAFETY: self-only, handles-only capture: no VA clone, names, thread state,
    // remote-process access, privileges, or device APIs. Only this object frees
    // the successful snapshot and walk marker, using their matching PSS APIs.
    if unsafe { PssCaptureSnapshot(GetCurrentProcess(), 4, 0, &mut snapshot) } != 0 {
        return Err(EventError::Unavailable);
    }
    let mut owner = HandleSnapshot {
        snapshot,
        marker: std::ptr::null_mut(),
    };
    if unsafe { PssWalkMarkerCreate(std::ptr::null(), &mut owner.marker) } != 0 {
        return Err(EventError::Unavailable);
    }
    let result = (|| {
        for _ in 0..4096 {
            let mut entry: HandleEntry = unsafe { std::mem::zeroed() };
            match unsafe {
                PssWalkSnapshot(
                    owner.snapshot,
                    2,
                    owner.marker,
                    (&mut entry as *mut HandleEntry).cast(),
                    size_of::<HandleEntry>() as u32,
                )
            } {
                0 => {
                    if entry.handle == raw {
                        return Ok(true);
                    }
                }
                259 => return Ok(false),
                _ => return Err(EventError::Unavailable),
            }
        }
        Err(EventError::Unavailable)
    })();
    owner.close()?;
    // Startup is single-owner: nobody in this process may concurrently close or
    // reuse the inherited event. A snapshot is not an event-origin credential.
    result
}
unsafe fn validate_properties(raw: RawHandle, query: Query) -> Result<(), EventError> {
    let mut basic = Basic::default();
    let mut returned = 0;
    let status = unsafe {
        query(
            raw,
            0,
            (&mut basic as *mut Basic).cast(),
            size_of::<Basic>() as u32,
            &mut returned,
        )
    };
    if status < 0 {
        return Err(if status as u32 == 0xc0000008 {
            EventError::Invalid
        } else {
            EventError::Unavailable
        });
    }
    if returned as usize != size_of::<Basic>() || basic.access != SYNCHRONIZE {
        return Err(EventError::Invalid);
    }
    let mut storage = [0usize; 128];
    returned = 0;
    if unsafe {
        query(
            raw,
            2,
            storage.as_mut_ptr().cast(),
            size_of_val(&storage) as u32,
            &mut returned,
        )
    } < 0
    {
        return Err(EventError::Unavailable);
    }
    if (returned as usize) < size_of::<Unicode>() || returned as usize > size_of_val(&storage) {
        return Err(EventError::Unavailable);
    }
    let unicode = unsafe { &*storage.as_ptr().cast::<Unicode>() };
    let start = storage.as_ptr() as usize;
    let end = start + returned as usize;
    let p = unicode.buffer as usize;
    if unicode.length != 10
        || unicode.maximum < unicode.length
        || !p.is_multiple_of(2)
        || p < start
        || p.checked_add(10).is_none_or(|last| last > end)
    {
        return Err(EventError::Invalid);
    }
    if unsafe { std::slice::from_raw_parts(unicode.buffer, 5) } != [69, 118, 101, 110, 116] {
        return Err(EventError::Invalid);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn c02_reserved_handle_encoding_is_exact() {
        for bad in [
            "",
            "0",
            "0000000000000000",
            "0x00000000000001",
            "000000000000000A",
            "ffffffffffffffff",
            "fffffffffffffffe",
            "00000000000000001",
            "00000000000000g1",
        ] {
            assert_eq!(parse(bad), Err(EventError::Invalid));
        }
        assert_eq!(parse("0000000000000004"), Ok(4));
    }
    unsafe extern "system" fn synthetic_query(
        raw: RawHandle,
        class: u32,
        buffer: *mut c_void,
        bytes: u32,
        returned: *mut u32,
    ) -> i32 {
        let case = raw as usize;
        if class == 0 {
            if case == 1 {
                return 0xc0000008u32 as i32;
            }
            if case == 2 {
                return 0xc0000002u32 as i32;
            }
            if bytes < size_of::<Basic>() as u32 {
                return 0xc0000004u32 as i32;
            }
            let basic = Basic {
                access: if case == 3 {
                    SYNCHRONIZE | 2
                } else {
                    SYNCHRONIZE
                },
                ..Basic::default()
            };
            unsafe {
                buffer.cast::<Basic>().write(basic);
                returned.write(size_of::<Basic>() as u32)
            };
            return 0;
        }
        if class != 2 || case == 4 {
            return 0xc0000002u32 as i32;
        }
        if bytes < (size_of::<Unicode>() + 10) as u32 {
            return 0xc0000004u32 as i32;
        }
        let name = unsafe { buffer.cast::<u8>().add(size_of::<Unicode>()).cast::<u16>() };
        let characters = if case == 5 {
            [77, 117, 116, 101, 120]
        } else {
            [69, 118, 101, 110, 116]
        };
        unsafe { std::ptr::copy_nonoverlapping(characters.as_ptr(), name, 5) };
        let unicode = Unicode {
            length: 10,
            maximum: 10,
            buffer: if case == 6 { std::ptr::null() } else { name },
        };
        unsafe {
            buffer.cast::<Unicode>().write(unicode);
            returned.write(if case == 7 {
                1
            } else {
                (size_of::<Unicode>() + 10) as u32
            })
        };
        0
    }
    #[test]
    fn c02_query_errors_type_and_granted_rights_fail_closed() {
        for (case, wanted) in [
            (1, EventError::Invalid),
            (2, EventError::Unavailable),
            (3, EventError::Invalid),
            (4, EventError::Unavailable),
            (5, EventError::Invalid),
            (6, EventError::Invalid),
            (7, EventError::Unavailable),
        ] {
            // SAFETY: synthetic table only writes the checked caller buffers. This
            // function cannot wait, acquire, close or call any device transport.
            assert_eq!(
                unsafe { validate_properties(case as RawHandle, synthetic_query) },
                Err(wanted)
            );
        }
        assert_eq!(
            unsafe { validate_properties(8usize as RawHandle, synthetic_query) },
            Ok(())
        );
    }
}
