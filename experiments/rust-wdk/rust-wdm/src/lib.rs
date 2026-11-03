//! Compile/link proof only. Deliberately refuses every attempted load.
#![no_std]

extern crate wdk_panic;

use wdk_sys::{
    DRIVER_OBJECT, NTSTATUS, PCUNICODE_STRING, STATUS_INVALID_PARAMETER, STATUS_NOT_SUPPORTED,
};

// SAFETY: Sole DriverEntry export in this isolated image. OS ABI pointers are
// never dereferenced or retained. No device, dispatch registration or attachment.
#[unsafe(export_name = "DriverEntry")]
pub extern "system" fn driver_entry(
    driver: *mut DRIVER_OBJECT,
    registry: PCUNICODE_STRING,
) -> NTSTATUS {
    // Depend on inputs without touching their memory; every branch is failure.
    let result =
        wdk_probe_core::checked_range(driver.is_null() as u64, registry.is_null() as u64, 1);
    if result.status == 0 {
        STATUS_NOT_SUPPORTED
    } else {
        STATUS_INVALID_PARAMETER
    }
}
