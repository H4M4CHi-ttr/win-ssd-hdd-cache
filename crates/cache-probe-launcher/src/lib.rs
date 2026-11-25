//! Limited Win32 process/event boundary, shared by production and synthetic IPC.
#![deny(unsafe_code)]
#[cfg(all(
    windows,
    target_arch = "x86_64",
    any(feature = "experimental-live", feature = "synthetic-ipc")
))]
#[allow(unsafe_code)]
#[path = "../../cache-probe-collector/src/abort_event.rs"]
pub mod abort_event;
#[cfg(all(
    windows,
    target_arch = "x86_64",
    any(feature = "experimental-live", feature = "synthetic-ipc")
))]
#[allow(unsafe_code)]
mod windows;
#[cfg(all(
    windows,
    target_arch = "x86_64",
    any(feature = "experimental-live", feature = "synthetic-ipc")
))]
pub use windows::*;
pub mod supervision;
