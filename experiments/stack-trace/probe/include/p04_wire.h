#ifndef WIN_CACHE_P04_WIRE_H
#define WIN_CACHE_P04_WIRE_H
#include "p04_recorder.h"
#define WIN_CACHE_P04_REQUEST_BYTES 48u
#define WIN_CACHE_P04_HEADER_BYTES 144u
#define WIN_CACHE_P04_RECORD_BYTES 96u
#define WIN_CACHE_P04_DRAIN_BYTES 1680u
#define WIN_CACHE_P04_BEGIN 1u
#define WIN_CACHE_P04_STATUS 2u
#define WIN_CACHE_P04_DRAIN 3u
#define WIN_CACHE_P04_STOP 4u
#define WIN_CACHE_P04_IOCTL_BEGIN 0x8337e000u
#define WIN_CACHE_P04_IOCTL_STATUS 0x8337e004u
#define WIN_CACHE_P04_IOCTL_DRAIN 0x8337e008u
#define WIN_CACHE_P04_IOCTL_STOP 0x8337e00cu
typedef WinCacheP02U32 WinCacheP04WireResult;
#define WIN_CACHE_P04_WIRE_OK 0u
#define WIN_CACHE_P04_WIRE_BAD_IOCTL 1u
#define WIN_CACHE_P04_WIRE_BAD_INPUT 2u
#define WIN_CACHE_P04_WIRE_BAD_OUTPUT 3u
typedef struct {
    WinCacheP02U32 command;
    WinCacheP02U64 session_id, target_id, call_id;
    WinCacheP02U32 max_records;
} WinCacheP04Request;
WinCacheP04WireResult win_cache_p04_decode_request(WinCacheP02U32 ioctl,
    const WinCacheP02U8 *bytes, WinCacheP02U32 input_bytes,
    WinCacheP02U32 output_capacity, WinCacheP04Request *out);
/* Infallible internal encoders: caller proves validated values, nonaliasing
 * inputs and writable output regions. Serialize scalars, never native padding. */
void win_cache_p04_encode_header(const WinCacheP04Request *request,
    const WinCacheP04Stats *stats, WinCacheP02U32 pnp_state,
    WinCacheP02U32 record_count, WinCacheP02U8 output[144]);
void win_cache_p04_encode_record(const WinCacheP04Record *record,
    WinCacheP02U8 output[96]);
#endif
