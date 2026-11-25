/* Windows x64 test transport. The only stgtest target is the local pipe created below. */
#define WIN32_LEAN_AND_MEAN
#define _WIN32_WINNT 0x0602
#include <windows.h>
#include <objbase.h>
#define _NTSCSI_USER_MODE_
#include <scsi.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include <wchar.h>
#include "ioctl.h"  /* unmodified header from the pinned WinSpd distribution */

#ifdef FILE_CACHE_SDK
#include "winspd_sdk.h"
#endif

enum { FRAME = 48, MAX_DATA = 65536, MAX_MESSAGE = FRAME + MAX_DATA };
_Static_assert(sizeof(void *) == 8, "requires x64");
_Static_assert(sizeof(SPD_IOCTL_STORAGE_UNIT_PARAMS) == 128, "geometry ABI");
_Static_assert(sizeof(SPD_IOCTL_TRANSACT_REQ) == 32, "request ABI");
_Static_assert(sizeof(SPD_IOCTL_TRANSACT_RSP) == FRAME, "response ABI");
_Static_assert(offsetof(SPD_IOCTL_TRANSACT_REQ, Kind) == 8, "kind offset");
_Static_assert(offsetof(SPD_IOCTL_TRANSACT_REQ, Op.Read.BlockAddress) == 16, "LBA offset");
_Static_assert(offsetof(SPD_IOCTL_TRANSACT_REQ, Op.Read.BlockCount) == 24, "count offset");
_Static_assert(offsetof(SPD_IOCTL_TRANSACT_RSP, Status.ScsiStatus) == 16, "status offset");
_Static_assert(sizeof(SPD_IOCTL_UNMAP_DESCRIPTOR) == 16, "unmap descriptor ABI");

typedef struct {
    const wchar_t *name, *ops;
    DWORD count;
    BOOL cached, expect_scsi_error;
} Scenario;

static const Scenario cases[] = {
#ifdef FILE_CACHE_SDK
    {L"read-after-write-error", L"WR", 2, FALSE, TRUE}, {L"close-error", L"WR", 2, TRUE, FALSE},
    {L"ro-warm",L"RR",2,TRUE,FALSE}, {L"ro-write",L"RRW",3,TRUE,TRUE},
    {L"ro-nocache-flush",L"F",1,FALSE,TRUE}, {L"ro-fua-write",L"W",1,FALSE,TRUE},
    {L"ro-unmap",L"RRU",3,TRUE,TRUE}, {L"ro-fua",L"R",1,FALSE,FALSE}, {L"ro-flush",L"F",1,TRUE,TRUE},
    {L"abrupt-write", L"RRW", 3, TRUE, FALSE},
    {L"lost-before", L"RRW", 3, TRUE, FALSE}, {L"lost-after", L"RRW", 3, TRUE, FALSE},
    {L"recover-read", L"RR", 2, TRUE, FALSE},
    {L"inner-busy", L"R", 1, FALSE, FALSE}, {L"callback-mismatch", L"W", 1, FALSE, TRUE},
#endif
    {L"warm", L"WRRF", 8, TRUE, FALSE}, {L"fua", L"WRF", 3, FALSE, FALSE},
    {L"write-error", L"W", 1, FALSE, TRUE}, {L"read-error", L"R", 1, FALSE, TRUE},
    {L"flush-error", L"F", 1, TRUE, TRUE}, {L"unmap", L"U", 1, TRUE, TRUE},
    {L"corrupt-reply", L"WR", 2, TRUE, FALSE},
    {L"peer-check", L"R", 1, TRUE, FALSE}, {L"deadline", L"R", 1, TRUE, FALSE}
};

static HANDLE finished, reply_pending, client_process;
static BOOL stall_test;
#ifdef FILE_CACHE_SDK
static BOOL abrupt_test, protected_geometry;
static volatile LONG writer_gone;
#endif
static BYTE frame[MAX_MESSAGE];

static BOOL read_exact(HANDLE f, void *data, DWORD count) {
    BYTE *pos = data;
    while (count) {
        DWORD n;
        if (!ReadFile(f, pos, count, &n, NULL) || !n) return FALSE;
        pos += n;
        count -= n;
    }
    return TRUE;
}

static BOOL write_exact(HANDLE f, const void *data, DWORD count) {
    const BYTE *pos = data;
    while (count) {
        DWORD n;
        if (!WriteFile(f, pos, count, &n, NULL) || !n) return FALSE;
        pos += n;
        count -= n;
    }
    return TRUE;
}

static BOOL write_message(HANDLE pipe, const void *data, DWORD count) {
    DWORD sent;
    return WriteFile(pipe, data, count, &sent, NULL) && sent == count;
}

static BOOL stop_and_reap(HANDLE process) {
    if (WaitForSingleObject(process, 0) == WAIT_TIMEOUT &&
        !TerminateProcess(process, ERROR_CANCELLED) &&
        WaitForSingleObject(process, 0) != WAIT_OBJECT_0) return FALSE;
    return WaitForSingleObject(process, 3000) == WAIT_OBJECT_0;
}

static DWORD WINAPI guard_deadline(void *unused) {
    (void)unused;
    HANDLE events[] = {finished, reply_pending};
    DWORD result = WaitForMultipleObjects(stall_test ? 2u : 1u, events, FALSE, 20000);
    if (result == WAIT_OBJECT_0) return 0;
    BOOL stalled_reply = stall_test && result == WAIT_OBJECT_0 + 1;
    if (stalled_reply && WaitForSingleObject(finished, 2000) == WAIT_OBJECT_0) return 0;
    HANDLE p = InterlockedCompareExchangePointer(&client_process, NULL, NULL);
    BOOL reaped = p && stop_and_reap(p);
    fprintf(stderr, "BROKER deadline reaped=%u stage=%s\n", (unsigned)reaped,
            stalled_reply ? "reply" : "overall");
    fflush(stderr);
    ExitProcess(ERROR_TIMEOUT);
}

/* The broker itself enters this fresh job before spawning anything. The kernel
   closes the last job handle if the broker dies, including during process creation. */
static HANDLE own_job(void) {
    HANDLE job = CreateJobObjectW(NULL, NULL);
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION limit = {0};
    limit.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if (!job) return NULL;
    if (!SetInformationJobObject(job, JobObjectExtendedLimitInformation, &limit, sizeof limit) ||
        !AssignProcessToJobObject(job, GetCurrentProcess())) {
        CloseHandle(job);
        return NULL;
    }
    return job; /* deliberately kept for the entire broker process lifetime */
}

static BOOL start_client(const wchar_t *pipe_name, const Scenario *s, PROCESS_INFORMATION *pi, HANDLE *log) {
    wchar_t app[32768], cmd[400];
    DWORD len = GetModuleFileNameW(NULL, app, ARRAYSIZE(app));
    if (!len || len >= ARRAYSIZE(app)) return FALSE;
    wchar_t *base = wcsrchr(app, L'\\');
    if (!base || wcscpy_s(base + 1, ARRAYSIZE(app) - (size_t)(base + 1 - app), L"stgtest-x64.exe")) return FALSE;
    if (swprintf_s(cmd, ARRAYSIZE(cmd), L"stgtest-x64.exe -s 41 %ls %lu %ls 7 2",
                   pipe_name, s->count, s->ops) < 0) return FALSE;

    SECURITY_ATTRIBUTES attr = {sizeof attr, NULL, TRUE};
    HANDLE input = NULL, input_writer = NULL, output = NULL, log_reader = NULL;
    SIZE_T bytes = 0;
    STARTUPINFOEXW start = {0};
    BOOL ok = FALSE, initialized = FALSE;
    start.StartupInfo.cb = sizeof start;
    if (!CreatePipe(&input, &input_writer, &attr, 0) || !CreatePipe(&log_reader, &output, &attr, 4096)) goto out;
    CloseHandle(input_writer); input_writer = NULL;
    if (!SetHandleInformation(log_reader, HANDLE_FLAG_INHERIT, 0)) goto out;
    InitializeProcThreadAttributeList(NULL, 1, 0, &bytes);
    start.lpAttributeList = HeapAlloc(GetProcessHeap(), 0, bytes);
    if (!start.lpAttributeList || !InitializeProcThreadAttributeList(start.lpAttributeList, 1, 0, &bytes)) goto out;
    initialized = TRUE;
    HANDLE handles[] = {input, output};
    if (!UpdateProcThreadAttribute(start.lpAttributeList, 0, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                                   handles, sizeof handles, NULL, NULL)) goto out;
    start.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    start.StartupInfo.hStdInput = input;
    start.StartupInfo.hStdOutput = start.StartupInfo.hStdError = output;
    ok = CreateProcessW(app, cmd, NULL, NULL, TRUE, EXTENDED_STARTUPINFO_PRESENT | CREATE_NO_WINDOW,
                        NULL, NULL, &start.StartupInfo, pi);
    if (ok) { *log = log_reader; log_reader = NULL; }
out:
    if (initialized) DeleteProcThreadAttributeList(start.lpAttributeList);
    if (start.lpAttributeList) HeapFree(GetProcessHeap(), 0, start.lpAttributeList);
    if (input) CloseHandle(input);
    if (input_writer) CloseHandle(input_writer);
    if (output) CloseHandle(output);
    if (log_reader) CloseHandle(log_reader);
    return ok;
}

static BOOL rust_transfer(BYTE *message,DWORD size,DWORD *length) {
    HANDLE in=GetStdHandle(STD_INPUT_HANDLE), out=GetStdHandle(STD_OUTPUT_HANDLE);
    SPD_IOCTL_TRANSACT_REQ req;
    SPD_IOCTL_TRANSACT_RSP rsp;
    memcpy(&req,message,sizeof req);
    if (!write_exact(out,&size,sizeof size) || !write_exact(out,message,size) ||
        (stall_test && !SetEvent(reply_pending))) return FALSE;
#ifdef FILE_CACHE_SDK
    if (abrupt_test && req.Kind==SpdIoctlTransactWriteKind) {
        // The orchestrator releases this pipe only after its file worker is reaped.
        BYTE unexpected;
        DWORD bytes=0;
        if (!ReadFile(in,&unexpected,1,&bytes,NULL) && bytes==0 && GetLastError()==ERROR_BROKEN_PIPE)
            InterlockedExchange(&writer_gone,1);
        return FALSE; /* any actual response byte invalidates this fault case */
    }
#endif
    if (!read_exact(in,length,sizeof *length)) return FALSE;
    if (*length < FRAME || *length > MAX_MESSAGE || !read_exact(in,message,*length)) return FALSE;
    memcpy(&rsp,message,sizeof rsp);
    UINT64 data = rsp.Status.ScsiStatus == 0 && req.Kind == SpdIoctlTransactReadKind ?
        (UINT64)req.Op.Read.BlockCount*512 : 0;
    return rsp.Hint==req.Hint && rsp.Kind==req.Kind && data<=MAX_DATA && *length==FRAME+data;
}

#ifdef FILE_CACHE_SDK
static BOOL rust_close_flush(DWORD *error) {
    DWORD tag=0x464c5348;
    return write_exact(GetStdHandle(STD_OUTPUT_HANDLE),&tag,sizeof tag) &&
           read_exact(GetStdHandle(STD_INPUT_HANDLE),error,sizeof *error);
}
#endif

typedef enum { DELIVER, CUT_BEFORE, CUT_AFTER } Delivery;

static BOOL exchange(HANDLE pipe,DWORD thread_id,BOOL mismatch,Delivery delivery) {
    DWORD size,reply=0;
    SPD_IOCTL_TRANSACT_REQ req;
    if (!ReadFile(pipe,frame,sizeof frame,&size,NULL) || size<FRAME) return FALSE;
    memcpy(&req,frame,sizeof req);
    if ((DWORD)(req.Hint >> 32)!=thread_id) return FALSE;
#ifdef FILE_CACHE_SDK
    fprintf(stderr,"SDK outer-request kind=%u\n",(unsigned)req.Kind);
    if (protected_geometry && req.Kind==SpdIoctlTransactWriteKind && size==FRAME+1024)
        fprintf(stderr,"PROTECTED_WRITE lba=%llu count=%lu first_byte=%u fua=%u\n",
                req.Op.Write.BlockAddress,req.Op.Write.BlockCount,(unsigned)frame[FRAME],(unsigned)req.Op.Write.ForceUnitAccess);
    if (delivery != DELIVER && (req.Kind != SpdIoctlTransactWriteKind ||
        req.Op.Write.BlockAddress != 7 || req.Op.Write.BlockCount != 2 ||
        req.Op.Write.ForceUnitAccess || size != FRAME + 1024)) return FALSE;
    if (delivery != CUT_BEFORE && !sdk_request(frame,size,&reply,mismatch)) {
        if (!abrupt_test || req.Kind != SpdIoctlTransactWriteKind ||
            !InterlockedCompareExchange(&writer_gone,0,0) || !DisconnectNamedPipe(pipe)) return FALSE;
        fprintf(stderr,"ABRUPT worker-eof=1 write_response_sent=0\n");
        return TRUE;
    }
    if (delivery != DELIVER) {
        if (delivery == CUT_AFTER && (reply != FRAME || frame[16] != 0)) return FALSE;
        if (!DisconnectNamedPipe(pipe)) return FALSE;
        fprintf(stderr,"CUT write after_apply=%u write_response_sent=0\n",(unsigned)(delivery==CUT_AFTER));
        return TRUE;
    }
#else
    (void)mismatch;
    if (delivery != DELIVER) return FALSE;
    if (!rust_transfer(frame,size,&reply)) return FALSE;
#endif
    if (!write_message(pipe,frame,reply)) return FALSE;
#ifdef FILE_CACHE_SDK
    if (protected_geometry && frame[16]) {
        SPD_IOCTL_TRANSACT_RSP rsp;
        memcpy(&rsp,frame,sizeof rsp);
        fprintf(stderr,"PROTECTED_REPLY kind=%u scsi=%u sense=%02x/%02x bytes=%lu\n",
                (unsigned)rsp.Kind,(unsigned)rsp.Status.ScsiStatus,(unsigned)rsp.Status.SenseKey,
                (unsigned)rsp.Status.ASC,reply-FRAME);
    }
#endif
    return TRUE;
}

static BOOL client_result(HANDLE client, HANDLE log, BOOL scsi_error, BOOL peer_test, BOOL bad_data, BOOL lost_write) {
    DWORD code, n;
    char text[2049];
    if (WaitForSingleObject(client, 5000) != WAIT_OBJECT_0 || !GetExitCodeProcess(client, &code)) return FALSE;
    DWORD total = 0;
    while (total < sizeof text - 1) {
        if (!ReadFile(log, text + total, (DWORD)(sizeof text - 1 - total), &n, NULL)) {
            if (GetLastError() != ERROR_BROKEN_PIPE) return FALSE;
            break;
        }
        if (!n) break;
        total += n;
    }
    if (total == sizeof text - 1) return FALSE;
    text[total] = 0;
    if (lost_write) {
        BOOL disconnected = code==ERROR_BROKEN_PIPE || code==ERROR_NO_DATA || code==ERROR_PIPE_NOT_CONNECTED;
        char expected[100];
        if (sprintf_s(expected,sizeof expected,"Write(Address=0:7, Count=2): transact error: %lu\n",code)<0) return FALSE;
        if (!disconnected || !strstr(text,expected) || strstr(text,"\nOK\n")) return FALSE;
        fprintf(stderr,"CLIENT_WRITE_TRANSPORT_ERROR lba=7 sectors=2 reaped=1 code=%lu\n",code);
        return TRUE;
    }
    BOOL ok = peer_test ? code != 0 && strstr(text, "OK\n") == NULL :
        bad_data ? code == ERROR_IO_DEVICE && strstr(text, "bad buffer: after Write") != NULL :
        scsi_error ? code == ERROR_IO_DEVICE && strstr(text, "condition fail") != NULL :
        code == 0 && strstr(text, "\nOK\n") != NULL;
    if (ok) fprintf(stderr, "BROKER client-result=%lu reaped=1\n", code);
    return ok;
}

int wmain(int argc, wchar_t **argv) {
    if (argc != 2) return 2;
    const Scenario *s = NULL;
    for (size_t i = 0; i < ARRAYSIZE(cases); ++i)
        if (wcscmp(argv[1], cases[i].name) == 0) s = &cases[i];
    if (!s) return 2;
    HANDLE in = GetStdHandle(STD_INPUT_HANDLE), out = GetStdHandle(STD_OUTPUT_HANDLE);
    if (GetFileType(in) != FILE_TYPE_PIPE || GetFileType(out) != FILE_TYPE_PIPE) return 3;
    if (!own_job()) return 4;
    BOOL peer_test = wcscmp(s->name, L"peer-check") == 0;
#ifdef FILE_CACHE_SDK
    abrupt_test = wcscmp(s->name,L"abrupt-write")==0;
#endif
    stall_test = wcscmp(s->name, L"deadline") == 0;
    finished = CreateEventW(NULL, TRUE, FALSE, NULL);
    reply_pending = CreateEventW(NULL, TRUE, FALSE, NULL);
    if (!finished || !reply_pending) return 5;
    HANDLE watchdog = CreateThread(NULL, 0, guard_deadline, NULL, 0, NULL);
    if (!watchdog) return 6;

    GUID id;
    wchar_t guid[40], name[128];
    if (FAILED(CoCreateGuid(&id)) || !StringFromGUID2(&id, guid, ARRAYSIZE(guid)) ||
        swprintf_s(name, ARRAYSIZE(name), L"\\\\.\\pipe\\file-cache-%ls\\0", guid) < 0) return 7;
    HANDLE pipe = CreateNamedPipeW(name, PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
        PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS, 1,
        MAX_MESSAGE, MAX_MESSAGE, 2000, NULL);
    if (pipe == INVALID_HANDLE_VALUE) return 8;
    PROCESS_INFORMATION pi = {0};
    HANDLE log = NULL;
    int result = 9;
    if (!start_client(name, s, &pi, &log)) goto end;
    InterlockedExchangePointer(&client_process, pi.hProcess);
    if (!ConnectNamedPipe(pipe, NULL) && GetLastError() != ERROR_PIPE_CONNECTED) goto end;
    ULONG peer;
    if (!GetNamedPipeClientProcessId(pipe, &peer) || WaitForSingleObject(pi.hProcess, 0) != WAIT_TIMEOUT) goto end;
    ULONG expected = peer_test ? GetCurrentProcessId() : pi.dwProcessId;
    if (peer != expected) {
        DisconnectNamedPipe(pipe);
        if (peer_test && peer == pi.dwProcessId && client_result(pi.hProcess, log, FALSE, TRUE, FALSE, FALSE)) {
            fprintf(stderr, "BROKER peer-rejected before-geometry\n");
            result = 0;
        }
        goto end;
    }
    if (peer_test) goto end;
    fprintf(stderr, "BROKER peer-verified\n");
    SPD_IOCTL_STORAGE_UNIT_PARAMS params = {0};
    params.Guid = id; params.BlockCount = 131; params.BlockLength = 512;
    memcpy(params.ProductId, "FileCacheLab", 12);
    memcpy(params.ProductRevisionLevel, "0001", 4);
    params.CacheSupported = s->cached ? 1u : 0u;
#ifdef FILE_CACHE_SDK
    params.WriteProtected = wcsncmp(s->name,L"ro-",3)==0 ? 1u : 0u;
#endif
    params.MaxTransferLength = MAX_DATA;
#ifdef FILE_CACHE_SDK
    SPD_IOCTL_STORAGE_UNIT_PARAMS actual={0};
    BOOL busy=wcscmp(s->name,L"inner-busy")==0;
    DWORD start_error=sdk_start(&params,&actual,rust_transfer,rust_close_flush,busy);
    if (start_error) {
        if (busy && start_error==ERROR_PIPE_BUSY) {
            DisconnectNamedPipe(pipe);
            if (client_result(pi.hProcess,log,FALSE,TRUE,FALSE,FALSE)) {
                fprintf(stderr,"SDK inner-busy before-dispatch\n");
                result=0;
            }
        } else { fprintf(stderr,"SDK startup-error=%lu\n",start_error); }
        goto end;
    }
    if (!write_message(pipe,&actual,sizeof actual)) goto end;
    protected_geometry=actual.WriteProtected!=0;
    if (protected_geometry)
        fprintf(stderr,"SDK advertised write_protected=%u unmap=%u\n",(unsigned)actual.WriteProtected,(unsigned)actual.UnmapSupported);
#else
    if (!write_message(pipe, &params, sizeof params)) goto end;
#endif
    Delivery last=DELIVER;
#ifdef FILE_CACHE_SDK
    if (wcscmp(s->name,L"lost-before")==0) last=CUT_BEFORE;
    if (wcscmp(s->name,L"lost-after")==0) last=CUT_AFTER;
#endif
    for (DWORD i=0; i<s->count; ++i) {
        if (!exchange(pipe,pi.dwThreadId,wcscmp(s->name,L"callback-mismatch")==0,
                      i+1==s->count ? last : DELIVER)) goto end;
    }
    BOOL disconnected=last!=DELIVER;
#ifdef FILE_CACHE_SDK
    disconnected=disconnected || (abrupt_test && InterlockedCompareExchange(&writer_gone,0,0));
#endif
    if (client_result(pi.hProcess, log, s->expect_scsi_error, FALSE,
                      wcscmp(s->name,L"corrupt-reply")==0,disconnected)) result=0;
end:
#ifdef FILE_CACHE_SDK
    {
        DWORD stop_error=sdk_stop(result==0 && !InterlockedCompareExchange(&writer_gone,0,0));
        if (stop_error) result=(int)stop_error;
    }
#endif
    if (pi.hProcess && !stop_and_reap(pi.hProcess)) result = 10;
    SetEvent(finished);
    WaitForSingleObject(watchdog, INFINITE);
    if (log) CloseHandle(log);
    if (pi.hThread) CloseHandle(pi.hThread);
    if (pi.hProcess) CloseHandle(pi.hProcess);
    CloseHandle(pipe); CloseHandle(watchdog); CloseHandle(reply_pending); CloseHandle(finished);
    if (result) fprintf(stderr, "BROKER failure=%d win32=%lu\n", result, GetLastError());
    return result;
}
