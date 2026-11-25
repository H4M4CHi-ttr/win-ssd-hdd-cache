//! Static package inspection and a private compile/link-only Windows capsule.
//! No device enumeration, registration, service or registry entry point is public.
#![deny(unsafe_code)]

#[allow(unsafe_code)]
#[cfg(any(all(windows, target_arch = "x86_64"), test))]
mod boundary;
pub mod package;

/// Retain native wrapper bodies and imports without invoking a Windows API.
/// This does not expose a function pointer, handle, or apply capability.
pub fn retain_link_proof() {
    #[cfg(all(windows, target_arch = "x86_64"))]
    boundary::retain_link_proof();
}
