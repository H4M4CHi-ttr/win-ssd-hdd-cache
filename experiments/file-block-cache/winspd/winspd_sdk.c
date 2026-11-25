/* User-mode WinSpd server adapter for the isolated file experiment. No driver entry points. */
#define WIN32_LEAN_AND_MEAN
#define _WIN32_WINNT 0x0602
#include <windows.h>
#include <objbase.h>
#include <stdio.h>
#include <string.h>
#include <wchar.h>
#include <winspd/winspd.h>
#include "winspd_sdk.h"

enum { HEADER = sizeof(SPD_IOCTL_TRANSACT_RSP), DATA = 65536, LIMIT = HEADER + DATA };
enum Phase { IDLE, ARMED, DONE, STOPPING, ABORTING };
static struct {
    HMODULE dll;
    SPD_STORAGE_UNIT *unit;
    HANDLE inner;
    BOOL started, fatal, cached, write_protected;
    SRWLOCK lock;
    enum Phase phase;
    BYTE expected[LIMIT], answer[LIMIT];
    DWORD expected_size, answer_size, callbacks, rejected, closes, close_error;
    SdkRustExchange transfer;
    SdkRustClose close_flush;
    DWORD (*create)(PWSTR,const SPD_STORAGE_UNIT_PARAMS *,const SPD_STORAGE_UNIT_INTERFACE *,SPD_STORAGE_UNIT **);
    DWORD (*start)(SPD_STORAGE_UNIT *,ULONG);
    VOID (*shutdown)(SPD_STORAGE_UNIT *);
    VOID (*wait)(SPD_STORAGE_UNIT *);
    VOID (*destroy)(SPD_STORAGE_UNIT *);
    SPD_STORAGE_UNIT_OPERATION_CONTEXT *(*context)(void);
} session;

static BOOL exports(void) {
    wchar_t path[32768];
    DWORD n = GetModuleFileNameW(NULL,path,ARRAYSIZE(path));
    if (!n || n >= ARRAYSIZE(path)) return FALSE;
    wchar_t *base = wcsrchr(path,L'\\');
    if (!base || wcscpy_s(base+1,ARRAYSIZE(path)-(size_t)(base+1-path),L"winspd-x64.dll")) return FALSE;
    session.dll = LoadLibraryExW(path,NULL,LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32);
    if (!session.dll) return FALSE;
    wchar_t actual[32768];
    n=GetModuleFileNameW(session.dll,actual,ARRAYSIZE(actual));
    if (!n || n>=ARRAYSIZE(actual) || CompareStringOrdinal(actual,-1,path,-1,TRUE)!=CSTR_EQUAL) return FALSE;
    /* On Windows x64 these are the function pointer types from the public header. */
#define RESOLVE(field, symbol) do { FARPROC p = GetProcAddress(session.dll, symbol); \
    _Static_assert(sizeof p == sizeof session.field, "x64 function pointer ABI"); \
    if (!p) return FALSE; memcpy(&session.field,&p,sizeof p); } while (0)
    RESOLVE(create,"SpdStorageUnitCreate");
    RESOLVE(start,"SpdStorageUnitStartDispatcher");
    RESOLVE(shutdown,"SpdStorageUnitShutdown");
    RESOLVE(wait,"SpdStorageUnitWaitDispatcher");
    RESOLVE(destroy,"SpdStorageUnitDelete");
    RESOLVE(context,"SpdStorageUnitGetOperationContext");
#undef RESOLVE
    return TRUE;
}

static BOOL valid_request(const BYTE *frame, DWORD size) {
    SPD_IOCTL_TRANSACT_REQ q;
    if (size < HEADER || size > LIMIT) return FALSE;
    memcpy(&q,frame,sizeof q);
    if (!q.Hint) return FALSE;
    if (q.Kind == SpdIoctlTransactUnmapKind) {
        SPD_UNMAP_DESCRIPTOR d;
        if (q.Op.Unmap.Count != 1 || size != HEADER + sizeof d) return FALSE;
        memcpy(&d,frame+HEADER,sizeof d);
        return !d.Reserved && d.BlockCount && d.BlockAddress < 131 && d.BlockCount <= 131-d.BlockAddress;
    }
    if (q.Kind != SpdIoctlTransactReadKind && q.Kind != SpdIoctlTransactWriteKind &&
        q.Kind != SpdIoctlTransactFlushKind) return FALSE;
    UINT64 lba=q.Op.Read.BlockAddress; UINT32 count=q.Op.Read.BlockCount;
    if (lba >= 131 || count > 131-lba) return FALSE;
    if (q.Kind != SpdIoctlTransactFlushKind && (!count || count > 128 || q.Op.Read.Reserved)) return FALSE;
    return size == HEADER + (q.Kind == SpdIoctlTransactWriteKind ? count*512u : 0);
}

static void deny_request(SPD_STORAGE_UNIT_STATUS *status,const SPD_IOCTL_TRANSACT_REQ *q,UINT8 key,UINT8 asc) {
    SPD_IOCTL_TRANSACT_RSP rsp={0};
    SpdStorageUnitStatusSetSense(status,key,asc,NULL);
    rsp.Hint=q->Hint; rsp.Kind=q->Kind; rsp.Status=*status;
    memcpy(session.answer,&rsp,sizeof rsp);
    session.answer_size=HEADER; session.phase=DONE;
}

static BOOLEAN dispatch(SPD_STORAGE_UNIT *unit, UINT8 kind, UINT64 first, UINT32 count,
                        BOOLEAN fua, void *buffer, SPD_STORAGE_UNIT_STATUS *status) {
    SPD_STORAGE_UNIT_OPERATION_CONTEXT *tls = session.context();
    SPD_IOCTL_TRANSACT_REQ q={0}, wanted;
    SPD_IOCTL_TRANSACT_RSP rsp;
    DWORD payload=0, reply_size=0;
    AcquireSRWLockExclusive(&session.lock);
    if (unit != session.unit || !tls || !tls->Request || !tls->Response ||
        status != &tls->Response->Status) goto bad;
    if (kind == SpdIoctlTransactReadKind) {
        if (!buffer || buffer != tls->DataBuffer || count > 128) goto bad;
        memset(buffer,0,(size_t)count*512); /* define the failed-read buffer before returning */
    }
    if (session.phase == ABORTING && kind == SpdIoctlTransactFlushKind && !first && !count &&
        !tls->Request->Hint && !tls->Request->Kind && !tls->Response->Hint && !tls->Response->Kind) {
        fprintf(stderr,"SDK abort-close callback=1 forwarded_to_origin=0\n");
        SpdStorageUnitStatusSetSense(status,3,0x0c,NULL);
        goto out;
    }
    if (session.phase == STOPPING && kind == SpdIoctlTransactFlushKind && !first && !count &&
        !tls->Request->Hint && !tls->Request->Kind && !tls->Response->Hint && !tls->Response->Kind &&
        session.cached && !session.closes) {
        ++session.closes;
        if (!session.close_flush(&session.close_error)) goto bad;
        if (session.close_error) SpdStorageUnitStatusSetSense(status,3,0x0c,NULL);
        goto out;
    }
    if (session.phase != ARMED || tls->Request->Kind != kind || !tls->Request->Hint ||
        tls->Response->Hint != tls->Request->Hint || tls->Response->Kind != kind) goto bad;
    memcpy(&wanted,session.expected,sizeof wanted);
    q.Hint=tls->Request->Hint; q.Kind=kind;
    if (kind == SpdIoctlTransactUnmapKind) {
        if (count != 1 || !buffer || buffer != tls->DataBuffer) goto bad;
        q.Op.Unmap.Count=count; payload=sizeof(SPD_UNMAP_DESCRIPTOR);
    } else {
        q.Op.Read.BlockAddress=first; q.Op.Read.BlockCount=count; q.Op.Read.ForceUnitAccess=fua?1u:0u;
        if (kind == SpdIoctlTransactWriteKind) {
            if (count > 128 || !buffer || buffer != tls->DataBuffer) goto bad;
            payload=count*512;
        }
    }
    if (memcmp(&q,&wanted,sizeof q) || session.expected_size != HEADER+payload ||
        (payload && memcmp(buffer,session.expected+HEADER,payload))) {
        ++session.rejected;
        deny_request(status,&q,5,0x24);
        goto out;
    }
    if (session.write_protected && kind!=SpdIoctlTransactReadKind) {
        /* Shutdown Flush is handled separately above. Unmap is not advertised. */
        if (kind==SpdIoctlTransactUnmapKind || (kind==SpdIoctlTransactFlushKind && !session.cached)) deny_request(status,&q,5,0x20);
        else deny_request(status,&q,7,0x27);
        fprintf(stderr,"SDK protection-denied kind=%u\n",(unsigned)kind);
        goto out;
    }
    memset(session.answer,0,HEADER);
    memcpy(session.answer,&q,sizeof q);
    if (payload) memcpy(session.answer+HEADER,buffer,payload);
    ++session.callbacks;
    fprintf(stderr,"SDK callback ordinal=%lu kind=%u\n",session.callbacks,(unsigned)kind);
    if (!session.transfer(session.answer,HEADER+payload,&reply_size) || reply_size < HEADER || reply_size > LIMIT) goto bad;
    memcpy(&rsp,session.answer,sizeof rsp);
    if (rsp.Hint!=q.Hint || rsp.Kind!=kind || reply_size != HEADER+
        ((kind==SpdIoctlTransactReadKind && !rsp.Status.ScsiStatus)?count*512u:0)) goto bad;
    *status=rsp.Status;
    if (reply_size>HEADER) memcpy(buffer,session.answer+HEADER,reply_size-HEADER);
    session.answer_size=reply_size; session.phase=DONE;
    goto out;
bad:
    session.fatal=TRUE;
    SpdStorageUnitStatusSetSense(status,5,0x24,NULL);
out:
    ReleaseSRWLockExclusive(&session.lock);
    return TRUE; /* synchronous completion, including CHECK CONDITION */
}

static BOOLEAN on_read(SPD_STORAGE_UNIT *u,PVOID b,UINT64 l,UINT32 n,BOOLEAN f,SPD_STORAGE_UNIT_STATUS *s) {
    return dispatch(u,SpdIoctlTransactReadKind,l,n,f,b,s);
}
static BOOLEAN on_write(SPD_STORAGE_UNIT *u,PVOID b,UINT64 l,UINT32 n,BOOLEAN f,SPD_STORAGE_UNIT_STATUS *s) {
    return dispatch(u,SpdIoctlTransactWriteKind,l,n,f,b,s);
}
static BOOLEAN on_flush(SPD_STORAGE_UNIT *u,UINT64 l,UINT32 n,SPD_STORAGE_UNIT_STATUS *s) {
    return dispatch(u,SpdIoctlTransactFlushKind,l,n,FALSE,NULL,s);
}
static BOOLEAN on_unmap(SPD_STORAGE_UNIT *u,SPD_UNMAP_DESCRIPTOR *d,UINT32 n,SPD_STORAGE_UNIT_STATUS *s) {
    return dispatch(u,SpdIoctlTransactUnmapKind,0,n,FALSE,d,s);
}
static const SPD_STORAGE_UNIT_INTERFACE methods={on_read,on_write,on_flush,on_unmap,{0}};

static HANDLE connect_local(const wchar_t *name) {
    return CreateFileW(name,GENERIC_READ|FILE_WRITE_DATA|FILE_WRITE_ATTRIBUTES,FILE_SHARE_READ|FILE_SHARE_WRITE,
                       NULL,OPEN_EXISTING,SECURITY_SQOS_PRESENT|SECURITY_IDENTIFICATION,NULL);
}

DWORD sdk_start(const SPD_IOCTL_STORAGE_UNIT_PARAMS *params, SPD_IOCTL_STORAGE_UNIT_PARAMS *geometry,
                SdkRustExchange transfer, SdkRustClose close_flush, BOOL occupy_inner) {
    GUID uuid;
    wchar_t guid[40], name[112], endpoint[128];
    const wchar_t prefix[]=L"\\\\.\\pipe\\";
    if (FAILED(CoCreateGuid(&uuid)) || !StringFromGUID2(&uuid,guid,ARRAYSIZE(guid)) ||
        swprintf_s(name,ARRAYSIZE(name),L"%lsfile-cache-inner-%ls",prefix,guid)<0 ||
        wcsncmp(name,prefix,ARRAYSIZE(prefix)-1)!=0) return ERROR_INVALID_NAME;
    session.transfer=transfer; session.close_flush=close_flush; session.cached=params->CacheSupported!=0;
    session.write_protected=params->WriteProtected!=0;
    if (!exports()) return ERROR_PROC_NOT_FOUND;
    DWORD err=session.create(name,params,&methods,&session.unit);
    if (err) return err;
    if (swprintf_s(endpoint,ARRAYSIZE(endpoint),L"%ls\\%u",name,(unsigned)SPD_IOCTL_BTL_T(session.unit->Btl))<0) return ERROR_INVALID_NAME;
    HANDLE occupied=INVALID_HANDLE_VALUE;
    if (occupy_inner) { occupied=connect_local(endpoint); if (occupied==INVALID_HANDLE_VALUE) return ERROR_INVALID_HANDLE; }
    /* Occupy the sole internal connection before any dispatcher can service a request. */
    session.inner=connect_local(endpoint);
    err=session.inner==INVALID_HANDLE_VALUE?GetLastError():ERROR_SUCCESS;
    if (occupied!=INVALID_HANDLE_VALUE) CloseHandle(occupied);
    if (err) return err;
    if (occupy_inner) return ERROR_INVALID_DATA;
    ULONG server=0; DWORD mode=PIPE_READMODE_MESSAGE;
    if (GetFileType(session.inner)!=FILE_TYPE_PIPE || !GetNamedPipeServerProcessId(session.inner,&server) ||
        server!=GetCurrentProcessId() || !SetNamedPipeHandleState(session.inner,&mode,NULL,NULL)) return ERROR_INVALID_HANDLE;
    err=session.start(session.unit,1);
    if (err) return err;
    session.started=TRUE;
    DWORD n=0;
    if (!ReadFile(session.inner,geometry,sizeof *geometry,&n,NULL) || n!=sizeof *geometry ||
        memcmp(params,geometry,sizeof *geometry)) return ERROR_INVALID_DATA;
    fprintf(stderr,"SDK ready inner-peer=verified dispatchers=1\n");
    return ERROR_SUCCESS;
}

BOOL sdk_request(BYTE *frame,DWORD size,DWORD *response_size,BOOL mismatch) {
    SPD_IOCTL_TRANSACT_REQ q;
    if (!valid_request(frame,size)) return FALSE;
    memcpy(&q,frame,sizeof q);
    AcquireSRWLockExclusive(&session.lock);
    BOOL ok=session.phase==IDLE && !session.fatal && !session.rejected;
    if (ok) {
        memcpy(session.expected,frame,size);
        if (mismatch) session.expected[0]^=1; /* local negative control for the admission check only */
        session.expected_size=size; session.answer_size=0; session.phase=ARMED;
    }
    ReleaseSRWLockExclusive(&session.lock);
    DWORD n=0;
    if (!ok || !WriteFile(session.inner,frame,size,&n,NULL) || n!=size ||
        !ReadFile(session.inner,frame,LIMIT,&n,NULL) || n<HEADER) return FALSE;
    AcquireSRWLockExclusive(&session.lock);
    ok=session.phase==DONE && !session.fatal && n>=session.answer_size &&
       !memcmp(frame,session.answer,session.answer_size);
    /* External errors carry no data. Validate the official inner reply before removing its tail. */
    if (ok && q.Kind==SpdIoctlTransactReadKind && frame[16]) {
        DWORD extra=q.Op.Read.BlockCount*512u;
        ok=session.answer_size==HEADER && n==HEADER+extra;
        for (DWORD i=HEADER;ok && i<n;++i) if (frame[i]) ok=FALSE;
        if (ok) fprintf(stderr,"SDK failed-read internal_bytes=%lu outer_bytes=0 zero_checked=1\n",extra);
    } else if (ok) { ok=n==session.answer_size; }
    *response_size=session.answer_size;
    session.phase=IDLE;
    ReleaseSRWLockExclusive(&session.lock);
    return ok;
}

DWORD sdk_stop(BOOL orderly) {
    BOOL ok=TRUE;
    if (session.started) {
        AcquireSRWLockExclusive(&session.lock);
        ok=session.phase==IDLE && !session.fatal;
        session.phase=orderly && ok?STOPPING:ABORTING;
        ReleaseSRWLockExclusive(&session.lock);
        session.shutdown(session.unit);
        session.wait(session.unit); /* keep inner handle, Rust service and module alive through callbacks */
        ok=ok && !session.fatal && session.closes==(orderly && session.cached ? 1u:0u);
        fprintf(stderr,"SDK stopped callbacks=%lu rejected=%lu close_calls=%lu close_error=%lu waited=1\n",
                session.callbacks,session.rejected,session.closes,session.close_error);
    }
    if (session.unit) session.destroy(session.unit);
    if (session.inner && session.inner!=INVALID_HANDLE_VALUE) CloseHandle(session.inner);
    if (session.dll) FreeLibrary(session.dll);
    if (!ok) return ERROR_GEN_FAILURE;
    if (session.close_error) return ERROR_WRITE_FAULT;
    return session.rejected ? ERROR_INVALID_DATA : ERROR_SUCCESS;
}
