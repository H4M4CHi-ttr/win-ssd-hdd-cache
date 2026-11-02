//! Isolated no_std ABI probe. Never a product driver or asynchronous boundary.
#![no_std]

use core::panic::PanicInfo;
use wdk_probe_core::{RangeReply, checked_range};

// SAFETY: Unique experiment-only symbol, fixed C ABI scalars/fully initialized
// repr(C) value. No pointers, callbacks, aliases, retention or unwind.
#[unsafe(no_mangle)]
pub extern "C" fn win_cache_probe_range(offset: u64, length: u64, capacity: u64) -> RangeReply {
    checked_range(offset, length, capacity)
}

unsafe extern "C" {
    // Linked exactly once by the host harness (abort) or C WDK shim (bugcheck).
    fn win_cache_probe_panic() -> !;
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    // SAFETY: Both supplied C implementations are nonreturning and accept no
    // pointers or data. A panic is a defect, never recovered or unwound.
    unsafe { win_cache_probe_panic() }
}
