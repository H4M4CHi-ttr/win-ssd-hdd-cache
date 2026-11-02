#pragma once
#include <stdint.h>
#include <stddef.h>

typedef struct WinCacheRangeReply {
    uint32_t status;
    uint32_t reserved;
    uint64_t start;
    uint64_t end;
} WinCacheRangeReply;

_Static_assert(sizeof(WinCacheRangeReply) == 24, "size");
_Static_assert(_Alignof(WinCacheRangeReply) == 8, "alignment");
_Static_assert(offsetof(WinCacheRangeReply, status) == 0, "status");
_Static_assert(offsetof(WinCacheRangeReply, reserved) == 4, "reserved");
_Static_assert(offsetof(WinCacheRangeReply, start) == 8, "start");
_Static_assert(offsetof(WinCacheRangeReply, end) == 16, "end");

WinCacheRangeReply win_cache_probe_range(uint64_t offset, uint64_t length, uint64_t capacity);

