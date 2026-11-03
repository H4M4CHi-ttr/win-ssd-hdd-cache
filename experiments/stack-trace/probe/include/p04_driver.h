#ifndef WIN_CACHE_P04_DRIVER_H
#define WIN_CACHE_P04_DRIVER_H
#include <ntddk.h>
#include <wdmsec.h>
#include <devpkey.h>
#include "p04_recorder.h"
#include "p04_wire.h"

/* Internal C-only ownership layout. Not an exported/native wire ABI. */
#define P04_ROLE_FILTER 0x50463446u
#define P04_ROLE_CONTROL 0x50463443u
#define P04_NOT_STARTED 0u
#define P04_STARTED 1u
#define P04_STOP_PENDING 2u
#define P04_STOPPED 3u
#define P04_REMOVE_PENDING 4u
#define P04_SURPRISE_REMOVED 5u
#define P04_DELETED 6u
#define P04_UNBOUND 0u
#define P04_PREPARING 1u
#define P04_ATTACHED 2u
#define P04_RETIRED 3u
#define P04_SELECTOR_UNITS 512u

typedef struct P04FilterExtension P04FilterExtension;
typedef struct {
    P04FilterExtension *Extension;
    WinCacheP04Token Token;
} P04CallbackContext;
struct P04FilterExtension {
    ULONG Role;
    PDEVICE_OBJECT Lower;
    IO_REMOVE_LOCK RemoveLock;
    KSPIN_LOCK StateLock;
    ULONG PnpState, PreviousStop, PreviousRemove;
    ULONGLONG LastCallId;
    WinCacheP04Recorder Recorder;
    P04CallbackContext Callbacks[WIN_CACHE_P04_CONTEXTS];
};
typedef struct {
    ULONG Role;
    PFILE_OBJECT OpenFile;
    PFILE_OBJECT Owner;
} P04ControlExtension;
typedef struct {
    KSPIN_LOCK PublicationLock;
    ULONG Binding;
    PDEVICE_OBJECT Target;
    PDEVICE_OBJECT Control;
    BOOLEAN SelectorValid;
    USHORT SelectorBytes;
    WCHAR Selector[P04_SELECTOR_UNITS];
} P04GlobalState;
_Static_assert(sizeof(P04FilterExtension) <= 65536, "per-target resident allocation budget");

DRIVER_INITIALIZE DriverEntry;
DRIVER_ADD_DEVICE P04AddDevice;
DRIVER_UNLOAD P04Unload;
DRIVER_DISPATCH P04Dispatch;
DRIVER_DISPATCH P04DispatchPnp;
DRIVER_DISPATCH P04DispatchPower;
DRIVER_DISPATCH P04DispatchControl;
IO_COMPLETION_ROUTINE P04ObservedCompletion;
IO_COMPLETION_ROUTINE P04ReleaseCompletion;
IO_COMPLETION_ROUTINE P04SyncCompletion;
#endif
