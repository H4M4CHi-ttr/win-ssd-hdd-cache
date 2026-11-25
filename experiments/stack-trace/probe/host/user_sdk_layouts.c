/* Compile only on AMD64 with real Microsoft user-mode SDK headers.
 * These are the layout/constant assumptions used by the narrow Rust FFI capsules.
 * This translation unit has no entry point and calls no Windows API. */
#define WIN32_LEAN_AND_MEAN
#define _WIN32_WINNT 0x0A00
#include <windows.h>
#include <setupapi.h>
#include <processsnapshot.h>
#include <winternl.h>
#include <stddef.h>

_Static_assert(sizeof(void*) == 8, "AMD64 only");
_Static_assert(sizeof(SP_DEVINFO_DATA) == 32 && offsetof(SP_DEVINFO_DATA, DevInst) == 20 &&
               offsetof(SP_DEVINFO_DATA, Reserved) == 24, "device instance data");
_Static_assert(sizeof(DEVPROPKEY) == 20 && SPDRP_UPPERFILTERS == 17 &&
               SPDRP_BUSTYPEGUID == 19 && SPDRP_UPPERFILTERS != SPDRP_BUSTYPEGUID,
               "legacy instance filter property and bus type are distinct");
_Static_assert(REG_SZ == 1 && REG_MULTI_SZ == 7 && INF_STYLE_WIN4 == 2, "registry and INF types");
_Static_assert(SP_COPY_NOOVERWRITE == 8 && SPOST_PATH == 1, "staging flags");
_Static_assert(sizeof(QUERY_SERVICE_CONFIGW) == 64 &&
               offsetof(QUERY_SERVICE_CONFIGW, lpBinaryPathName) == 16, "service metadata");
_Static_assert(sizeof(SERVICE_STATUS_PROCESS) == 36 &&
               offsetof(SERVICE_STATUS_PROCESS, dwCurrentState) == 4, "service status");
_Static_assert(sizeof(SECURITY_ATTRIBUTES) == 24 && sizeof(PROCESS_INFORMATION) == 24, "child process");
_Static_assert(sizeof(STARTUPINFOW) == 104 && offsetof(STARTUPINFOW, hStdInput) == 80 &&
               offsetof(STARTUPINFOEXW, lpAttributeList) == 104 && sizeof(STARTUPINFOEXW) == 112,
               "explicit handle inheritance");
_Static_assert(sizeof(FILE_ID_INFO) == 24 && PROC_THREAD_ATTRIBUTE_HANDLE_LIST == 0x20002,
               "child image and allowlist");
_Static_assert(sizeof(PSS_HANDLE_ENTRY) == 136 && offsetof(PSS_HANDLE_ENTRY, Handle) == 0 &&
               offsetof(PSS_HANDLE_ENTRY, GrantedAccess) == 28, "self handle snapshot entry");
_Static_assert(PSS_CAPTURE_HANDLES == 4 && PSS_WALK_HANDLES == 2, "self handle snapshot flags");
_Static_assert(sizeof(UNICODE_STRING) == 16 && offsetof(UNICODE_STRING, Buffer) == 8 &&
               sizeof(PUBLIC_OBJECT_BASIC_INFORMATION) == 56 &&
               offsetof(PUBLIC_OBJECT_BASIC_INFORMATION, GrantedAccess) == 4, "object queries");
_Static_assert(SYNCHRONIZE == 0x100000 && EVENT_MODIFY_STATE == 2, "event rights");
