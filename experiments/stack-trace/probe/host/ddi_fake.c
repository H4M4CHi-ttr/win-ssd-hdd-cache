/* Host-only DDI scheduler. It drives the actual driver TU; it is not an
 * alternative lifecycle implementation and does not model Windows ABI. */
#include <ntddk.h>
#include <wdmsec.h>
#include <devpkey.h>
#include <stdlib.h>

#if defined(_WIN32)
#define H_TLS __declspec(thread)
#else
#define H_TLS _Thread_local
#endif
_Static_assert(sizeof(LONG) == 4 && sizeof(ULONG) == 4 && sizeof(WCHAR) == 2, "fixed fake widths");
_Static_assert(sizeof(L"x"[0]) == 2, "host compiler must use short wchar literals");

const DEVPROPKEY DEVPKEY_Device_InstanceId = {
    {0x78c34fc8u, 0x104au, 0x4acau, {0x9e,0xa4,0x52,0x4d,0x52,0x99,0x6e,0x57}}, 256u
};
typedef struct {
    PDEVICE_OBJECT value;
    PDRIVER_OBJECT driver;
    size_t object_region, extension_region;
    ULONG extension_bytes;
    unsigned references;
    int control, attached, pending, freed;
} HDevice;
typedef struct {
    PIRP value;
    size_t region;
    PDEVICE_OBJECT device;
    IO_STACK_LOCATION original;
    IO_STATUS_BLOCK original_status;
    PVOID original_system, original_user, original_mdl;
    HLowerPlan plan;
    HResult result;
    int dispatched, lower_done;
} HRequest;
typedef struct { HOperation operation; const void *object; unsigned thread; } HEvent;
typedef struct { PFILE_OBJECT file; PDEVICE_OBJECT device; int open; } HFile;

static P04HostMutex *h_meta;
static P04HostCond *h_changed;
static P04HostMutex *h_mutexes[4096];
static P04HostCond *h_conditions[4096];
static size_t h_mutex_count, h_condition_count;
static P04HostRegion h_regions[4096];
static size_t h_region_count;
static HDevice h_devices[32];
static size_t h_device_count;
static HRequest h_requests[1024];
static size_t h_request_count;
static HEvent h_events[32768];
static size_t h_event_count;
static HFile h_files[16];
static DRIVER_OBJECT h_driver_value;
static DRIVER_EXTENSION h_driver_extension;
static DEVICE_OBJECT h_pdo_value, h_lower_value;
static FILE_OBJECT h_early_file;
static UCHAR h_selector[1024], h_property[1024];
static ULONG h_selector_bytes, h_property_bytes, h_selector_type, h_property_type;
static NTSTATUS h_selector_status, h_property_status;
static int h_keys[3], h_link, h_attach_injection, h_create_injection;
static HOperation h_failure;
static void *h_guard;
static struct {
    HOperation operation; const void *object; unsigned thread;
    int armed, reached, released;
} h_gate;
static H_TLS KIRQL h_irql;
static H_TLS unsigned h_locks, h_label;

void h_expect(int value, const char *reason) { if (!value) p04_host_fail(reason); }
static void h_no_locks(void) { h_expect(h_locks == 0, "DDI_called_under_spinlock"); }
static P04HostMutex *h_new_mutex(void)
{
    P04HostMutex *value = p04_host_mutex_new();
    p04_host_mutex_lock(h_meta);
    h_expect(h_mutex_count < 4096, "mutex_capacity");
    h_mutexes[h_mutex_count++] = value;
    p04_host_mutex_unlock(h_meta);
    return value;
}
static P04HostCond *h_new_cond(void)
{
    P04HostCond *value = p04_host_cond_new();
    p04_host_mutex_lock(h_meta);
    h_expect(h_condition_count < 4096, "condition_capacity");
    h_conditions[h_condition_count++] = value;
    p04_host_mutex_unlock(h_meta);
    return value;
}
static size_t h_new_region(size_t bytes)
{
    P04HostRegion region = p04_host_region_new(bytes);
    size_t index;
    p04_host_mutex_lock(h_meta);
    h_expect(h_region_count < 4096, "region_capacity");
    index = h_region_count++;
    h_regions[index] = region;
    p04_host_mutex_unlock(h_meta);
    return index;
}
static void h_event(HOperation operation, const void *object)
{
    p04_host_mutex_lock(h_meta);
    h_expect(h_event_count < 32768, "event_capacity");
    h_events[h_event_count++] = (HEvent){operation, object, h_label};
    p04_host_cond_signal(h_changed);
    if (h_gate.armed && h_gate.operation == operation && h_gate.thread == h_label
            && (h_gate.object == NULL || h_gate.object == object)) {
        h_gate.reached = 1;
        p04_host_cond_signal(h_changed);
        while (!h_gate.released) p04_host_cond_wait(h_changed, h_meta);
        h_gate.armed = 0;
    }
    p04_host_mutex_unlock(h_meta);
}
static HDevice *h_device(PDEVICE_OBJECT value)
{
    size_t i;
    for (i = 0; i < h_device_count; ++i) if (h_devices[i].value == value) return &h_devices[i];
    p04_host_fail("unknown_device");
}
static HRequest *h_find_request(PIRP value)
{
    size_t i;
    for (i = 0; i < h_request_count; ++i) if (h_requests[i].value == value) return &h_requests[i];
    p04_host_fail("unknown_request");
}
static unsigned h_count_locked(HOperation operation, const void *object)
{
    unsigned result = 0;
    size_t i;
    for (i = 0; i < h_event_count; ++i)
        if (h_events[i].operation == operation && (object == NULL || h_events[i].object == object)) ++result;
    return result;
}
void h_gate_arm(HOperation operation, const void *object, unsigned thread)
{
    p04_host_mutex_lock(h_meta);
    h_expect(!h_gate.armed, "gate_already_armed");
    memset(&h_gate, 0, sizeof(h_gate));
    h_gate.operation = operation; h_gate.object = object; h_gate.thread = thread; h_gate.armed = 1;
    p04_host_mutex_unlock(h_meta);
}
void h_gate_wait(void)
{
    p04_host_mutex_lock(h_meta);
    while (!h_gate.reached) p04_host_cond_wait(h_changed, h_meta);
    p04_host_mutex_unlock(h_meta);
}
void h_gate_release(void)
{
    p04_host_mutex_lock(h_meta);
    h_expect(h_gate.reached, "gate_not_reached");
    h_gate.released = 1; p04_host_cond_signal(h_changed);
    p04_host_mutex_unlock(h_meta);
}
void h_wait_operation(HOperation operation, const void *object, unsigned count)
{
    p04_host_mutex_lock(h_meta);
    while (h_count_locked(operation, object) < count) p04_host_cond_wait(h_changed, h_meta);
    p04_host_mutex_unlock(h_meta);
}
unsigned h_count_operation(HOperation operation, const void *object)
{
    unsigned result;
    p04_host_mutex_lock(h_meta); result = h_count_locked(operation, object); p04_host_mutex_unlock(h_meta);
    return result;
}
void h_thread_label(unsigned value) { h_label = value; }
void h_set_irql(KIRQL value) { h_no_locks(); h_irql = value; }
KIRQL KeGetCurrentIrql(void) { return h_irql; }

void h_destroy(void)
{
    size_t i;
    h_expect(!h_gate.armed, "unconsumed_gate");
    for (i = 0; i < h_region_count; ++i) p04_host_region_free(&h_regions[i]);
    for (i = 0; i < h_condition_count; ++i) p04_host_cond_free(h_conditions[i]);
    for (i = 0; i < h_mutex_count; ++i) p04_host_mutex_free(h_mutexes[i]);
    h_region_count = h_condition_count = h_mutex_count = 0;
}
void h_reset(void)
{
    static const WCHAR instance[] = L"PCI\\P04\\FIXTURE";
    if (h_meta == NULL) { h_meta = p04_host_mutex_new(); h_changed = p04_host_cond_new(); }
    h_destroy();
    memset(h_devices, 0, sizeof(h_devices)); memset(h_requests, 0, sizeof(h_requests));
    memset(h_files, 0, sizeof(h_files)); memset(h_keys, 0, sizeof(h_keys));
    memset(&h_driver_value, 0, sizeof(h_driver_value));
    memset(&h_driver_extension, 0, sizeof(h_driver_extension));
    memset(&h_pdo_value, 0, sizeof(h_pdo_value)); memset(&h_lower_value, 0, sizeof(h_lower_value));
    memset(&h_early_file, 0, sizeof(h_early_file)); memset(&h_gate, 0, sizeof(h_gate));
    h_device_count = h_request_count = h_event_count = 0;
    h_driver_value.DriverExtension = &h_driver_extension;
    h_lower_value.Flags = DO_DIRECT_IO | DO_POWER_PAGABLE;
    h_lower_value.Characteristics = FILE_REMOVABLE_MEDIA | FILE_DEVICE_SECURE_OPEN;
    h_lower_value.DeviceType = FILE_DEVICE_DISK; h_lower_value.StackSize = 3;
    h_irql = PASSIVE_LEVEL; h_locks = h_label = 0;
    h_link = h_attach_injection = h_create_injection = 0; h_failure = 0;
    h_set_selector(instance, (ULONG)sizeof(instance), REG_SZ, STATUS_SUCCESS);
    h_set_property(instance, (ULONG)sizeof(instance), DEVPROP_TYPE_STRING, STATUS_SUCCESS);
    { size_t region = h_new_region(4096); h_guard = h_regions[region].bytes; p04_host_region_protect(&h_regions[region]); }
}
PDRIVER_OBJECT h_driver(void) { return &h_driver_value; }
PDEVICE_OBJECT h_pdo(void) { return &h_pdo_value; }
PDEVICE_OBJECT h_lower(void) { return &h_lower_value; }
PDEVICE_OBJECT h_fdo(void)
{
    size_t i;
    for (i = h_device_count; i > 0; --i) if (!h_devices[i - 1].control) return h_devices[i - 1].value;
    return NULL;
}
PDEVICE_OBJECT h_cdo(void)
{
    size_t i;
    for (i = h_device_count; i > 0; --i) if (h_devices[i - 1].control) return h_devices[i - 1].value;
    return NULL;
}
void *h_buffer(size_t bytes) { return h_regions[h_new_region(bytes)].bytes; }
void *h_payload_guard(void) { return h_guard; }
void h_set_selector(const WCHAR *value, ULONG bytes, ULONG type, NTSTATUS status)
{
    memset(h_selector, 0, sizeof(h_selector));
    if (value != NULL) memcpy(h_selector, value, bytes < sizeof(h_selector) ? bytes : sizeof(h_selector));
    h_selector_bytes = bytes; h_selector_type = type; h_selector_status = status;
}
void h_set_property(const WCHAR *value, ULONG bytes, DEVPROPTYPE type, NTSTATUS status)
{
    memset(h_property, 0, sizeof(h_property));
    if (value != NULL) memcpy(h_property, value, bytes < sizeof(h_property) ? bytes : sizeof(h_property));
    h_property_bytes = bytes; h_property_type = type; h_property_status = status;
}
void h_fail_next(HOperation value) { h_failure = value; }
static int h_should_fail(HOperation operation)
{
    if (h_failure != operation) return 0;
    h_failure = 0; return 1;
}
void h_inject_attach_dispatch(int value) { h_attach_injection = value; }
void h_inject_early_create(int value) { h_create_injection = value; }

void RtlInitUnicodeString(PUNICODE_STRING value, PCWSTR source)
{
    size_t length = 0;
    if (source != NULL) while (source[length] != 0) ++length;
    h_expect(length <= 32766, "unicode_literal_bounds");
    value->Buffer = (PWSTR)source; value->Length = (USHORT)(length * 2);
    value->MaximumLength = (USHORT)(source == NULL ? 0 : (length + 1) * 2);
}
BOOLEAN RtlEqualUnicodeString(const UNICODE_STRING *left, const UNICODE_STRING *right, BOOLEAN insensitive)
{
    size_t i;
    if (left->Length != right->Length || (left->Length & 1u) != 0) return FALSE;
    for (i = 0; i < left->Length / 2u; ++i) {
        WCHAR a = left->Buffer[i], b = right->Buffer[i];
        /* All fixed selector fixtures are ASCII. Real Unicode folding is a
         * property of the genuine Rtl routine, not claimed by this fixture. */
        if (insensitive) {
            if (a >= 'a' && a <= 'z') a = (WCHAR)(a - ('a' - 'A'));
            if (b >= 'a' && b <= 'z') b = (WCHAR)(b - ('a' - 'A'));
        }
        if (a != b) return FALSE;
    }
    return TRUE;
}
NTSTATUS ZwOpenKey(PHANDLE output, ACCESS_MASK access, POBJECT_ATTRIBUTES attributes)
{
    unsigned key = attributes->RootDirectory == NULL ? 1u : 2u;
    h_no_locks(); h_expect(h_irql == PASSIVE_LEVEL, "registry_irql");
    h_expect(access == KEY_READ || access == KEY_QUERY_VALUE, "registry_access");
    h_expect((attributes->Attributes & OBJ_KERNEL_HANDLE) != 0, "registry_kernel_handle");
    h_expect(!h_keys[key], "key_open_twice"); h_keys[key] = 1;
    *output = (HANDLE)(uintptr_t)key;
    return STATUS_SUCCESS;
}
NTSTATUS ZwQueryValueKey(HANDLE key, PUNICODE_STRING name, KEY_VALUE_INFORMATION_CLASS information_class,
                        PVOID buffer, ULONG capacity, PULONG returned)
{
    const ULONG header = (ULONG)offsetof(KEY_VALUE_PARTIAL_INFORMATION, Data);
    PKEY_VALUE_PARTIAL_INFORMATION result = buffer;
    UNICODE_STRING expected;
    h_no_locks(); h_expect(h_irql == PASSIVE_LEVEL, "registry_query_irql");
    h_expect((uintptr_t)key == 2u && h_keys[2], "parameters_key_required");
    RtlInitUnicodeString(&expected, L"TargetInstanceId");
    h_expect(RtlEqualUnicodeString(name, &expected, FALSE), "selector_name");
    h_expect(information_class == KeyValuePartialInformation, "key_partial_information");
    *returned = header + h_selector_bytes;
    if (!NT_SUCCESS(h_selector_status)) return h_selector_status;
    if (capacity < *returned) return STATUS_BUFFER_TOO_SMALL;
    result->TitleIndex = 0; result->Type = h_selector_type; result->DataLength = h_selector_bytes;
    memcpy(result->Data, h_selector, h_selector_bytes);
    return STATUS_SUCCESS;
}
NTSTATUS ZwClose(HANDLE key)
{
    unsigned index = (unsigned)(uintptr_t)key;
    h_no_locks(); h_expect(index == 1 || index == 2, "key_close_type");
    h_expect(h_keys[index], "key_double_close"); h_keys[index] = 0; return STATUS_SUCCESS;
}
NTSTATUS IoGetDevicePropertyData(PDEVICE_OBJECT pdo, const DEVPROPKEY *property, LCID locale,
                                ULONG flags, ULONG capacity, PVOID buffer, PULONG returned,
                                PDEVPROPTYPE type)
{
    h_no_locks(); h_expect(h_irql == PASSIVE_LEVEL, "property_irql");
    h_expect(pdo == h_pdo(), "property_on_supplied_pdo");
    h_expect(property->pid == 256u && locale == 0 && flags == 0, "property_key");
    *returned = h_property_bytes; *type = h_property_type;
    if (!NT_SUCCESS(h_property_status)) return h_property_status;
    if (capacity < h_property_bytes) return STATUS_BUFFER_TOO_SMALL;
    memcpy(buffer, h_property, h_property_bytes); return STATUS_SUCCESS;
}

static NTSTATUS h_create_device(PDRIVER_OBJECT driver, ULONG extension_bytes, DEVICE_TYPE type,
                               ULONG characteristics, BOOLEAN exclusive, int control, PDEVICE_OBJECT *out)
{
    HOperation operation = control ? H_CREATE_CDO : H_CREATE_FDO;
    HDevice *metadata;
    size_t object_region, extension_region;
    h_no_locks(); h_expect(h_irql == PASSIVE_LEVEL, "create_device_irql");
    h_expect(exclusive == FALSE, "driver_fileobject_sharing_not_exclusive_do");
    h_event(operation, NULL);
    if (h_should_fail(operation)) { *out = NULL; return STATUS_INSUFFICIENT_RESOURCES; }
    object_region = h_new_region(sizeof(DEVICE_OBJECT)); extension_region = h_new_region(extension_bytes);
    p04_host_mutex_lock(h_meta);
    h_expect(h_device_count < 32, "device_capacity");
    metadata = &h_devices[h_device_count++];
    metadata->value = h_regions[object_region].bytes; metadata->driver = driver;
    metadata->object_region = object_region; metadata->extension_region = extension_region;
    metadata->extension_bytes = extension_bytes; metadata->control = control;
    metadata->value->DriverObject = driver; metadata->value->DeviceExtension = h_regions[extension_region].bytes;
    metadata->value->Flags = DO_DEVICE_INITIALIZING; metadata->value->DeviceType = type;
    metadata->value->Characteristics = characteristics;
    metadata->value->NextDevice = driver->DeviceObject; driver->DeviceObject = metadata->value;
    *out = metadata->value;
    p04_host_mutex_unlock(h_meta);
    return STATUS_SUCCESS;
}
NTSTATUS IoCreateDevice(PDRIVER_OBJECT driver, ULONG extension_bytes, PUNICODE_STRING name,
                        DEVICE_TYPE type, ULONG characteristics, BOOLEAN exclusive, PDEVICE_OBJECT *out)
{
    h_expect(name == NULL, "filter_do_must_be_unnamed");
    return h_create_device(driver, extension_bytes, type, characteristics, exclusive, 0, out);
}
NTSTATUS WdmlibIoCreateDeviceSecure(PDRIVER_OBJECT driver, ULONG extension_bytes, PUNICODE_STRING name,
    DEVICE_TYPE type, ULONG characteristics, BOOLEAN exclusive, const UNICODE_STRING *sddl,
    const GUID *class_guid, PDEVICE_OBJECT *out)
{
    UNICODE_STRING expected;
    RtlInitUnicodeString(&expected, L"D:P(A;;GA;;;SY)(A;;GA;;;BA)");
    h_expect(name != NULL && name->Length != 0, "cdo_name_required");
    h_expect(RtlEqualUnicodeString(sddl, &expected, FALSE), "cdo_sddl_literal");
    h_expect(class_guid != NULL && class_guid->Data1 == 0xd475d19au, "cdo_class_guid");
    h_expect((characteristics & FILE_DEVICE_SECURE_OPEN) != 0, "cdo_secure_open");
    return h_create_device(driver, extension_bytes, type, characteristics, exclusive, 1, out);
}
NTSTATUS IoCreateSymbolicLink(PUNICODE_STRING link, PUNICODE_STRING target)
{
    h_no_locks(); h_event(H_CREATE_LINK, NULL);
    h_expect(link != NULL && target != NULL && !h_link, "link_create");
    if (h_should_fail(H_CREATE_LINK)) return STATUS_INSUFFICIENT_RESOURCES;
    h_link = 1;
    if (h_create_injection) {
        PIRP request = h_request(IRP_MJ_CREATE, 0);
        request->Stack[0].FileObject = &h_early_file;
        h_expect(h_dispatch(h_cdo(), request) == STATUS_DEVICE_NOT_READY, "early_create_not_ready");
        h_expect(h_result(request).information == 0, "early_create_no_information");
    }
    return STATUS_SUCCESS;
}
NTSTATUS IoDeleteSymbolicLink(PUNICODE_STRING name)
{
    h_no_locks(); h_expect(name != NULL && h_link, "link_delete_once");
    h_event(H_DELETE_LINK, NULL); h_link = 0; return STATUS_SUCCESS;
}
NTSTATUS IoAttachDeviceToDeviceStackSafe(PDEVICE_OBJECT source, PDEVICE_OBJECT target, PDEVICE_OBJECT *lower)
{
    HDevice *device = h_device(source);
    uintptr_t output = (uintptr_t)lower, base = (uintptr_t)source->DeviceExtension;
    h_no_locks(); h_expect(h_irql == PASSIVE_LEVEL, "attach_irql");
    h_expect(target == h_pdo() && !device->control && !device->attached, "attach_exact_supplied_pdo");
    h_expect(output >= base && output + sizeof(*lower) <= base + device->extension_bytes,
             "attach_must_assign_extension_field");
    if (h_should_fail(H_ATTACH_FAIL)) { h_event(H_ATTACH_FAIL, source); return STATUS_INSUFFICIENT_RESOURCES; }
    *lower = h_lower(); source->StackSize = (UCHAR)(h_lower()->StackSize + 1u);
    device->attached = 1; h_event(H_ATTACH_OK, source);
    if (h_attach_injection) {
        PIRP request = h_request(IRP_MJ_READ, 0);
        request->Stack[0].Parameters.Read.Length = 512;
        h_expect(h_dispatch(source, request) == STATUS_SUCCESS, "early_dispatch_status");
        h_expect(h_result(request).completed == 1 && h_result(request).forwarded == 1,
                 "early_dispatch_valid_lifetime");
    }
    return STATUS_SUCCESS;
}
void IoDetachDevice(PDEVICE_OBJECT lower)
{
    HDevice *device = h_device(h_fdo());
    h_no_locks(); h_expect(lower == h_lower() && device->attached, "detach_once");
    device->attached = 0; h_event(H_DETACH, device->value);
}
static void h_maybe_free_device(HDevice *device)
{
    PDEVICE_OBJECT *link;
    if (!device->pending || device->references != 0 || device->freed) return;
    h_expect(!device->attached, "delete_attached_device");
    link = &device->driver->DeviceObject;
    while (*link != NULL && *link != device->value) link = &(*link)->NextDevice;
    h_expect(*link == device->value, "device_list_membership");
    *link = device->value->NextDevice;
    device->freed = 1;
    p04_host_region_protect(&h_regions[device->extension_region]);
    p04_host_region_protect(&h_regions[device->object_region]);
}
void IoDeleteDevice(PDEVICE_OBJECT value)
{
    HDevice *device;
    h_no_locks(); h_expect(h_irql <= APC_LEVEL, "delete_device_irql");
    p04_host_mutex_lock(h_meta); device = h_device(value);
    h_expect(!device->pending && !device->freed, "delete_device_once");
    device->pending = 1; h_maybe_free_device(device);
    p04_host_mutex_unlock(h_meta);
    h_event(device->control ? H_DELETE_CDO : H_DELETE_FDO, value);
}

void KeInitializeSpinLock(PKSPIN_LOCK value) { value->mutex = h_new_mutex(); value->owner = 0; }
void KeAcquireSpinLock(PKSPIN_LOCK value, PKIRQL previous)
{
    h_expect(h_irql <= DISPATCH_LEVEL, "spin_acquire_irql");
    h_event(H_SPIN_ACQUIRE_BEFORE, value);
    *previous = h_irql; h_irql = DISPATCH_LEVEL;
    p04_host_mutex_lock(value->mutex);
    h_expect(value->owner == 0, "spin_owner"); value->owner = h_label + 1; ++h_locks;
}
void KeReleaseSpinLock(PKSPIN_LOCK value, KIRQL previous)
{
    h_expect(h_irql == DISPATCH_LEVEL && value->owner == h_label + 1 && h_locks > 0, "spin_release");
    value->owner = 0; --h_locks; p04_host_mutex_unlock(value->mutex); h_irql = previous;
    h_event(H_SPIN_RELEASE, value);
}
void KeAcquireSpinLockAtDpcLevel(PKSPIN_LOCK value)
{
    h_expect(h_irql == DISPATCH_LEVEL, "nested_spin_irql");
    p04_host_mutex_lock(value->mutex);
    h_expect(value->owner == 0, "nested_spin_owner"); value->owner = h_label + 1; ++h_locks;
}
void KeReleaseSpinLockFromDpcLevel(PKSPIN_LOCK value)
{
    h_expect(h_irql == DISPATCH_LEVEL && value->owner == h_label + 1 && h_locks > 0, "nested_spin_release");
    value->owner = 0; --h_locks; p04_host_mutex_unlock(value->mutex);
}
void IoInitializeRemoveLock(PIO_REMOVE_LOCK value, ULONG tag, ULONG minutes, ULONG high_water)
{
    UNREFERENCED_PARAMETER(tag); UNREFERENCED_PARAMETER(minutes); UNREFERENCED_PARAMETER(high_water);
    memset(value, 0, sizeof(*value)); value->mutex = h_new_mutex(); value->condition = h_new_cond();
    h_event(H_INITIALIZE_FDO, value);
}
NTSTATUS IoAcquireRemoveLock(PIO_REMOVE_LOCK value, const void *tag)
{
    unsigned i;
    h_expect(h_irql <= DISPATCH_LEVEL, "remove_acquire_irql");
    p04_host_mutex_lock(value->mutex);
    if (value->removed) { p04_host_mutex_unlock(value->mutex); return STATUS_DELETE_PENDING; }
    h_expect(value->count < 512, "remove_lock_capacity");
    for (i = 0; i < value->count; ++i) h_expect(value->tags[i] != tag, "duplicate_remove_tag");
    value->tags[value->count++] = tag;
    p04_host_mutex_unlock(value->mutex); return STATUS_SUCCESS;
}
static void h_release_tag(PIO_REMOVE_LOCK value, const void *tag)
{
    unsigned i;
    for (i = 0; i < value->count; ++i) if (value->tags[i] == tag) break;
    h_expect(i < value->count, "unowned_remove_tag");
    value->tags[i] = value->tags[--value->count]; value->tags[value->count] = NULL;
    p04_host_cond_signal(value->condition);
}
void IoReleaseRemoveLock(PIO_REMOVE_LOCK value, const void *tag)
{
    h_no_locks(); h_event(H_REMOVE_RELEASE_BEFORE, value);
    p04_host_mutex_lock(value->mutex); h_release_tag(value, tag); p04_host_mutex_unlock(value->mutex);
    h_event(H_REMOVE_RELEASE_AFTER, value);
}
void IoReleaseRemoveLockAndWait(PIO_REMOVE_LOCK value, const void *tag)
{
    h_no_locks(); h_expect(h_irql == PASSIVE_LEVEL, "remove_wait_irql");
    p04_host_mutex_lock(value->mutex);
    h_expect(!value->removed, "remove_wait_once"); value->removed = 1; h_release_tag(value, tag);
    h_event(H_ANDWAIT_ENTER, value);
    while (value->count != 0) p04_host_cond_wait(value->condition, value->mutex);
    p04_host_mutex_unlock(value->mutex); h_event(H_ANDWAIT_END, value);
}
unsigned h_remove_holds(PIO_REMOVE_LOCK value)
{
    unsigned count;
    p04_host_mutex_lock(value->mutex); count = value->count; p04_host_mutex_unlock(value->mutex); return count;
}
void KeInitializeEvent(PKEVENT value, int type, BOOLEAN signaled)
{
    h_no_locks(); h_expect(type == NotificationEvent, "notification_event");
    value->mutex = h_new_mutex(); value->condition = h_new_cond(); value->signaled = signaled != FALSE;
}
LONG KeSetEvent(PKEVENT value, LONG increment, BOOLEAN wait)
{
    int old;
    h_expect(h_irql <= DISPATCH_LEVEL && increment == IO_NO_INCREMENT && !wait, "event_set_args");
    p04_host_mutex_lock(value->mutex); old = value->signaled; value->signaled = 1;
    p04_host_cond_signal(value->condition); p04_host_mutex_unlock(value->mutex); return old;
}
NTSTATUS KeWaitForSingleObject(PVOID object, int reason, int mode, BOOLEAN alertable, PLARGE_INTEGER timeout)
{
    PKEVENT value = object;
    h_no_locks(); h_expect(h_irql == PASSIVE_LEVEL, "event_wait_passive");
    h_expect(reason == Executive && mode == KernelMode && !alertable && timeout == NULL, "event_wait_args");
    h_event(H_EVENT_WAIT, object); p04_host_mutex_lock(value->mutex);
    while (!value->signaled) p04_host_cond_wait(value->condition, value->mutex);
    p04_host_mutex_unlock(value->mutex); return STATUS_SUCCESS;
}

PIRP h_request(UCHAR major, UCHAR minor)
{
    HRequest *request;
    size_t region = h_new_region(sizeof(IRP));
    p04_host_mutex_lock(h_meta);
    h_expect(h_request_count < 1024, "request_capacity");
    request = &h_requests[h_request_count++]; request->value = h_regions[region].bytes; request->region = region;
    request->value->Stack[0].MajorFunction = major; request->value->Stack[0].MinorFunction = minor;
    request->value->IoStatus.Status = STATUS_NOT_SUPPORTED;
    request->value->IoStatus.Information = (ULONG_PTR)0xfedcba9876543210ULL;
    request->value->AssociatedIrp.SystemBuffer = h_guard;
    request->value->UserBuffer = h_guard; request->value->MdlAddress = h_guard;
    request->plan = (HLowerPlan){0, STATUS_SUCCESS, STATUS_SUCCESS, 512, FALSE, FALSE, DISPATCH_LEVEL};
    p04_host_mutex_unlock(h_meta); return request->value;
}
void h_plan(PIRP value, HLowerPlan plan) { h_find_request(value)->plan = plan; }
PIO_STACK_LOCATION IoGetCurrentIrpStackLocation(PIRP value) { return &value->Stack[0]; }
void IoCopyCurrentIrpStackLocationToNext(PIRP value)
{
    HRequest *request = h_find_request(value);
    h_expect(!value->Copied && !value->Skipped, "copy_stack_once");
    value->Stack[1] = value->Stack[0]; value->Copied = TRUE; ++request->result.copied;
}
void IoSkipCurrentIrpStackLocation(PIRP value)
{
    h_expect(!value->Copied && !value->Skipped && value->Completion == NULL, "skip_stack_once");
    value->Skipped = TRUE;
}
void IoSetCompletionRoutine(PIRP value, PIO_COMPLETION_ROUTINE routine, PVOID context,
                            BOOLEAN success, BOOLEAN error, BOOLEAN cancel)
{
    h_expect(value->Copied && !value->Skipped && value->Completion == NULL, "copy_before_completion");
    h_expect(success && error && cancel, "completion_all_outcomes");
    value->Completion = routine; value->CompletionContext = context;
    value->OnSuccess = success; value->OnError = error; value->OnCancel = cancel;
    ++h_find_request(value)->result.installed;
}
static void h_finish_irp(PIRP value)
{
    HRequest *request = h_find_request(value);
    HDevice *device = h_device(request->device);
    unsigned i;
    p04_host_mutex_lock(h_meta);
    h_expect(request->result.completed == 0, "irp_completed_twice");
    request->result.status = value->IoStatus.Status; request->result.information = value->IoStatus.Information;
    request->result.completed = 1;
    if (device->control && request->original.MajorFunction == IRP_MJ_CREATE && NT_SUCCESS(value->IoStatus.Status)) {
        for (i = 0; i < 16; ++i) if (h_files[i].open && h_files[i].file == request->original.FileObject) break;
        if (i == 16) {
            for (i = 0; i < 16; ++i) if (!h_files[i].open) break;
            h_expect(i < 16, "file_capacity");
            h_files[i] = (HFile){request->original.FileObject, request->device, 1}; ++device->references;
        }
    }
    if (device->control && request->original.MajorFunction == IRP_MJ_CLOSE) {
        for (i = 0; i < 16; ++i) if (h_files[i].open && h_files[i].file == request->original.FileObject) break;
        h_expect(i < 16, "close_open_file"); h_files[i].open = 0;
        h_expect(device->references > 0, "close_device_ref"); --device->references;
    }
    h_expect(device->references > 0, "irp_device_ref"); --device->references;
    h_maybe_free_device(device);
    p04_host_region_protect(&h_regions[request->region]);
    p04_host_mutex_unlock(h_meta); h_event(H_COMPLETE, value);
}
void IoCompleteRequest(PIRP value, UCHAR increment)
{
    h_no_locks(); h_expect(increment == IO_NO_INCREMENT, "completion_boost"); h_finish_irp(value);
}
void IoMarkIrpPending(PIRP value)
{
    HRequest *request = h_find_request(value);
    h_expect(value->PendingReturned && value->Copied && !value->Skipped, "pending_propagation");
    ++request->result.marked; h_expect(request->result.marked == 1, "pending_once");
    h_event(H_MARK_PENDING, value);
}
void h_complete_lower(PIRP value)
{
    HRequest *request = h_find_request(value);
    KIRQL previous = h_irql;
    PIO_COMPLETION_ROUTINE completion;
    PVOID context;
    NTSTATUS outcome = STATUS_CONTINUE_COMPLETION;
    h_expect(request->dispatched && request->result.forwarded == 1 && !request->lower_done,
             "lower_completion_once");
    request->lower_done = 1;
    value->IoStatus.Status = request->plan.completed; value->IoStatus.Information = request->plan.information;
    value->PendingReturned = request->plan.pending; value->Cancel = request->plan.cancel;
    completion = value->Completion; context = value->CompletionContext;
    h_irql = request->plan.completion_irql;
    if (completion != NULL) outcome = completion(request->device, value, context);
    h_expect(outcome == STATUS_CONTINUE_COMPLETION || outcome == STATUS_MORE_PROCESSING_REQUIRED,
             "completion_return_not_pending");
    if (outcome != STATUS_MORE_PROCESSING_REQUIRED) h_finish_irp(value);
    h_irql = previous;
}
NTSTATUS IoCallDriver(PDEVICE_OBJECT lower, PIRP value)
{
    HRequest *request = h_find_request(value);
    IO_STACK_LOCATION *stack = value->Copied ? &value->Stack[1] : &value->Stack[0];
    const NTSTATUS result = request->plan.returned;
    const int delayed = request->plan.delayed;
    h_no_locks(); h_expect(h_irql <= DISPATCH_LEVEL && lower == h_lower(), "forward_target_irql");
    h_expect(value->Copied || value->Skipped, "forward_stack_prepared");
    h_expect(memcmp(stack, &request->original, sizeof(*stack)) == 0, "original_parameters_unchanged");
    h_expect(value->AssociatedIrp.SystemBuffer == request->original_system
        && value->UserBuffer == request->original_user && value->MdlAddress == request->original_mdl,
        "original_buffers_unchanged");
    if (stack->MajorFunction != IRP_MJ_PNP) {
        h_expect(value->IoStatus.Status == request->original_status.Status
            && value->IoStatus.Information == request->original_status.Information,
            "ordinary_status_unchanged_before_lower");
    } else h_expect(value->IoStatus.Information == request->original_status.Information,
                    "pnp_information_unchanged_before_lower");
    ++request->result.forwarded; h_expect(request->result.forwarded == 1, "forward_once");
    h_event(H_FORWARD, value);
    if (!delayed) h_complete_lower(value);
    h_event(H_CALL_RETURN_BEFORE, value);
    return result;
}
NTSTATUS h_dispatch(PDEVICE_OBJECT device, PIRP value)
{
    HRequest *request = h_find_request(value);
    HDevice *metadata = h_device(device);
    PDRIVER_DISPATCH dispatch;
    p04_host_mutex_lock(h_meta);
    h_expect(!metadata->freed && !request->dispatched, "dispatch_live_objects");
    ++metadata->references; request->device = device; request->dispatched = 1;
    request->original = value->Stack[0]; request->original_status = value->IoStatus;
    request->original_system = value->AssociatedIrp.SystemBuffer;
    request->original_user = value->UserBuffer; request->original_mdl = value->MdlAddress;
    dispatch = device->DriverObject->MajorFunction[value->Stack[0].MajorFunction];
    p04_host_mutex_unlock(h_meta); h_expect(dispatch != NULL, "registered_dispatch");
    return dispatch(device, value);
}
HResult h_result(PIRP value)
{
    HResult result;
    p04_host_mutex_lock(h_meta); result = h_find_request(value)->result; p04_host_mutex_unlock(h_meta); return result;
}
unsigned h_live_devices(void)
{
    unsigned result = 0; size_t i;
    p04_host_mutex_lock(h_meta);
    for (i = 0; i < h_device_count; ++i) if (!h_devices[i].freed) ++result;
    p04_host_mutex_unlock(h_meta); return result;
}
unsigned h_open_handles(void)
{
    unsigned result = 0; unsigned i;
    p04_host_mutex_lock(h_meta);
    for (i = 0; i < 3; ++i) result += h_keys[i] != 0;
    for (i = 0; i < 16; ++i) result += h_files[i].open != 0;
    p04_host_mutex_unlock(h_meta); return result;
}
unsigned h_pending_requests(void)
{
    unsigned result = 0; size_t i;
    p04_host_mutex_lock(h_meta);
    for (i = 0; i < h_request_count; ++i)
        if (h_requests[i].dispatched && !h_requests[i].result.completed) ++result;
    p04_host_mutex_unlock(h_meta); return result;
}
int h_device_freed(PDEVICE_OBJECT value)
{
    int result; p04_host_mutex_lock(h_meta); result = h_device(value)->freed; p04_host_mutex_unlock(h_meta); return result;
}
int h_device_delete_pending(PDEVICE_OBJECT value)
{
    int result; p04_host_mutex_lock(h_meta); result = h_device(value)->pending; p04_host_mutex_unlock(h_meta); return result;
}
void h_expect_setup(const HOperation *expected, size_t count)
{
    size_t i, actual = 0;
    p04_host_mutex_lock(h_meta);
    for (i = 0; i < h_event_count; ++i) {
        HOperation operation = h_events[i].operation;
        if (operation >= H_CREATE_FDO && operation <= H_DELETE_FDO) {
            h_expect(actual < count && expected[actual] == operation, "literal_setup_order"); ++actual;
        }
    }
    h_expect(actual == count, "literal_setup_count");
    p04_host_mutex_unlock(h_meta);
}
