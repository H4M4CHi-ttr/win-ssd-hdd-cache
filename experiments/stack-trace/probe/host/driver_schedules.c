/* Literal schedules exercise the ACTUAL driver TU. The only substitutions are
 * the isolated host DDI provider and host threading/guard-page platform. */
#include <ntddk.h>
#include <stdio.h>
#include <string.h>
#include "../wdk/driver.c"

static FILE_OBJECT file_a, file_b;
static int open_a, open_b;
static unsigned passed;
typedef struct { PIRP irp; UCHAR *bytes; NTSTATUS returned; HResult result; } CommandResult;
typedef struct { PDEVICE_OBJECT device; PIRP irp; unsigned label; NTSTATUS result; int add; } Job;

static void expect(int value) { h_expect(value, "literal_schedule_expectation"); }
static void put32(UCHAR *bytes, ULONG value)
{ unsigned i; for (i = 0; i < 4; ++i) bytes[i] = (UCHAR)(value >> (8u * i)); }
static void put64(UCHAR *bytes, ULONGLONG value)
{ unsigned i; for (i = 0; i < 8; ++i) bytes[i] = (UCHAR)(value >> (8u * i)); }
static ULONG get32(const UCHAR *bytes)
{ ULONG result = 0; unsigned i; for (i = 0; i < 4; ++i) result |= (ULONG)bytes[i] << (8u * i); return result; }
static ULONGLONG get64(const UCHAR *bytes)
{ ULONGLONG result = 0; unsigned i; for (i = 0; i < 8; ++i) result |= (ULONGLONG)bytes[i] << (8u * i); return result; }
static P04FilterExtension *extension(void) { return h_fdo()->DeviceExtension; }
static WinCacheP04Stats stats(void)
{
    WinCacheP04Stats result; KIRQL old; P04FilterExtension *value = extension();
    KeAcquireSpinLock(&value->StateLock, &old);
    win_cache_p04_stats(&value->Recorder, &result);
    KeReleaseSpinLock(&value->StateLock, old); return result;
}
static PIRP request(UCHAR major, UCHAR minor)
{
    PIRP result = h_request(major, minor);
    if (major == IRP_MJ_READ || major == IRP_MJ_WRITE) {
        result->Stack[0].Parameters.Read.ByteOffset.QuadPart = 4096;
        result->Stack[0].Parameters.Read.Length = 512;
    }
    return result;
}
static HLowerPlan delayed(NTSTATUS result)
{ HLowerPlan plan = {1, STATUS_PENDING, result, 512, TRUE, FALSE, DISPATCH_LEVEL}; return plan; }
static void job_dispatch(void *context)
{
    Job *job = context; h_thread_label(job->label);
    if (job->add) job->result = h_driver()->DriverExtension->AddDevice(h_driver(), h_pdo());
    else job->result = h_dispatch(job->device, job->irp);
}
static void job_complete(void *context)
{ Job *job = context; h_thread_label(job->label); h_complete_lower(job->irp); }
static void start_fixture(void)
{
    UNICODE_STRING registry;
    h_reset(); memset(&file_a, 0, sizeof(file_a)); memset(&file_b, 0, sizeof(file_b)); open_a = open_b = 0;
    RtlInitUnicodeString(&registry, L"\\Registry\\Machine\\System\\CurrentControlSet\\Services\\P04Fixture");
    expect(DriverEntry(h_driver(), &registry) == STATUS_SUCCESS);
    expect(h_driver()->DriverExtension->AddDevice != NULL && h_driver()->DriverUnload != NULL);
}
static NTSTATUS file_irp(PFILE_OBJECT file, UCHAR major)
{
    PIRP value = request(major, 0); value->Stack[0].FileObject = file;
    return h_dispatch(h_cdo(), value);
}
static void create_a(void) { expect(file_irp(&file_a, IRP_MJ_CREATE) == STATUS_SUCCESS); open_a = 1; }
static void create_b(void) { expect(file_irp(&file_b, IRP_MJ_CREATE) == STATUS_SUCCESS); open_b = 1; }
static void close_a(void)
{
    expect(file_irp(&file_a, IRP_MJ_CLEANUP) == STATUS_SUCCESS);
    expect(file_irp(&file_a, IRP_MJ_CLOSE) == STATUS_SUCCESS); open_a = 0;
}
static void close_b(void)
{
    expect(file_irp(&file_b, IRP_MJ_CLEANUP) == STATUS_SUCCESS);
    expect(file_irp(&file_b, IRP_MJ_CLOSE) == STATUS_SUCCESS); open_b = 0;
}
static PIRP prepare_command(PFILE_OBJECT file, ULONG code, ULONGLONG session, ULONGLONG call,
                             ULONG maximum, ULONG input_length, ULONG output_length, UCHAR **buffer)
{
    PIRP value = request(IRP_MJ_DEVICE_CONTROL, 0);
    UCHAR *bytes = h_buffer(output_length > 48 ? output_length : 48);
    memset(bytes, 0xa5, output_length > 48 ? output_length : 48);
    memcpy(bytes, "WCHP04RQ", 8); put32(bytes + 8, 1); put32(bytes + 12, 48);
    put64(bytes + 16, session); put64(bytes + 24, 1); put64(bytes + 32, call);
    put32(bytes + 40, maximum); put32(bytes + 44, 0);
    value->AssociatedIrp.SystemBuffer = bytes; value->Stack[0].FileObject = file;
    value->Stack[0].Parameters.DeviceIoControl.IoControlCode = code;
    value->Stack[0].Parameters.DeviceIoControl.InputBufferLength = input_length;
    value->Stack[0].Parameters.DeviceIoControl.OutputBufferLength = output_length;
    *buffer = bytes; return value;
}
static CommandResult command_for(PFILE_OBJECT file, ULONG code, ULONGLONG call)
{
    CommandResult result; const ULONG capacity = code == 0x8337e008u ? 1680u : 144u;
    result.irp = prepare_command(file, code, 7, call, code == 0x8337e008u ? 16u : 0u, 48, capacity, &result.bytes);
    result.returned = h_dispatch(h_cdo(), result.irp); result.result = h_result(result.irp); return result;
}
static CommandResult command(ULONG code, ULONGLONG call) { return command_for(&file_a, code, call); }
static void attach(void)
{
    expect(h_driver()->DriverExtension->AddDevice(h_driver(), h_pdo()) == STATUS_SUCCESS);
    expect(h_fdo() != NULL && h_cdo() != NULL && g_P04.Target == h_fdo());
}
static void start(void)
{
    PIRP value = request(IRP_MJ_PNP, IRP_MN_START_DEVICE);
    expect(h_dispatch(h_fdo(), value) == STATUS_SUCCESS);
    expect(extension()->PnpState == 1 && h_result(value).completed == 1 && h_result(value).marked == 0);
}
static void running_fixture(void)
{
    CommandResult result; start_fixture(); attach(); start(); create_a(); result = command(0x8337e000u, 1);
    expect(result.returned == STATUS_SUCCESS && result.result.information == 144);
    expect(get32(result.bytes + 56) == 1 && get32(result.bytes + 60) == 1);
    expect(get32(result.bytes + 72) == 7 && get32(result.bytes + 76) == 0 && get64(result.bytes + 80) == 0);
}
static void finish_fixture(void)
{
    expect(h_pending_requests() == 0);
    if (h_fdo() != NULL && !h_device_freed(h_fdo())) {
        PIRP value = request(IRP_MJ_PNP, IRP_MN_REMOVE_DEVICE);
        expect(h_dispatch(h_fdo(), value) == STATUS_SUCCESS);
    }
    if (open_a) close_a();
    if (open_b) close_b();
    expect(h_live_devices() == 0 && h_open_handles() == 0 && h_pending_requests() == 0);
    expect(h_driver()->DeviceObject == NULL);
    h_driver()->DriverUnload(h_driver());
}
static void pass(const char *name) { ++passed; printf("P04_CASE_PASS %s\n", name); fflush(stdout); }

static void add_absent_no_attach(void)
{
    UNICODE_STRING registry;
    start_fixture(); h_set_selector(NULL, 0, REG_SZ, STATUS_OBJECT_NAME_NOT_FOUND);
    RtlInitUnicodeString(&registry, L"\\Registry\\Machine\\P04");
    expect(DriverEntry(h_driver(), &registry) == STATUS_SUCCESS);
    expect(P04AddDevice(h_driver(), h_pdo()) == STATUS_SUCCESS && h_live_devices() == 0);
    expect(h_count_operation(H_CREATE_FDO, NULL) == 0); finish_fixture();
}
static void add_malformed_no_attach(void)
{
    static const WCHAR values[][4] = {{'A',0,'B',0},{'A','B','C','D'},{0,0,0,0}};
    unsigned i;
    for (i = 0; i < 3; ++i) {
        UNICODE_STRING registry; start_fixture(); h_set_selector(values[i], 8, REG_SZ, STATUS_SUCCESS);
        RtlInitUnicodeString(&registry, L"\\Registry\\Machine\\P04");
        expect(DriverEntry(h_driver(), &registry) == STATUS_SUCCESS);
        expect(P04AddDevice(h_driver(), h_pdo()) == STATUS_SUCCESS && h_live_devices() == 0); finish_fixture();
    }
}
static void add_mismatch_no_attach(void)
{
    static const WCHAR wrong[] = L"PCI\\P04\\OTHER";
    start_fixture(); h_set_property(wrong, (ULONG)sizeof(wrong), DEVPROP_TYPE_STRING, STATUS_SUCCESS);
    expect(P04AddDevice(h_driver(), h_pdo()) == STATUS_SUCCESS && h_live_devices() == 0); finish_fixture();
}
static void add_exact_once(void)
{
    static const HOperation expected[] = {H_CREATE_FDO,H_INITIALIZE_FDO,H_CREATE_CDO,H_CREATE_LINK,H_ATTACH_OK};
    start_fixture(); attach(); expect(P04AddDevice(h_driver(), h_pdo()) == STATUS_SUCCESS);
    h_expect_setup(expected, sizeof(expected)/sizeof(expected[0]));
    expect(h_count_operation(H_ATTACH_OK, NULL) == 1 && g_P04.Binding == P04_ATTACHED);
    expect((h_fdo()->Flags & (DO_POWER_PAGABLE | DO_POWER_INRUSH | DO_DEVICE_INITIALIZING)) == 0);
    expect((h_fdo()->Flags & DO_DIRECT_IO) != 0 && stats().observer_state == 0); finish_fixture();
}
static void add_concurrent_exact_single_claim(void)
{
    Job first = {0}; P04HostThread *thread;
    start_fixture(); first.label = 1; first.add = 1;
    h_gate_arm(H_SPIN_RELEASE, &g_P04.PublicationLock, 1);
    thread = p04_host_thread_start(job_dispatch, &first); h_gate_wait();
    expect(g_P04.Binding == P04_PREPARING);
    expect(P04AddDevice(h_driver(), h_pdo()) == STATUS_SUCCESS && h_count_operation(H_ATTACH_OK, NULL) == 0);
    h_gate_release(); p04_host_thread_join(thread);
    expect(first.result == STATUS_SUCCESS && h_count_operation(H_ATTACH_OK, NULL) == 1); finish_fixture();
}
static void setup_failure(HOperation operation, const HOperation *expected, size_t count)
{
    start_fixture(); h_fail_next(operation);
    expect(P04AddDevice(h_driver(), h_pdo()) == STATUS_SUCCESS);
    h_expect_setup(expected, count);
    expect(g_P04.Target == NULL && g_P04.Binding == P04_UNBOUND && h_live_devices() == 0);
    expect(h_count_operation(H_DETACH, NULL) == 0 && h_count_operation(H_ANDWAIT_ENTER, NULL) == 0);
    finish_fixture();
}
static void add_create_fdo_failure(void)
{ const HOperation e[] = {H_CREATE_FDO}; setup_failure(H_CREATE_FDO,e,sizeof(e)/sizeof(e[0])); }
static void add_create_cdo_failure(void)
{ const HOperation e[] = {H_CREATE_FDO,H_INITIALIZE_FDO,H_CREATE_CDO,H_DELETE_FDO}; setup_failure(H_CREATE_CDO,e,sizeof(e)/sizeof(e[0])); }
static void add_link_failure(void)
{ const HOperation e[] = {H_CREATE_FDO,H_INITIALIZE_FDO,H_CREATE_CDO,H_CREATE_LINK,H_DELETE_CDO,H_DELETE_FDO}; setup_failure(H_CREATE_LINK,e,sizeof(e)/sizeof(e[0])); }
static void add_attach_failure(void)
{ const HOperation e[] = {H_CREATE_FDO,H_INITIALIZE_FDO,H_CREATE_CDO,H_CREATE_LINK,H_ATTACH_FAIL,H_DELETE_LINK,H_DELETE_CDO,H_DELETE_FDO}; setup_failure(H_ATTACH_FAIL,e,sizeof(e)/sizeof(e[0])); }
static void add_inline_dispatch_before_return(void)
{
    start_fixture(); h_inject_attach_dispatch(1); h_inject_early_create(1); attach();
    expect(stats().attempted == 0 && stats().active_contexts == 0 && h_remove_holds(&extension()->RemoveLock) == 0);
    expect(h_pending_requests() == 0 && h_open_handles() == 0); finish_fixture();
}
static void io_inline_success(void)
{
    PIRP value; WinCacheP04Stats s; CommandResult drain;
    running_fixture(); value = request(IRP_MJ_READ,0);
    expect(h_dispatch(h_fdo(),value) == STATUS_SUCCESS); s = stats();
    expect(s.attempted == 2 && s.dropped == 0 && s.active_contexts == 0 && s.queued_records == 2);
    expect(h_remove_holds(&extension()->RemoveLock) == 0 && h_result(value).completed == 1);
    drain = command(0x8337e008u,2);
    expect(drain.returned == STATUS_SUCCESS && drain.result.information == 336);
    expect(get64(drain.bytes+144) == 0 && get64(drain.bytes+152) == 1 && get32(drain.bytes+160) == 1);
    expect(get64(drain.bytes+240) == 1 && get64(drain.bytes+248) == 1 && get32(drain.bytes+256) == 2);
    expect(get32(drain.bytes+308) == 1 && get64(drain.bytes+312) == 512);
    expect(get64(drain.bytes+96) == 2 && get32(drain.bytes+68) == 0 && drain.bytes[336] == 0xa5);
    finish_fixture();
}
static void io_inline_complete_pending_return(void)
{
    PIRP value; HLowerPlan plan = {0,STATUS_PENDING,STATUS_SUCCESS,512,TRUE,FALSE,DISPATCH_LEVEL};
    running_fixture(); value = request(IRP_MJ_READ,0); h_plan(value,plan);
    expect(h_dispatch(h_fdo(),value) == STATUS_PENDING);
    expect(h_result(value).marked == 1 && h_result(value).completed == 1 && stats().active_contexts == 0);
    finish_fixture();
}
static void io_inline_return_after_target_freed(void)
{
    PIRP value, remove; Job dispatching={0}; P04HostThread *thread; PDEVICE_OBJECT target;
    running_fixture(); target=h_fdo(); value=request(IRP_MJ_READ,0);
    dispatching.device=target; dispatching.irp=value; dispatching.label=1;
    h_gate_arm(H_CALL_RETURN_BEFORE,value,1);
    thread=p04_host_thread_start(job_dispatch,&dispatching); h_gate_wait();
    expect(h_result(value).completed==1 && stats().active_contexts==0);
    remove=request(IRP_MJ_PNP,IRP_MN_REMOVE_DEVICE);
    expect(h_dispatch(target,remove)==STATUS_SUCCESS && h_device_freed(target));
    /* Both the completed IRP and the FDO/extension are inaccessible pages
     * before the ordinary dispatch resumes its post-IoCallDriver tail. */
    h_gate_release(); p04_host_thread_join(thread);
    expect(dispatching.result==STATUS_SUCCESS && h_result(value).completed==1);
    finish_fixture();
}
static void io_delayed_completion(void)
{
    PIRP value; running_fixture(); value=request(IRP_MJ_WRITE,0); h_plan(value,delayed(STATUS_SUCCESS));
    expect(h_dispatch(h_fdo(),value) == STATUS_PENDING);
    expect(stats().active_contexts == 1 && h_remove_holds(&extension()->RemoveLock) == 1);
    h_complete_lower(value);
    expect(stats().active_contexts == 0 && stats().attempted == 2 && h_result(value).marked == 1);
    finish_fixture();
}
static void io_cancel_and_error(void)
{
    unsigned i; const NTSTATUS outcomes[] = {STATUS_CANCELLED,STATUS_UNSUCCESSFUL};
    for(i=0;i<2;++i) {
        PIRP value; HLowerPlan plan={0,STATUS_UNSUCCESSFUL,STATUS_UNSUCCESSFUL,0xfedcba9876543210ULL,FALSE,FALSE,DISPATCH_LEVEL};
        CommandResult drain;
        running_fixture(); plan.returned=outcomes[i]; plan.completed=outcomes[i]; plan.cancel=(BOOLEAN)(i==0);
        value=request(IRP_MJ_READ,0); h_plan(value,plan); expect(h_dispatch(h_fdo(),value)==outcomes[i]);
        drain=command(0x8337e008u,2);
        expect(get32(drain.bytes+304)==(ULONG)outcomes[i] && get32(drain.bytes+308)==0 && get64(drain.bytes+312)==0);
        expect(h_result(value).information==0xfedcba9876543210ULL && stats().active_contexts==0); finish_fixture();
    }
}
static void observer_no_context_release_only(void)
{
    PIRP values[65]; unsigned i; WinCacheP04Stats s;
    running_fixture();
    for(i=0;i<65;++i) { values[i]=request(IRP_MJ_READ,0); h_plan(values[i],delayed(STATUS_SUCCESS)); expect(h_dispatch(h_fdo(),values[i])==STATUS_PENDING); }
    s=stats(); expect(s.attempted==65 && s.dropped==1 && s.active_contexts==64 && s.queued_records==64);
    expect(h_remove_holds(&extension()->RemoveLock)==65);
    for(i=0;i<65;++i) h_complete_lower(values[i]);
    expect(stats().attempted==129 && stats().active_contexts==0 && h_remove_holds(&extension()->RemoveLock)==0); finish_fixture();
}
static void observer_stopped_release_only(void)
{
    PIRP value; running_fixture(); expect(command(0x8337e00cu,2).returned==STATUS_SUCCESS);
    value=request(IRP_MJ_READ,0); h_plan(value,delayed(STATUS_SUCCESS)); expect(h_dispatch(h_fdo(),value)==STATUS_PENDING);
    expect(stats().active_contexts==0 && stats().attempted==0 && h_remove_holds(&extension()->RemoveLock)==1);
    h_complete_lower(value); expect(stats().attempted==0 && h_remove_holds(&extension()->RemoveLock)==0); finish_fixture();
}
static void observer_ring_full_does_not_fail_original_io(void)
{
    unsigned i; WinCacheP04Stats s;
    running_fixture();
    for(i=0;i<130;++i) {
        PIRP value=request(IRP_MJ_READ,0);
        expect(h_dispatch(h_fdo(),value)==STATUS_SUCCESS && h_result(value).completed==1);
    }
    s=stats();
    expect(s.attempted==260 && s.dropped==4 && s.queued_records==256 && s.active_contexts==0);
    expect(h_remove_holds(&extension()->RemoveLock)==0); finish_fixture();
}
static void context_retire_reuse_before_old_remove_release(void)
{
    PIRP first, second; Job job={0}; P04HostThread *thread; P04FilterExtension *ext;
    running_fixture(); ext=extension(); first=request(IRP_MJ_READ,0); second=request(IRP_MJ_WRITE,0);
    h_plan(first,delayed(STATUS_SUCCESS)); h_plan(second,delayed(STATUS_SUCCESS));
    expect(h_dispatch(h_fdo(),first)==STATUS_PENDING);
    job.irp=first; job.label=1; h_gate_arm(H_SPIN_RELEASE,&ext->StateLock,1);
    thread=p04_host_thread_start(job_complete,&job); h_gate_wait();
    expect(stats().active_contexts==0 && h_remove_holds(&ext->RemoveLock)==1);
    expect(h_dispatch(h_fdo(),second)==STATUS_PENDING && ext->Callbacks[0].Token.request_id==2);
    h_gate_release(); p04_host_thread_join(thread);
    expect(stats().active_contexts==1 && h_remove_holds(&ext->RemoveLock)==1);
    h_complete_lower(second); expect(stats().attempted==4 && stats().fault_mask==0); finish_fixture();
}
static void remove_wait_case(int observed)
{
    PIRP value, remove; Job job={0}; P04HostThread *thread; PDEVICE_OBJECT target; PIO_REMOVE_LOCK lock;
    running_fixture(); if(!observed) expect(command(0x8337e00cu,2).returned==STATUS_SUCCESS);
    target=h_fdo(); lock=&extension()->RemoveLock;
    value=request(IRP_MJ_READ,0); remove=request(IRP_MJ_PNP,IRP_MN_REMOVE_DEVICE);
    h_plan(value,delayed(STATUS_SUCCESS)); expect(h_dispatch(target,value)==STATUS_PENDING);
    job.device=target; job.irp=remove; job.label=2; thread=p04_host_thread_start(job_dispatch,&job);
    h_wait_operation(H_ANDWAIT_ENTER,lock,1);
    expect(!h_device_freed(target) && h_count_operation(H_DETACH,NULL)==0 && g_P04.Target==NULL);
    h_complete_lower(value); p04_host_thread_join(thread);
    expect(job.result==STATUS_SUCCESS && h_device_freed(target) && h_count_operation(H_DETACH,NULL)==1);
    expect(h_result(remove).installed==0 && h_result(remove).completed==1); finish_fixture();
}
static void remove_waits_for_tracked_completion(void) { remove_wait_case(1); }
static void remove_waits_for_unobserved_completion(void) { remove_wait_case(0); }
static void active_zero_is_not_rundown(void)
{
    PIRP value, remove; Job completing={0},removing={0}; P04HostThread *t1,*t2; P04FilterExtension *ext; PDEVICE_OBJECT target;
    running_fixture(); ext=extension(); target=h_fdo(); value=request(IRP_MJ_READ,0); remove=request(IRP_MJ_PNP,IRP_MN_REMOVE_DEVICE);
    h_plan(value,delayed(STATUS_SUCCESS)); expect(h_dispatch(target,value)==STATUS_PENDING);
    completing.irp=value; completing.label=1; h_gate_arm(H_SPIN_RELEASE,&ext->StateLock,1);
    t1=p04_host_thread_start(job_complete,&completing); h_gate_wait(); expect(stats().active_contexts==0);
    removing.device=target; removing.irp=remove; removing.label=2; t2=p04_host_thread_start(job_dispatch,&removing);
    h_wait_operation(H_ANDWAIT_ENTER,&ext->RemoveLock,1); expect(!h_device_freed(target));
    h_gate_release(); p04_host_thread_join(t1); p04_host_thread_join(t2); expect(h_device_freed(target)); finish_fixture();
}
static void remove_waits_for_control_call_not_open_handle(void)
{
    UCHAR *buffer; PIRP control,remove; Job controlling={0},removing={0}; P04HostThread *t1,*t2;
    P04FilterExtension *ext; PDEVICE_OBJECT target;
    running_fixture(); ext=extension(); target=h_fdo();
    control=prepare_command(&file_a,0x8337e004u,7,2,0,48,144,&buffer); remove=request(IRP_MJ_PNP,IRP_MN_REMOVE_DEVICE);
    controlling.device=h_cdo(); controlling.irp=control; controlling.label=1;
    h_gate_arm(H_SPIN_RELEASE,&g_P04.PublicationLock,1); t1=p04_host_thread_start(job_dispatch,&controlling); h_gate_wait();
    removing.device=target; removing.irp=remove; removing.label=2; t2=p04_host_thread_start(job_dispatch,&removing);
    h_wait_operation(H_ANDWAIT_ENTER,&ext->RemoveLock,1);
    expect(!h_device_freed(target) && h_open_handles()==1);
    h_gate_release(); p04_host_thread_join(t1); p04_host_thread_join(t2);
    expect(controlling.result==STATUS_SUCCESS && h_device_freed(target));
    expect(h_device_delete_pending(h_cdo()) && !h_device_freed(h_cdo())); finish_fixture();
}
static void cdo_close_after_fdo_deleted(void)
{
    PDEVICE_OBJECT target,control; PIRP remove;
    running_fixture(); target=h_fdo(); control=h_cdo(); remove=request(IRP_MJ_PNP,IRP_MN_REMOVE_DEVICE);
    expect(h_dispatch(target,remove)==STATUS_SUCCESS && h_device_freed(target));
    expect(h_device_delete_pending(control) && !h_device_freed(control));
    close_a(); expect(h_device_freed(control)); finish_fixture();
}
static void start_lower_pending_bottom_up(void)
{
    PIRP value; Job job={0}; P04HostThread *thread;
    start_fixture(); attach(); value=request(IRP_MJ_PNP,IRP_MN_START_DEVICE); h_plan(value,delayed(STATUS_SUCCESS));
    job.device=h_fdo(); job.irp=value; job.label=1; thread=p04_host_thread_start(job_dispatch,&job);
    h_wait_operation(H_EVENT_WAIT,NULL,1); expect(extension()->PnpState==0 && h_result(value).completed==0);
    create_a(); expect(command(0x8337e000u,1).returned==STATUS_DEVICE_NOT_READY);
    h_complete_lower(value); p04_host_thread_join(thread);
    expect(job.result==STATUS_SUCCESS && extension()->PnpState==1 && h_result(value).marked==0);
    expect(command(0x8337e000u,1).returned==STATUS_SUCCESS); finish_fixture();
}
static void start_lower_failure(void)
{
    PIRP value; HLowerPlan plan={0,STATUS_UNSUCCESSFUL,STATUS_UNSUCCESSFUL,0xdeadbeef12345678ULL,FALSE,FALSE,DISPATCH_LEVEL};
    start_fixture(); attach(); value=request(IRP_MJ_PNP,IRP_MN_START_DEVICE); h_plan(value,plan);
    expect(h_dispatch(h_fdo(),value)==STATUS_UNSUCCESSFUL && extension()->PnpState==0);
    expect(h_result(value).information==0xdeadbeef12345678ULL && h_result(value).completed==1); finish_fixture();
}
static void cancel_pending(int remove)
{
    PIRP query,cancel,create; Job job={0}; P04HostThread *thread;
    running_fixture(); query=request(IRP_MJ_PNP,(UCHAR)(remove?IRP_MN_QUERY_REMOVE_DEVICE:IRP_MN_QUERY_STOP_DEVICE));
    expect(h_dispatch(h_fdo(),query)==STATUS_SUCCESS);
    cancel=request(IRP_MJ_PNP,(UCHAR)(remove?IRP_MN_CANCEL_REMOVE_DEVICE:IRP_MN_CANCEL_STOP_DEVICE));
    h_plan(cancel,delayed(STATUS_SUCCESS)); job.device=h_fdo(); job.irp=cancel; job.label=1;
    thread=p04_host_thread_start(job_dispatch,&job); h_wait_operation(H_EVENT_WAIT,NULL,1);
    expect(extension()->PnpState==(remove?4u:2u));
    if(remove) { create=request(IRP_MJ_CREATE,0); expect(!NT_SUCCESS(h_dispatch(h_fdo(),create)) && h_result(create).forwarded==0); }
    h_complete_lower(cancel); p04_host_thread_join(thread);
    expect(job.result==STATUS_SUCCESS && extension()->PnpState==1 && h_result(cancel).marked==0); finish_fixture();
}
static void cancel_remove_lower_pending_bottom_up(void) { cancel_pending(1); }
static void cancel_stop_lower_pending_bottom_up(void) { cancel_pending(0); }
static void spurious_cancels(void)
{
    PIRP first,second; running_fixture(); first=request(IRP_MJ_PNP,IRP_MN_CANCEL_REMOVE_DEVICE);
    second=request(IRP_MJ_PNP,IRP_MN_CANCEL_STOP_DEVICE);
    expect(h_dispatch(h_fdo(),first)==STATUS_SUCCESS && h_dispatch(h_fdo(),second)==STATUS_SUCCESS);
    expect(extension()->PnpState==1 && stats().observer_state==1 && stats().skipped==2); finish_fixture();
}
static void query_stop_resource_free_forward(void)
{
    PIRP query,value,cancel; HLowerPlan plan={0,STATUS_UNSUCCESSFUL,STATUS_UNSUCCESSFUL,0x123456789abcdef0ULL,FALSE,FALSE,DISPATCH_LEVEL};
    running_fixture(); query=request(IRP_MJ_PNP,IRP_MN_QUERY_STOP_DEVICE); h_plan(query,plan);
    expect(h_dispatch(h_fdo(),query)==STATUS_UNSUCCESSFUL && extension()->PnpState==2);
    value=request(IRP_MJ_READ,0); expect(h_dispatch(h_fdo(),value)==STATUS_SUCCESS && h_result(value).forwarded==1);
    expect(h_result(query).information==0x123456789abcdef0ULL);
    cancel=request(IRP_MJ_PNP,IRP_MN_CANCEL_STOP_DEVICE); expect(h_dispatch(h_fdo(),cancel)==STATUS_SUCCESS);
    expect(extension()->PnpState==1); finish_fixture();
}
static void stop_preserves_late_callback_state(void)
{
    PIRP value,stop,restarted; WinCacheP04Stats s;
    running_fixture(); value=request(IRP_MJ_READ,0); h_plan(value,delayed(STATUS_SUCCESS));
    expect(h_dispatch(h_fdo(),value)==STATUS_PENDING); stop=request(IRP_MJ_PNP,IRP_MN_STOP_DEVICE);
    expect(h_dispatch(h_fdo(),stop)==STATUS_SUCCESS && extension()->PnpState==3);
    s=stats(); expect(s.observer_state==2 && s.active_contexts==1 && s.skipped==1);
    h_complete_lower(value); expect(stats().observer_state==3 && stats().active_contexts==0);
    restarted=request(IRP_MJ_PNP,IRP_MN_START_DEVICE); expect(h_dispatch(h_fdo(),restarted)==STATUS_SUCCESS);
    expect(extension()->PnpState==1 && stats().observer_state==3 && stats().skipped==1); finish_fixture();
}
static void surprise_then_remove(void)
{
    PIRP value,surprise; running_fixture(); value=request(IRP_MJ_READ,0); h_plan(value,delayed(STATUS_SUCCESS));
    expect(h_dispatch(h_fdo(),value)==STATUS_PENDING); surprise=request(IRP_MJ_PNP,IRP_MN_SURPRISE_REMOVAL);
    expect(h_dispatch(h_fdo(),surprise)==STATUS_SUCCESS && extension()->PnpState==5);
    expect(h_count_operation(H_DETACH,NULL)==0 && stats().active_contexts==1);
    h_complete_lower(value); finish_fixture();
}
static void remove_without_start(void) { start_fixture(); attach(); finish_fixture(); }
static void cancel_remove_restores_not_started(void)
{
    PIRP query,cancel;
    start_fixture(); attach(); query=request(IRP_MJ_PNP,IRP_MN_QUERY_REMOVE_DEVICE);
    cancel=request(IRP_MJ_PNP,IRP_MN_CANCEL_REMOVE_DEVICE);
    expect(h_dispatch(h_fdo(),query)==STATUS_SUCCESS && extension()->PnpState==4);
    expect(h_dispatch(h_fdo(),cancel)==STATUS_SUCCESS && extension()->PnpState==0);
    expect(stats().observer_state==0 && stats().fault_mask==0); finish_fixture();
}
static void removed_binding_never_rebinds(void)
{
    PIRP remove; unsigned creates;
    start_fixture(); attach(); remove=request(IRP_MJ_PNP,IRP_MN_REMOVE_DEVICE);
    expect(h_dispatch(h_fdo(),remove)==STATUS_SUCCESS && g_P04.Binding==P04_RETIRED);
    creates=h_count_operation(H_CREATE_FDO,NULL);
    expect(P04AddDevice(h_driver(),h_pdo())==STATUS_SUCCESS && h_count_operation(H_CREATE_FDO,NULL)==creates);
    expect(g_P04.Target==NULL); finish_fixture();
}
static void power_dispatch_irql_matrix(void)
{
    KIRQL level;
    for(level=0;level<=2;++level) {
        PIRP value; running_fixture(); value=request(IRP_MJ_POWER,2); h_set_irql(level);
        expect(h_dispatch(h_fdo(),value)==STATUS_SUCCESS); h_set_irql(PASSIVE_LEVEL);
        expect(h_result(value).installed==1 && h_count_operation(H_EVENT_WAIT,NULL)==0 && stats().skipped==1);
        finish_fixture();
    }
}
static void usage_case(ULONG type, BOOLEAN in_path, int started)
{
    PIRP value; ULONG flags; HLowerPlan plan={0,STATUS_UNSUCCESSFUL,STATUS_UNSUCCESSFUL,0xfedcba9876543210ULL,FALSE,FALSE,DISPATCH_LEVEL};
    start_fixture(); attach(); if(started) { start(); create_a(); expect(command(0x8337e000u,1).returned==STATUS_SUCCESS); }
    flags=h_fdo()->Flags; value=request(IRP_MJ_PNP,IRP_MN_DEVICE_USAGE_NOTIFICATION);
    value->Stack[0].Parameters.UsageNotification.Type=(DEVICE_USAGE_NOTIFICATION_TYPE)type;
    value->Stack[0].Parameters.UsageNotification.InPath=in_path; h_plan(value,plan);
    if(in_path) {
        h_expect(h_dispatch(h_fdo(),value)==STATUS_UNSUCCESSFUL, "handled_usage_veto_failure_status");
        expect(h_result(value).forwarded==0);
        expect(h_result(value).information==0xfedcba9876543210ULL);
        expect((stats().fault_mask&0x100u)!=0 && stats().skipped==(started?1u:0u));
    } else {
        expect(h_dispatch(h_fdo(),value)==STATUS_UNSUCCESSFUL && h_result(value).forwarded==1);
        expect(h_result(value).information==0xfedcba9876543210ULL && (stats().fault_mask&0x100u)==0);
    }
    expect(h_fdo()->Flags==flags);
    if(!started && in_path) { start(); create_a(); expect(command(0x8337e000u,1).returned!=STATUS_SUCCESS && stats().observer_state==0); }
    finish_fixture();
}
static void usage_0_through_7_true_false(void)
{ ULONG type; for(type=0;type<=7;++type) { usage_case(type,FALSE,1); usage_case(type,TRUE,1); } }
static void usage_future_true_false(void) { usage_case(0x7fffffffu,FALSE,1); usage_case(0x7fffffffu,TRUE,1); }
static void usage_noncanonical_boolean_true(void)
{ ULONG type; for(type=0;type<=7;++type) usage_case(type,2,1); usage_case(0x7fffffffu,2,1); }
static void usage_postdisplay_before_start(void) { usage_case(5,TRUE,0); }
static void control_buffer_alias_and_truncation(void)
{
    UCHAR *bytes; PIRP value; CommandResult result; WinCacheP04Stats before;
    running_fixture(); value=request(IRP_MJ_READ,0); expect(h_dispatch(h_fdo(),value)==STATUS_SUCCESS); before=stats();
    value=prepare_command(&file_a,0x8337e008u,7,2,16,48,1679,&bytes);
    expect(h_dispatch(h_cdo(),value)==STATUS_BUFFER_TOO_SMALL && h_result(value).information==0);
    expect(stats().queued_records==before.queued_records && extension()->LastCallId==1 && memcmp(bytes,"WCHP04RQ",8)==0);
    value=prepare_command(&file_a,0x8337e004u,7,2,0,47,144,&bytes);
    expect(h_dispatch(h_cdo(),value)==STATUS_INVALID_PARAMETER && extension()->LastCallId==1);
    result=command(0x8337e008u,2); expect(result.returned==STATUS_SUCCESS && get64(result.bytes+32)==7 && get64(result.bytes+48)==2);
    expect(get32(result.bytes+24)==2 && get64(result.bytes+96)==2);
    result=command(0x8337e004u,2); expect(result.returned==STATUS_INVALID_PARAMETER && result.result.information==0); finish_fixture();
}
static void control_second_fileobject_denied(void)
{
    running_fixture(); expect(file_irp(&file_b,IRP_MJ_CREATE)==STATUS_SHARING_VIOLATION);
    expect(command_for(&file_b,0x8337e004u,2).returned==STATUS_ACCESS_DENIED);
    expect(extension()->LastCallId==1); finish_fixture();
}
static void control_cleanup_during_remove(void)
{
    PIRP value,remove; Job job={0}; P04HostThread *thread; PIO_REMOVE_LOCK lock;
    running_fixture(); lock=&extension()->RemoveLock; value=request(IRP_MJ_READ,0); remove=request(IRP_MJ_PNP,IRP_MN_REMOVE_DEVICE);
    h_plan(value,delayed(STATUS_SUCCESS)); expect(h_dispatch(h_fdo(),value)==STATUS_PENDING);
    job.device=h_fdo(); job.irp=remove; job.label=2; thread=p04_host_thread_start(job_dispatch,&job);
    h_wait_operation(H_ANDWAIT_ENTER,lock,1);
    expect(file_irp(&file_a,IRP_MJ_CLEANUP)==STATUS_SUCCESS && h_remove_holds(lock)==1);
    h_complete_lower(value); p04_host_thread_join(thread); finish_fixture();
}
static void cleanup_before_begin_no_session(void)
{
    Job job={0}; P04HostThread *thread; UCHAR *bytes;
    start_fixture(); attach(); start(); create_a();
    job.device=h_cdo(); job.label=1; job.irp=prepare_command(&file_a,0x8337e000u,7,1,0,48,144,&bytes);
    h_gate_arm(H_SPIN_ACQUIRE_BEFORE,&g_P04.PublicationLock,1); thread=p04_host_thread_start(job_dispatch,&job); h_gate_wait();
    expect(file_irp(&file_a,IRP_MJ_CLEANUP)==STATUS_SUCCESS);
    h_gate_release(); p04_host_thread_join(thread);
    expect(job.result==STATUS_ACCESS_DENIED && stats().observer_state==0 && stats().fault_mask==0 && extension()->LastCallId==0);
    finish_fixture();
}
static void begin_before_cleanup_stops_session(void)
{
    running_fixture(); expect(file_irp(&file_a,IRP_MJ_CLEANUP)==STATUS_SUCCESS);
    expect(stats().observer_state==3 && (stats().fault_mask&0x20u)!=0 && extension()->LastCallId==1);
    expect(command(0x8337e004u,2).returned==STATUS_ACCESS_DENIED); finish_fixture();
}
static void close_before_begin_does_not_poison_session(void)
{
    start_fixture(); attach(); start(); create_a(); close_a();
    expect(stats().observer_state==0 && stats().fault_mask==0 && extension()->LastCallId==0);
    create_b(); expect(command_for(&file_b,0x8337e000u,1).returned==STATUS_SUCCESS); finish_fixture();
}
static void control_old_close_preserves_new_owner(void)
{
    start_fixture(); attach(); start(); create_a(); expect(file_irp(&file_a,IRP_MJ_CLEANUP)==STATUS_SUCCESS);
    create_b(); expect(command_for(&file_b,0x8337e000u,1).returned==STATUS_SUCCESS);
    expect(file_irp(&file_a,IRP_MJ_CLOSE)==STATUS_SUCCESS); open_a=0;
    expect(command_for(&file_b,0x8337e004u,2).returned==STATUS_SUCCESS);
    expect(command_for(&file_a,0x8337e004u,3).returned==STATUS_ACCESS_DENIED);
    expect(stats().observer_state==1 && stats().fault_mask==0); finish_fixture();
}

#define RUN(name) do { name(); pass(#name); } while (0)
int main(void)
{
    RUN(add_absent_no_attach); RUN(add_malformed_no_attach); RUN(add_mismatch_no_attach); RUN(add_exact_once);
    RUN(add_concurrent_exact_single_claim); RUN(add_create_fdo_failure); RUN(add_create_cdo_failure);
    RUN(add_link_failure); RUN(add_attach_failure); RUN(add_inline_dispatch_before_return);
    RUN(io_inline_success); RUN(io_inline_complete_pending_return); RUN(io_inline_return_after_target_freed);
    RUN(io_delayed_completion); RUN(io_cancel_and_error);
    RUN(observer_no_context_release_only); RUN(observer_stopped_release_only); RUN(observer_ring_full_does_not_fail_original_io);
    RUN(context_retire_reuse_before_old_remove_release);
    RUN(remove_waits_for_tracked_completion); RUN(remove_waits_for_unobserved_completion); RUN(active_zero_is_not_rundown);
    RUN(remove_waits_for_control_call_not_open_handle); RUN(cdo_close_after_fdo_deleted);
    RUN(start_lower_pending_bottom_up); RUN(start_lower_failure); RUN(cancel_remove_lower_pending_bottom_up);
    RUN(cancel_stop_lower_pending_bottom_up); RUN(spurious_cancels); RUN(query_stop_resource_free_forward);
    RUN(stop_preserves_late_callback_state); RUN(surprise_then_remove); RUN(remove_without_start);
    RUN(cancel_remove_restores_not_started); RUN(removed_binding_never_rebinds);
    RUN(power_dispatch_irql_matrix); RUN(usage_0_through_7_true_false); RUN(usage_future_true_false);
    RUN(usage_noncanonical_boolean_true); RUN(usage_postdisplay_before_start); RUN(control_buffer_alias_and_truncation);
    RUN(control_second_fileobject_denied); RUN(control_cleanup_during_remove); RUN(cleanup_before_begin_no_session);
    RUN(begin_before_cleanup_stops_session); RUN(close_before_begin_does_not_poison_session); RUN(control_old_close_preserves_new_owner);
    h_destroy(); printf("P04_SUITE_PASS suite=driver cases=%u\n",passed); return 0;
}
