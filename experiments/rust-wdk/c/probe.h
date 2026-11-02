#pragma once
#if defined(_KERNEL_MODE)
/* Do not mix host CRT headers into the pinned WDK kernel headers. */
#include <ntddk.h>
typedef ULONG WinCacheU32;
typedef ULONGLONG WinCacheU64;
#define WIN_CACHE_OFFSET_OF(type, field) FIELD_OFFSET(type, field)
#else
#include <stdint.h>
#include <stddef.h>
typedef uint32_t WinCacheU32;
typedef uint64_t WinCacheU64;
#define WIN_CACHE_OFFSET_OF(type, field) offsetof(type, field)
#endif

_Static_assert(sizeof(WinCacheU32) == 4, "u32 width");
_Static_assert(sizeof(WinCacheU64) == 8, "u64 width");

typedef struct WinCacheRangeReply {
    WinCacheU32 status;
    WinCacheU32 reserved;
    WinCacheU64 start;
    WinCacheU64 end;
} WinCacheRangeReply;

_Static_assert(sizeof(WinCacheRangeReply) == 24, "size");
_Static_assert(_Alignof(WinCacheRangeReply) == 8, "alignment");
_Static_assert(WIN_CACHE_OFFSET_OF(WinCacheRangeReply, status) == 0, "status");
_Static_assert(WIN_CACHE_OFFSET_OF(WinCacheRangeReply, reserved) == 4, "reserved");
_Static_assert(WIN_CACHE_OFFSET_OF(WinCacheRangeReply, start) == 8, "start");
_Static_assert(WIN_CACHE_OFFSET_OF(WinCacheRangeReply, end) == 16, "end");

WinCacheRangeReply win_cache_probe_range(WinCacheU64 offset, WinCacheU64 length, WinCacheU64 capacity);

