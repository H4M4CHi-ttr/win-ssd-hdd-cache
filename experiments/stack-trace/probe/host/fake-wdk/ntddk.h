#ifndef WIN_CACHE_P04_FAKE_NTDDK_H
#define WIN_CACHE_P04_FAKE_NTDDK_H
/* Deliberately partial host simulation, NOT Windows layout/IRQL emulation. */
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include "platform.h"
/* MSVC's ordinary C runtime already provides SAL. Keep it when present;
 * portable hosts need only these no-op declaration annotations. */
#ifndef _In_
#define _In_
#endif
#ifndef _In_opt_
#define _In_opt_
#endif
#ifndef _Out_
#define _Out_
#endif
#ifndef _Out_opt_
#define _Out_opt_
#endif
#ifndef _Inout_
#define _Inout_
#endif
#ifndef _Use_decl_annotations_
#define _Use_decl_annotations_
#endif
#ifndef _IRQL_requires_max_
#define _IRQL_requires_max_(value)
#endif
#ifndef _IRQL_requires_
#define _IRQL_requires_(value)
#endif
#define UNREFERENCED_PARAMETER(value) ((void)(value))
#define RtlZeroMemory(pointer, bytes) ((void)memset((pointer), 0, (bytes)))
#define RtlCopyMemory(to, from, bytes) ((void)memcpy((to), (from), (bytes)))
#define FIELD_OFFSET(type, field) ((int32_t)offsetof(type, field))
#define TRUE ((uint8_t)1)
#define FALSE ((uint8_t)0)
#define NT_SUCCESS(value) ((int32_t)(value) >= 0)
#define PASSIVE_LEVEL 0u
#define APC_LEVEL 1u
#define DISPATCH_LEVEL 2u
#define IO_NO_INCREMENT 0
#define Executive 0
#define KernelMode 0
#define NotificationEvent 0
#define STATUS_SUCCESS ((int32_t)0)
#define STATUS_PENDING ((int32_t)0x103)
#define STATUS_CONTINUE_COMPLETION STATUS_SUCCESS
#define STATUS_MORE_PROCESSING_REQUIRED ((int32_t)0xc0000016u)
#define STATUS_UNSUCCESSFUL ((int32_t)0xc0000001u)
#define STATUS_INVALID_PARAMETER ((int32_t)0xc000000du)
#define STATUS_INVALID_DEVICE_REQUEST ((int32_t)0xc0000010u)
#define STATUS_ACCESS_DENIED ((int32_t)0xc0000022u)
#define STATUS_BUFFER_TOO_SMALL ((int32_t)0xc0000023u)
#define STATUS_OBJECT_NAME_NOT_FOUND ((int32_t)0xc0000034u)
#define STATUS_SHARING_VIOLATION ((int32_t)0xc0000043u)
#define STATUS_DELETE_PENDING ((int32_t)0xc0000056u)
#define STATUS_INSUFFICIENT_RESOURCES ((int32_t)0xc000009au)
#define STATUS_DEVICE_NOT_READY ((int32_t)0xc00000a3u)
#define STATUS_NOT_SUPPORTED ((int32_t)0xc00000bbu)
#define STATUS_CANCELLED ((int32_t)0xc0000120u)
#define STATUS_INVALID_DEVICE_STATE ((int32_t)0xc0000184u)
#define STATUS_DEVICE_REMOVED ((int32_t)0xc00002b6u)
#define OBJ_CASE_INSENSITIVE 0x40u
#define OBJ_KERNEL_HANDLE 0x200u
#define KEY_READ 0x20019u
#define KEY_QUERY_VALUE 1u
#define REG_SZ 1u
#define FILE_DEVICE_DISK 7u
#define FILE_DEVICE_UNKNOWN 0x22u
#define FILE_DEVICE_SECURE_OPEN 0x100u
#define FILE_REMOVABLE_MEDIA 1u
#define DO_BUFFERED_IO 4u
#define DO_DIRECT_IO 0x10u
#define DO_DEVICE_INITIALIZING 0x80u
#define DO_POWER_PAGABLE 0x2000u
#define DO_POWER_INRUSH 0x4000u
#define IRP_MJ_CREATE 0x00u
#define IRP_MJ_CLOSE 0x02u
#define IRP_MJ_READ 0x03u
#define IRP_MJ_WRITE 0x04u
#define IRP_MJ_FLUSH_BUFFERS 0x09u
#define IRP_MJ_DEVICE_CONTROL 0x0eu
#define IRP_MJ_INTERNAL_DEVICE_CONTROL 0x0fu
#define IRP_MJ_CLEANUP 0x12u
#define IRP_MJ_POWER 0x16u
#define IRP_MJ_PNP 0x1bu
#define IRP_MJ_MAXIMUM_FUNCTION 0x1bu
#define IRP_MN_START_DEVICE 0x00u
#define IRP_MN_QUERY_REMOVE_DEVICE 0x01u
#define IRP_MN_REMOVE_DEVICE 0x02u
#define IRP_MN_CANCEL_REMOVE_DEVICE 0x03u
#define IRP_MN_STOP_DEVICE 0x04u
#define IRP_MN_QUERY_STOP_DEVICE 0x05u
#define IRP_MN_CANCEL_STOP_DEVICE 0x06u
#define IRP_MN_DEVICE_USAGE_NOTIFICATION 0x16u
#define IRP_MN_SURPRISE_REMOVAL 0x17u
typedef uint8_t UCHAR, BOOLEAN, KIRQL, *PKIRQL;
typedef uint16_t USHORT, WCHAR, *PWSTR;
typedef const WCHAR *PCWSTR;
typedef uint32_t ULONG, *PULONG, LCID, DEVICE_TYPE, ACCESS_MASK, DEVPROPTYPE, *PDEVPROPTYPE;
typedef int32_t LONG, NTSTATUS;
typedef uint64_t ULONGLONG, ULONG_PTR;
typedef int64_t LONGLONG;
typedef void VOID, *PVOID, *HANDLE, **PHANDLE;
typedef struct { LONGLONG QuadPart; } LARGE_INTEGER, *PLARGE_INTEGER;
typedef struct { USHORT Length, MaximumLength; PWSTR Buffer; } UNICODE_STRING, *PUNICODE_STRING;
typedef struct { ULONG Data1; USHORT Data2, Data3; UCHAR Data4[8]; } GUID;
typedef struct { GUID fmtid; ULONG pid; } DEVPROPKEY;
typedef struct {
    ULONG Length; HANDLE RootDirectory; PUNICODE_STRING ObjectName; ULONG Attributes;
    PVOID SecurityDescriptor; PVOID SecurityQualityOfService;
} OBJECT_ATTRIBUTES, *POBJECT_ATTRIBUTES;
#define InitializeObjectAttributes(out,name,attrs,root,security) do { \
    (out)->Length = (ULONG)sizeof(*(out)); (out)->RootDirectory = (root); \
    (out)->ObjectName = (name); (out)->Attributes = (attrs); \
    (out)->SecurityDescriptor = (security); (out)->SecurityQualityOfService = NULL; \
} while (0)
typedef enum { KeyValueBasicInformation, KeyValueFullInformation, KeyValuePartialInformation } KEY_VALUE_INFORMATION_CLASS;
typedef struct { ULONG TitleIndex, Type, DataLength; UCHAR Data[1]; } KEY_VALUE_PARTIAL_INFORMATION, *PKEY_VALUE_PARTIAL_INFORMATION;
typedef struct _FILE_OBJECT { UNICODE_STRING FileName; } FILE_OBJECT, *PFILE_OBJECT;
typedef struct { NTSTATUS Status; ULONG_PTR Information; } IO_STATUS_BLOCK;
typedef enum { DeviceUsageTypeUndefined, DeviceUsageTypePaging, DeviceUsageTypeHibernation,
    DeviceUsageTypeDumpFile, DeviceUsageTypeBoot, DeviceUsageTypePostDisplay,
    DeviceUsageTypeGuestAssigned, DeviceUsageTypeInlineCryptoEngine } DEVICE_USAGE_NOTIFICATION_TYPE;
typedef struct _IO_STACK_LOCATION {
    UCHAR MajorFunction, MinorFunction, Flags, Control;
    union {
        struct { ULONG Length, Key; LARGE_INTEGER ByteOffset; } Read;
        struct { ULONG Length, Key; LARGE_INTEGER ByteOffset; } Write;
        struct { ULONG OutputBufferLength, InputBufferLength, IoControlCode; PVOID Type3InputBuffer; } DeviceIoControl;
        struct { BOOLEAN InPath; DEVICE_USAGE_NOTIFICATION_TYPE Type; } UsageNotification;
        UCHAR Opaque[32];
    } Parameters;
    PFILE_OBJECT FileObject;
} IO_STACK_LOCATION, *PIO_STACK_LOCATION;
struct _DEVICE_OBJECT;
struct _DRIVER_OBJECT;
struct _IRP;
typedef struct _DEVICE_OBJECT DEVICE_OBJECT, *PDEVICE_OBJECT;
typedef struct _DRIVER_OBJECT DRIVER_OBJECT, *PDRIVER_OBJECT;
typedef struct _IRP IRP, *PIRP;
typedef NTSTATUS DRIVER_INITIALIZE(PDRIVER_OBJECT, PUNICODE_STRING);
typedef NTSTATUS DRIVER_ADD_DEVICE(PDRIVER_OBJECT, PDEVICE_OBJECT);
typedef VOID DRIVER_UNLOAD(PDRIVER_OBJECT);
typedef NTSTATUS DRIVER_DISPATCH(PDEVICE_OBJECT, PIRP);
typedef NTSTATUS IO_COMPLETION_ROUTINE(PDEVICE_OBJECT, PIRP, PVOID);
typedef DRIVER_ADD_DEVICE *PDRIVER_ADD_DEVICE;
typedef DRIVER_DISPATCH *PDRIVER_DISPATCH;
typedef IO_COMPLETION_ROUTINE *PIO_COMPLETION_ROUTINE;
typedef struct { PDRIVER_ADD_DEVICE AddDevice; } DRIVER_EXTENSION;
struct _DRIVER_OBJECT {
    PDEVICE_OBJECT DeviceObject;
    DRIVER_EXTENSION *DriverExtension;
    DRIVER_UNLOAD *DriverUnload;
    PDRIVER_DISPATCH MajorFunction[IRP_MJ_MAXIMUM_FUNCTION + 1];
};
struct _DEVICE_OBJECT {
    PDRIVER_OBJECT DriverObject;
    PDEVICE_OBJECT NextDevice, AttachedDevice;
    PVOID DeviceExtension;
    ULONG Flags, Characteristics, DeviceType, AlignmentRequirement;
    UCHAR StackSize;
};
struct _IRP {
    IO_STATUS_BLOCK IoStatus;
    BOOLEAN PendingReturned, Cancel;
    struct { PVOID SystemBuffer; } AssociatedIrp;
    PVOID UserBuffer, MdlAddress;
    IO_STACK_LOCATION Stack[2];
    PIO_COMPLETION_ROUTINE Completion;
    PVOID CompletionContext;
    BOOLEAN OnSuccess, OnError, OnCancel, Copied, Skipped;
};
typedef struct { P04HostMutex *mutex; unsigned owner; } KSPIN_LOCK, *PKSPIN_LOCK;
typedef struct { P04HostMutex *mutex; P04HostCond *condition; int signaled; } KEVENT, *PKEVENT;
typedef struct {
    P04HostMutex *mutex; P04HostCond *condition;
    const void *tags[512]; unsigned count; int removed;
} IO_REMOVE_LOCK, *PIO_REMOVE_LOCK;

void RtlInitUnicodeString(PUNICODE_STRING, PCWSTR);
BOOLEAN RtlEqualUnicodeString(const UNICODE_STRING *, const UNICODE_STRING *, BOOLEAN);
NTSTATUS ZwOpenKey(PHANDLE, ACCESS_MASK, POBJECT_ATTRIBUTES);
NTSTATUS ZwQueryValueKey(HANDLE, PUNICODE_STRING, KEY_VALUE_INFORMATION_CLASS, PVOID, ULONG, PULONG);
NTSTATUS ZwClose(HANDLE);
NTSTATUS IoGetDevicePropertyData(PDEVICE_OBJECT, const DEVPROPKEY *, LCID, ULONG, ULONG, PVOID, PULONG, PDEVPROPTYPE);
NTSTATUS IoCreateDevice(PDRIVER_OBJECT, ULONG, PUNICODE_STRING, DEVICE_TYPE, ULONG, BOOLEAN, PDEVICE_OBJECT *);
NTSTATUS IoCreateSymbolicLink(PUNICODE_STRING, PUNICODE_STRING);
NTSTATUS IoDeleteSymbolicLink(PUNICODE_STRING);
NTSTATUS IoAttachDeviceToDeviceStackSafe(PDEVICE_OBJECT, PDEVICE_OBJECT, PDEVICE_OBJECT *);
void IoDetachDevice(PDEVICE_OBJECT);
void IoDeleteDevice(PDEVICE_OBJECT);
void IoInitializeRemoveLock(PIO_REMOVE_LOCK, ULONG, ULONG, ULONG);
NTSTATUS IoAcquireRemoveLock(PIO_REMOVE_LOCK, const void *);
void IoReleaseRemoveLock(PIO_REMOVE_LOCK, const void *);
void IoReleaseRemoveLockAndWait(PIO_REMOVE_LOCK, const void *);
void KeInitializeSpinLock(PKSPIN_LOCK);
void KeAcquireSpinLock(PKSPIN_LOCK, PKIRQL);
void KeReleaseSpinLock(PKSPIN_LOCK, KIRQL);
void KeAcquireSpinLockAtDpcLevel(PKSPIN_LOCK);
void KeReleaseSpinLockFromDpcLevel(PKSPIN_LOCK);
KIRQL KeGetCurrentIrql(void);
void KeInitializeEvent(PKEVENT, int, BOOLEAN);
LONG KeSetEvent(PKEVENT, LONG, BOOLEAN);
NTSTATUS KeWaitForSingleObject(PVOID, int, int, BOOLEAN, PLARGE_INTEGER);
PIO_STACK_LOCATION IoGetCurrentIrpStackLocation(PIRP);
void IoCopyCurrentIrpStackLocationToNext(PIRP);
void IoSkipCurrentIrpStackLocation(PIRP);
void IoSetCompletionRoutine(PIRP, PIO_COMPLETION_ROUTINE, PVOID, BOOLEAN, BOOLEAN, BOOLEAN);
NTSTATUS IoCallDriver(PDEVICE_OBJECT, PIRP);
void IoCompleteRequest(PIRP, UCHAR);
void IoMarkIrpPending(PIRP);

/* Host fixture API. None of these names are available to actual WDK source. */
typedef enum {
    H_CREATE_FDO = 1, H_INITIALIZE_FDO, H_CREATE_CDO, H_CREATE_LINK,
    H_ATTACH_OK, H_ATTACH_FAIL, H_DELETE_LINK, H_DELETE_CDO, H_DELETE_FDO,
    H_DETACH, H_SPIN_RELEASE, H_REMOVE_RELEASE_BEFORE, H_REMOVE_RELEASE_AFTER,
    H_ANDWAIT_ENTER, H_ANDWAIT_END, H_CALL_RETURN_BEFORE, H_SPIN_ACQUIRE_BEFORE,
    H_EVENT_WAIT, H_FORWARD, H_COMPLETE, H_MARK_PENDING
} HOperation;
typedef struct {
    int delayed;
    NTSTATUS returned, completed;
    ULONG_PTR information;
    BOOLEAN pending, cancel;
    KIRQL completion_irql;
} HLowerPlan;
typedef struct {
    NTSTATUS status;
    ULONG_PTR information;
    unsigned completed, forwarded, marked, copied, installed;
} HResult;
void h_reset(void);
void h_destroy(void);
PDRIVER_OBJECT h_driver(void);
PDEVICE_OBJECT h_pdo(void);
PDEVICE_OBJECT h_lower(void);
PDEVICE_OBJECT h_fdo(void);
PDEVICE_OBJECT h_cdo(void);
void h_set_selector(const WCHAR *, ULONG, ULONG, NTSTATUS);
void h_set_property(const WCHAR *, ULONG, DEVPROPTYPE, NTSTATUS);
void h_fail_next(HOperation);
void h_inject_attach_dispatch(int);
void h_inject_early_create(int);
void h_plan(PIRP, HLowerPlan);
PIRP h_request(UCHAR, UCHAR);
NTSTATUS h_dispatch(PDEVICE_OBJECT, PIRP);
void h_complete_lower(PIRP);
HResult h_result(PIRP);
void *h_buffer(size_t);
void *h_payload_guard(void);
void h_set_irql(KIRQL);
void h_thread_label(unsigned);
void h_gate_arm(HOperation, const void *, unsigned);
void h_gate_wait(void);
void h_gate_release(void);
void h_wait_operation(HOperation, const void *, unsigned);
unsigned h_count_operation(HOperation, const void *);
void h_expect_setup(const HOperation *, size_t);
unsigned h_live_devices(void);
unsigned h_open_handles(void);
unsigned h_pending_requests(void);
unsigned h_remove_holds(PIO_REMOVE_LOCK);
int h_device_freed(PDEVICE_OBJECT);
int h_device_delete_pending(PDEVICE_OBJECT);
void h_expect(int, const char *);
#endif
