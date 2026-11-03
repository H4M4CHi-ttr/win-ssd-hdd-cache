#ifndef WIN_CACHE_P04_FAKE_WDMSEC_H
#define WIN_CACHE_P04_FAKE_WDMSEC_H
#include <ntddk.h>
NTSTATUS WdmlibIoCreateDeviceSecure(PDRIVER_OBJECT, ULONG, PUNICODE_STRING,
    DEVICE_TYPE, ULONG, BOOLEAN, const UNICODE_STRING *, const GUID *, PDEVICE_OBJECT *);
#define IoCreateDeviceSecure WdmlibIoCreateDeviceSecure
#endif
