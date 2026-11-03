/* P04 is a never-loaded observation experiment, not a product storage driver.
 * This complete lifecycle TU is compiled unchanged by the host DDI schedules
 * and by the pinned WDK build. There is no HOST-only lifecycle implementation.
 * No allocation, payload access, originated storage I/O or wait is performed
 * by ordinary dispatch/completion or a control transaction.
 */
#include <initguid.h>
#include "p04_driver.h"

static P04GlobalState g_P04;
static const GUID P04ControlClass = {
    0xd475d19au, 0x6bb4u, 0x4d16u, {0x9bu,0x66u,0x48u,0x3eu,0x40u,0x26u,0xfau,0x11u}
};

static void P04DeviceNames(PUNICODE_STRING device, PUNICODE_STRING link)
{
    RtlInitUnicodeString(device, L"\\Device\\WinSsdHddCacheProbeP04");
    RtlInitUnicodeString(link, L"\\DosDevices\\WinSsdHddCacheProbeP04");
}

/* Counted UTF-16, with exactly one final NUL; no truncation or normalization. */
static int P04ValidString(const WCHAR *value, ULONG bytes)
{
    ULONG units, i;
    if (bytes < 2u * sizeof(WCHAR) || bytes > P04_SELECTOR_UNITS * sizeof(WCHAR)
        || (bytes % sizeof(WCHAR)) != 0) return 0;
    units = bytes / (ULONG)sizeof(WCHAR);
    if (value[units - 1u] != 0) return 0;
    for (i = 0; i + 1u < units; ++i) if (value[i] == 0) return 0;
    return 1;
}

static void P04ReadSelector(PUNICODE_STRING registry_path)
{
    HANDLE service = NULL, parameters = NULL;
    OBJECT_ATTRIBUTES attributes;
    UNICODE_STRING name;
    NTSTATUS status;
    ULONG returned = 0;
    const ULONG header = (ULONG)FIELD_OFFSET(KEY_VALUE_PARTIAL_INFORMATION, Data);
    union {
        KEY_VALUE_PARTIAL_INFORMATION alignment;
        UCHAR bytes[sizeof(KEY_VALUE_PARTIAL_INFORMATION) + P04_SELECTOR_UNITS * sizeof(WCHAR)];
    } storage;
    PKEY_VALUE_PARTIAL_INFORMATION value = (PKEY_VALUE_PARTIAL_INFORMATION)storage.bytes;

    InitializeObjectAttributes(&attributes, registry_path,
        OBJ_KERNEL_HANDLE | OBJ_CASE_INSENSITIVE, NULL, NULL);
    status = ZwOpenKey(&service, KEY_READ, &attributes);
    if (!NT_SUCCESS(status)) return;
    RtlInitUnicodeString(&name, L"Parameters");
    InitializeObjectAttributes(&attributes, &name,
        OBJ_KERNEL_HANDLE | OBJ_CASE_INSENSITIVE, service, NULL);
    status = ZwOpenKey(&parameters, KEY_QUERY_VALUE, &attributes);
    if (NT_SUCCESS(status)) {
        RtlInitUnicodeString(&name, L"TargetInstanceId");
        status = ZwQueryValueKey(parameters, &name, KeyValuePartialInformation,
            storage.bytes, (ULONG)sizeof(storage.bytes), &returned);
        if (NT_SUCCESS(status) && returned >= header && returned <= sizeof(storage.bytes)
            && value->Type == REG_SZ && value->DataLength <= returned - header
            && P04ValidString((const WCHAR *)value->Data, value->DataLength)) {
            RtlCopyMemory(g_P04.Selector, value->Data, value->DataLength);
            g_P04.SelectorBytes = (USHORT)(value->DataLength - sizeof(WCHAR));
            g_P04.SelectorValid = TRUE;
        }
        (void)ZwClose(parameters);
    }
    (void)ZwClose(service);
}

_Use_decl_annotations_
NTSTATUS DriverEntry(PDRIVER_OBJECT driver, PUNICODE_STRING registry_path)
{
    ULONG i;
    RtlZeroMemory(&g_P04, sizeof(g_P04));
    KeInitializeSpinLock(&g_P04.PublicationLock);
    P04ReadSelector(registry_path);
    for (i = 0; i <= IRP_MJ_MAXIMUM_FUNCTION; ++i) driver->MajorFunction[i] = P04Dispatch;
    driver->MajorFunction[IRP_MJ_PNP] = P04DispatchPnp;
    driver->MajorFunction[IRP_MJ_POWER] = P04DispatchPower;
    driver->DriverExtension->AddDevice = P04AddDevice;
    driver->DriverUnload = P04Unload;
    return STATUS_SUCCESS;
}

_Use_decl_annotations_
VOID P04Unload(PDRIVER_OBJECT driver)
{
    /* Device/remove/open-file references, not an unload-time forced delete,
     * own lifetime. PnP removal has already disposed of the binding objects. */
    UNREFERENCED_PARAMETER(driver);
    RtlZeroMemory(g_P04.Selector, sizeof(g_P04.Selector));
    g_P04.SelectorValid = FALSE;
    g_P04.SelectorBytes = 0;
}

_Use_decl_annotations_
NTSTATUS P04AddDevice(PDRIVER_OBJECT driver, PDEVICE_OBJECT pdo)
{
    WCHAR instance[P04_SELECTOR_UNITS];
    ULONG returned = 0;
    DEVPROPTYPE type = 0;
    UNICODE_STRING actual, selected, device_name, link_name, sddl;
    NTSTATUS status;
    KIRQL previous;
    PDEVICE_OBJECT filter = NULL, control = NULL;
    P04FilterExtension *extension;
    P04ControlExtension *control_extension;
    BOOLEAN link_created = FALSE;

    if (!g_P04.SelectorValid) return STATUS_SUCCESS;
    status = IoGetDevicePropertyData(pdo, &DEVPKEY_Device_InstanceId, 0, 0,
        (ULONG)sizeof(instance), instance, &returned, &type);
    if (!NT_SUCCESS(status) || type != DEVPROP_TYPE_STRING || !P04ValidString(instance, returned))
        return STATUS_SUCCESS;
    actual.Buffer = instance;
    actual.Length = (USHORT)(returned - sizeof(WCHAR));
    actual.MaximumLength = (USHORT)returned;
    selected.Buffer = g_P04.Selector;
    selected.Length = g_P04.SelectorBytes;
    selected.MaximumLength = (USHORT)(g_P04.SelectorBytes + sizeof(WCHAR));
    if (!RtlEqualUnicodeString(&actual, &selected, TRUE)) return STATUS_SUCCESS;

    KeAcquireSpinLock(&g_P04.PublicationLock, &previous);
    if (g_P04.Binding != P04_UNBOUND) {
        KeReleaseSpinLock(&g_P04.PublicationLock, previous);
        return STATUS_SUCCESS;
    }
    g_P04.Binding = P04_PREPARING;
    KeReleaseSpinLock(&g_P04.PublicationLock, previous);

    status = IoCreateDevice(driver, (ULONG)sizeof(P04FilterExtension), NULL,
        FILE_DEVICE_DISK, 0, FALSE, &filter);
    if (!NT_SUCCESS(status)) goto rollback;
    extension = (P04FilterExtension *)filter->DeviceExtension;
    RtlZeroMemory(extension, sizeof(*extension));
    extension->Role = P04_ROLE_FILTER;
    extension->PnpState = P04_NOT_STARTED;
    IoInitializeRemoveLock(&extension->RemoveLock, 0x34305057u, 0, 0);
    KeInitializeSpinLock(&extension->StateLock);
    win_cache_p04_init(&extension->Recorder);

    P04DeviceNames(&device_name, &link_name);
    RtlInitUnicodeString(&sddl, L"D:P(A;;GA;;;SY)(A;;GA;;;BA)");
    status = IoCreateDeviceSecure(driver, (ULONG)sizeof(P04ControlExtension), &device_name,
        FILE_DEVICE_UNKNOWN, FILE_DEVICE_SECURE_OPEN, FALSE, &sddl, &P04ControlClass, &control);
    if (!NT_SUCCESS(status)) goto rollback;
    control_extension = (P04ControlExtension *)control->DeviceExtension;
    RtlZeroMemory(control_extension, sizeof(*control_extension));
    control_extension->Role = P04_ROLE_CONTROL;
    status = IoCreateSymbolicLink(&link_name, &device_name);
    if (!NT_SUCCESS(status)) goto rollback;
    link_created = TRUE;

    /* Last fallible preparation step. The real extension field is assigned
     * atomically by Safe; initialized state can service immediate dispatch. */
    status = IoAttachDeviceToDeviceStackSafe(filter, pdo, &extension->Lower);
    if (!NT_SUCCESS(status)) goto rollback;

    filter->Flags |= extension->Lower->Flags & (DO_BUFFERED_IO | DO_DIRECT_IO);
    filter->Flags &= ~(DO_POWER_PAGABLE | DO_POWER_INRUSH);
    filter->Characteristics = extension->Lower->Characteristics | FILE_DEVICE_SECURE_OPEN;
    filter->AlignmentRequirement = extension->Lower->AlignmentRequirement;
    control->Flags |= DO_BUFFERED_IO;
    KeAcquireSpinLock(&g_P04.PublicationLock, &previous);
    g_P04.Target = filter;
    g_P04.Control = control;
    g_P04.Binding = P04_ATTACHED;
    filter->Flags &= ~DO_DEVICE_INITIALIZING;
    control->Flags &= ~DO_DEVICE_INITIALIZING;
    KeReleaseSpinLock(&g_P04.PublicationLock, previous);
    return STATUS_SUCCESS;

rollback:
    /* No successful attachment has occurred on any rollback edge. */
    if (link_created) (void)IoDeleteSymbolicLink(&link_name);
    if (control != NULL) IoDeleteDevice(control);
    if (filter != NULL) IoDeleteDevice(filter);
    KeAcquireSpinLock(&g_P04.PublicationLock, &previous);
    g_P04.Binding = P04_UNBOUND;
    KeReleaseSpinLock(&g_P04.PublicationLock, previous);
    return STATUS_SUCCESS; /* Optional observer failure must not fail the disk. */
}

static NTSTATUS P04CompleteControl(PIRP irp, NTSTATUS status, ULONG information)
{
    irp->IoStatus.Status = status;
    irp->IoStatus.Information = information;
    IoCompleteRequest(irp, IO_NO_INCREMENT);
    return status;
}

static NTSTATUS P04Acquire(P04FilterExtension *extension, PIRP irp)
{
    return IoAcquireRemoveLock(&extension->RemoveLock, irp);
}

static int P04IsControl(PDEVICE_OBJECT device)
{
    /* Role is the ULONG first member of either extension. Read that member
     * directly rather than aliasing a control extension as the filter type. */
    return *(const ULONG *)device->DeviceExtension == P04_ROLE_CONTROL;
}

static NTSTATUS P04ForwardRelease(P04FilterExtension *extension, PIRP irp)
{
    IoCopyCurrentIrpStackLocationToNext(irp);
    IoSetCompletionRoutine(irp, P04ReleaseCompletion, extension, TRUE, TRUE, TRUE);
    /* Completion may delete the last reference before this call returns. */
    return IoCallDriver(extension->Lower, irp);
}

_Use_decl_annotations_
NTSTATUS P04ReleaseCompletion(PDEVICE_OBJECT device, PIRP irp, PVOID context)
{
    P04FilterExtension *extension = (P04FilterExtension *)context;
    UNREFERENCED_PARAMETER(device);
    if (irp->PendingReturned) IoMarkIrpPending(irp);
    IoReleaseRemoveLock(&extension->RemoveLock, irp);
    return STATUS_CONTINUE_COMPLETION;
}

_Use_decl_annotations_
NTSTATUS P04ObservedCompletion(PDEVICE_OBJECT device, PIRP irp, PVOID context)
{
    const P04CallbackContext *callback = (const P04CallbackContext *)context;
    P04FilterExtension *extension = callback->Extension;
    const WinCacheP04Token token = callback->Token;
    WinCacheP02Submit saved;
    WinCacheP02Completion snapshot;
    const WinCacheP02Completion *complete = NULL;
    KIRQL previous;
    UNREFERENCED_PARAMETER(device);

    KeAcquireSpinLock(&extension->StateLock, &previous);
    if (win_cache_p04_saved_submit(&extension->Recorder, token, &saved)
        && win_cache_p02_snapshot_complete(&irp->IoStatus, &saved, &snapshot) == WIN_CACHE_P02_OK)
        complete = &snapshot;
    win_cache_p04_complete(&extension->Recorder, token, complete);
    (void)win_cache_p04_seal(&extension->Recorder);
    KeReleaseSpinLock(&extension->StateLock, previous);
    /* The context slot may already belong to a different IRP. Only copied
     * extension/token locals may be used after retirement. */
    if (irp->PendingReturned) IoMarkIrpPending(irp);
    IoReleaseRemoveLock(&extension->RemoveLock, irp);
    return STATUS_CONTINUE_COMPLETION;
}

_Use_decl_annotations_
NTSTATUS P04Dispatch(PDEVICE_OBJECT device, PIRP irp)
{
    P04FilterExtension *extension = (P04FilterExtension *)device->DeviceExtension;
    PIO_STACK_LOCATION stack;
    WinCacheP02Submit snapshot;
    WinCacheP04Token token;
    P04CallbackContext *callback = NULL;
    KIRQL previous;
    NTSTATUS status;
    if (P04IsControl(device)) return P04DispatchControl(device, irp);
    status = P04Acquire(extension, irp);
    if (!NT_SUCCESS(status)) return P04CompleteControl(irp, status, 0);
    stack = IoGetCurrentIrpStackLocation(irp);
    KeAcquireSpinLock(&extension->StateLock, &previous);
    if (stack->MajorFunction == IRP_MJ_CREATE && extension->PnpState == P04_REMOVE_PENDING) {
        KeReleaseSpinLock(&extension->StateLock, previous);
        IoReleaseRemoveLock(&extension->RemoveLock, irp);
        return P04CompleteControl(irp, STATUS_DELETE_PENDING, 0);
    }
    if (win_cache_p02_snapshot_submit(stack, &snapshot) == WIN_CACHE_P02_OK
        && win_cache_p04_submit(&extension->Recorder, &snapshot, &token)) {
        callback = &extension->Callbacks[token.slot];
        callback->Extension = extension;
        callback->Token = token;
    }
    KeReleaseSpinLock(&extension->StateLock, previous);
    if (callback == NULL) return P04ForwardRelease(extension, irp);
    IoCopyCurrentIrpStackLocationToNext(irp);
    IoSetCompletionRoutine(irp, P04ObservedCompletion, callback, TRUE, TRUE, TRUE);
    return IoCallDriver(extension->Lower, irp);
}

_Use_decl_annotations_
NTSTATUS P04SyncCompletion(PDEVICE_OBJECT device, PIRP irp, PVOID context)
{
    UNREFERENCED_PARAMETER(device);
    UNREFERENCED_PARAMETER(irp);
    (void)KeSetEvent((PKEVENT)context, IO_NO_INCREMENT, FALSE);
    /* The waiting dispatch owns this IRP. Unlike pass-through completion it
     * must NOT mark pending or continue upper completion here. */
    return STATUS_MORE_PROCESSING_REQUIRED;
}

static NTSTATUS P04SynchronousPnp(P04FilterExtension *extension, PIRP irp, UCHAR minor)
{
    KEVENT completed;
    NTSTATUS returned, status;
    KIRQL previous;
    KeInitializeEvent(&completed, NotificationEvent, FALSE);
    IoCopyCurrentIrpStackLocationToNext(irp);
    IoSetCompletionRoutine(irp, P04SyncCompletion, &completed, TRUE, TRUE, TRUE);
    returned = IoCallDriver(extension->Lower, irp);
    if (returned == STATUS_PENDING)
        (void)KeWaitForSingleObject(&completed, Executive, KernelMode, FALSE, NULL);
    status = irp->IoStatus.Status;
    KeAcquireSpinLock(&extension->StateLock, &previous);
    if (minor == IRP_MN_START_DEVICE) {
        if (NT_SUCCESS(status)) extension->PnpState = P04_STARTED;
    } else {
        if (minor == IRP_MN_CANCEL_REMOVE_DEVICE && extension->PnpState == P04_REMOVE_PENDING)
            extension->PnpState = extension->PreviousRemove;
        if (minor == IRP_MN_CANCEL_STOP_DEVICE && extension->PnpState == P04_STOP_PENDING)
            extension->PnpState = extension->PreviousStop;
        status = STATUS_SUCCESS;
        irp->IoStatus.Status = STATUS_SUCCESS;
    }
    KeReleaseSpinLock(&extension->StateLock, previous);
    IoCompleteRequest(irp, IO_NO_INCREMENT);
    IoReleaseRemoveLock(&extension->RemoveLock, irp);
    return status;
}

static NTSTATUS P04Remove(PDEVICE_OBJECT device, P04FilterExtension *extension, PIRP irp)
{
    PDEVICE_OBJECT control, lower = extension->Lower;
    UNICODE_STRING device_name, link_name;
    NTSTATUS returned;
    KIRQL previous;
    KeAcquireSpinLock(&g_P04.PublicationLock, &previous);
    control = g_P04.Control;
    g_P04.Target = NULL;
    g_P04.Control = NULL;
    g_P04.Binding = P04_RETIRED;
    KeReleaseSpinLock(&g_P04.PublicationLock, previous);
    KeAcquireSpinLock(&extension->StateLock, &previous);
    win_cache_p04_skip(&extension->Recorder, 0);
    win_cache_p04_stop(&extension->Recorder);
    (void)win_cache_p04_seal(&extension->Recorder);
    extension->PnpState = P04_DELETED;
    KeReleaseSpinLock(&extension->StateLock, previous);
    irp->IoStatus.Status = STATUS_SUCCESS;
    IoSkipCurrentIrpStackLocation(irp);
    returned = IoCallDriver(lower, irp);
    /* No IRP dereference after forwarding. Its tag is only an opaque value. */
    IoReleaseRemoveLockAndWait(&extension->RemoveLock, irp);
    P04DeviceNames(&device_name, &link_name);
    (void)IoDeleteSymbolicLink(&link_name);
    if (control != NULL) IoDeleteDevice(control);
    IoDetachDevice(lower);
    IoDeleteDevice(device);
    return returned;
}

_Use_decl_annotations_
NTSTATUS P04DispatchPnp(PDEVICE_OBJECT device, PIRP irp)
{
    P04FilterExtension *extension = (P04FilterExtension *)device->DeviceExtension;
    PIO_STACK_LOCATION stack;
    UCHAR minor;
    WinCacheP02U32 veto;
    NTSTATUS status;
    KIRQL previous;
    if (P04IsControl(device)) return P04DispatchControl(device, irp);
    status = P04Acquire(extension, irp);
    if (!NT_SUCCESS(status)) return P04CompleteControl(irp, status, 0);
    stack = IoGetCurrentIrpStackLocation(irp);
    minor = stack->MinorFunction;
    if (minor == IRP_MN_REMOVE_DEVICE) return P04Remove(device, extension, irp);
    veto = minor == IRP_MN_DEVICE_USAGE_NOTIFICATION && stack->Parameters.UsageNotification.InPath != FALSE;
    KeAcquireSpinLock(&extension->StateLock, &previous);
    win_cache_p04_skip(&extension->Recorder, veto);
    switch (minor) {
    case IRP_MN_QUERY_REMOVE_DEVICE:
        extension->PreviousRemove = extension->PnpState;
        extension->PnpState = P04_REMOVE_PENDING;
        break;
    case IRP_MN_QUERY_STOP_DEVICE:
        extension->PreviousStop = extension->PnpState;
        extension->PnpState = P04_STOP_PENDING;
        break;
    case IRP_MN_STOP_DEVICE:
        extension->PnpState = P04_STOPPED;
        win_cache_p04_stop(&extension->Recorder);
        (void)win_cache_p04_seal(&extension->Recorder);
        break;
    case IRP_MN_SURPRISE_REMOVAL:
        extension->PnpState = P04_SURPRISE_REMOVED;
        win_cache_p04_stop(&extension->Recorder);
        (void)win_cache_p04_seal(&extension->Recorder);
        break;
    default:
        break;
    }
    KeReleaseSpinLock(&extension->StateLock, previous);
    if (veto) {
        /* This driver handled and rejected the transition. NOT_SUPPORTED
         * denotes an unhandled PnP request, not this explicit policy veto. */
        irp->IoStatus.Status = STATUS_UNSUCCESSFUL;
        IoCompleteRequest(irp, IO_NO_INCREMENT);
        IoReleaseRemoveLock(&extension->RemoveLock, irp);
        return STATUS_UNSUCCESSFUL;
    }
    if (minor == IRP_MN_START_DEVICE || minor == IRP_MN_CANCEL_REMOVE_DEVICE || minor == IRP_MN_CANCEL_STOP_DEVICE)
        return P04SynchronousPnp(extension, irp, minor);
    if (minor == IRP_MN_QUERY_REMOVE_DEVICE || minor == IRP_MN_QUERY_STOP_DEVICE
        || minor == IRP_MN_STOP_DEVICE || minor == IRP_MN_SURPRISE_REMOVAL)
        irp->IoStatus.Status = STATUS_SUCCESS;
    return P04ForwardRelease(extension, irp);
}

_Use_decl_annotations_
NTSTATUS P04DispatchPower(PDEVICE_OBJECT device, PIRP irp)
{
    P04FilterExtension *extension = (P04FilterExtension *)device->DeviceExtension;
    NTSTATUS status;
    KIRQL previous;
    if (P04IsControl(device)) return P04DispatchControl(device, irp);
    status = P04Acquire(extension, irp);
    if (!NT_SUCCESS(status)) return P04CompleteControl(irp, status, 0);
    KeAcquireSpinLock(&extension->StateLock, &previous);
    win_cache_p04_skip(&extension->Recorder, 0);
    KeReleaseSpinLock(&extension->StateLock, previous);
    return P04ForwardRelease(extension, irp);
}

static NTSTATUS P04CreateControl(P04ControlExtension *control, PIRP irp, PFILE_OBJECT file)
{
    KIRQL previous;
    NTSTATUS status = STATUS_SUCCESS;
    KeAcquireSpinLock(&g_P04.PublicationLock, &previous);
    if (file == NULL || file->FileName.Length != 0) status = STATUS_INVALID_PARAMETER;
    else if (g_P04.Target == NULL)
        status = g_P04.Binding == P04_RETIRED ? STATUS_DELETE_PENDING : STATUS_DEVICE_NOT_READY;
    else if (control->OpenFile != NULL && control->OpenFile != file) status = STATUS_SHARING_VIOLATION;
    else control->OpenFile = file;
    KeReleaseSpinLock(&g_P04.PublicationLock, previous);
    return P04CompleteControl(irp, status, 0);
}

static NTSTATUS P04CleanupControl(P04ControlExtension *control, PIRP irp, PFILE_OBJECT file)
{
    P04FilterExtension *extension = NULL;
    KIRQL previous;
    KeAcquireSpinLock(&g_P04.PublicationLock, &previous);
    if (file != NULL && (control->OpenFile == file || control->Owner == file)) {
        if (control->OpenFile == file) control->OpenFile = NULL;
        if (control->Owner == file) control->Owner = NULL;
        if (g_P04.Target != NULL) {
            P04FilterExtension *candidate = (P04FilterExtension *)g_P04.Target->DeviceExtension;
            if (NT_SUCCESS(P04Acquire(candidate, irp))) {
                extension = candidate;
                KeAcquireSpinLockAtDpcLevel(&extension->StateLock);
                win_cache_p04_consumer_lost(&extension->Recorder);
                (void)win_cache_p04_seal(&extension->Recorder);
                KeReleaseSpinLockFromDpcLevel(&extension->StateLock);
            }
        }
    }
    KeReleaseSpinLock(&g_P04.PublicationLock, previous);
    if (extension != NULL) IoReleaseRemoveLock(&extension->RemoveLock, irp);
    return P04CompleteControl(irp, STATUS_SUCCESS, 0);
}

static NTSTATUS P04ControlCommand(P04ControlExtension *control, PIRP irp, PIO_STACK_LOCATION stack)
{
    WinCacheP04Request request;
    WinCacheP04Stats stats;
    WinCacheP04Record record;
    WinCacheP04WireResult decoded;
    P04FilterExtension *extension = NULL;
    WinCacheP02U8 *output = (WinCacheP02U8 *)irp->AssociatedIrp.SystemBuffer;
    WinCacheP02U32 count = 0;
    ULONG information = 0;
    NTSTATUS status;
    KIRQL previous;

    /* The complete request is copied before any aliased output is written. */
    decoded = win_cache_p04_decode_request(stack->Parameters.DeviceIoControl.IoControlCode,
        output, stack->Parameters.DeviceIoControl.InputBufferLength,
        stack->Parameters.DeviceIoControl.OutputBufferLength, &request);
    if (decoded != WIN_CACHE_P04_WIRE_OK) {
        status = decoded == WIN_CACHE_P04_WIRE_BAD_IOCTL ? STATUS_INVALID_DEVICE_REQUEST
            : decoded == WIN_CACHE_P04_WIRE_BAD_OUTPUT ? STATUS_BUFFER_TOO_SMALL : STATUS_INVALID_PARAMETER;
        return P04CompleteControl(irp, status, 0);
    }

    KeAcquireSpinLock(&g_P04.PublicationLock, &previous);
    if (stack->FileObject == NULL || control->OpenFile != stack->FileObject
        || (request.command != WIN_CACHE_P04_BEGIN && control->Owner != stack->FileObject)) {
        status = STATUS_ACCESS_DENIED;
        goto unlock_publication;
    }
    if (g_P04.Target == NULL) {
        status = g_P04.Binding == P04_RETIRED ? STATUS_DELETE_PENDING : STATUS_DEVICE_NOT_READY;
        goto unlock_publication;
    }
    extension = (P04FilterExtension *)g_P04.Target->DeviceExtension;
    status = P04Acquire(extension, irp);
    if (!NT_SUCCESS(status)) {
        extension = NULL;
        goto unlock_publication;
    }
    KeAcquireSpinLockAtDpcLevel(&extension->StateLock);
    win_cache_p04_stats(&extension->Recorder, &stats);
    if (request.command == WIN_CACHE_P04_BEGIN) {
        if (stats.observer_state != WIN_CACHE_P04_UNARMED) {
            status = STATUS_INVALID_DEVICE_STATE;
            goto unlock_recorder;
        }
        if (extension->PnpState != P04_STARTED) {
            status = STATUS_DEVICE_NOT_READY;
            goto unlock_recorder;
        }
        if (!win_cache_p04_begin(&extension->Recorder, request.session_id)) {
            status = STATUS_INVALID_DEVICE_STATE;
            goto unlock_recorder;
        }
        control->Owner = stack->FileObject;
    } else {
        if (request.session_id != stats.session_id || request.call_id <= extension->LastCallId) {
            status = STATUS_INVALID_PARAMETER;
            goto unlock_recorder;
        }
        if (request.command == WIN_CACHE_P04_STOP) {
            win_cache_p04_stop(&extension->Recorder);
            (void)win_cache_p04_seal(&extension->Recorder);
        } else if (request.command == WIN_CACHE_P04_DRAIN) {
            while (count < request.max_records && win_cache_p04_pop(&extension->Recorder, &record)) {
                win_cache_p04_encode_record(&record,
                    output + WIN_CACHE_P04_HEADER_BYTES + count * WIN_CACHE_P04_RECORD_BYTES);
                ++count;
            }
        }
    }
    extension->LastCallId = request.call_id;
    win_cache_p04_stats(&extension->Recorder, &stats);
    win_cache_p04_encode_header(&request, &stats, extension->PnpState, count, output);
    information = WIN_CACHE_P04_HEADER_BYTES + count * WIN_CACHE_P04_RECORD_BYTES;
    status = STATUS_SUCCESS;
unlock_recorder:
    KeReleaseSpinLockFromDpcLevel(&extension->StateLock);
unlock_publication:
    KeReleaseSpinLock(&g_P04.PublicationLock, previous);
    if (extension != NULL) IoReleaseRemoveLock(&extension->RemoveLock, irp);
    return P04CompleteControl(irp, status, information);
}

_Use_decl_annotations_
NTSTATUS P04DispatchControl(PDEVICE_OBJECT device, PIRP irp)
{
    P04ControlExtension *control = (P04ControlExtension *)device->DeviceExtension;
    PIO_STACK_LOCATION stack = IoGetCurrentIrpStackLocation(irp);
    switch (stack->MajorFunction) {
    case IRP_MJ_CREATE:
        return P04CreateControl(control, irp, stack->FileObject);
    case IRP_MJ_CLEANUP:
        return P04CleanupControl(control, irp, stack->FileObject);
    case IRP_MJ_CLOSE:
        /* CLEANUP owns identity retirement. An old CLOSE is not authority to
         * revoke a newer FileObject, and never follows a remembered target. */
        return P04CompleteControl(irp, STATUS_SUCCESS, 0);
    case IRP_MJ_DEVICE_CONTROL:
        return P04ControlCommand(control, irp, stack);
    default:
        return P04CompleteControl(irp, STATUS_INVALID_DEVICE_REQUEST, 0);
    }
}
