#include <ntddk.h>
#include "probe.h"

__declspec(noreturn) void win_cache_probe_panic(void) {
    /* Fatal defect path only. This build-only image is never loaded by CI. */
    KeBugCheckEx(MANUALLY_INITIATED_CRASH, 0xD09, 0, 0, 0);
}

DRIVER_INITIALIZE DriverEntry;
NTSTATUS DriverEntry(PDRIVER_OBJECT driver, PUNICODE_STRING registry) {
    /* No pointer access, dispatch registration, device creation, or attach. */
    WinCacheRangeReply r = win_cache_probe_range(driver == NULL, registry == NULL, 1);
    return r.status == 0 ? STATUS_NOT_SUPPORTED : STATUS_INVALID_PARAMETER;
}
