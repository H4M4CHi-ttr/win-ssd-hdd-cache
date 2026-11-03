#ifndef WIN_CACHE_P04_RECORDER_H
#define WIN_CACHE_P04_RECORDER_H

#include "p02_snapshot.h"

/* Fixed, externally serialized metadata. No IRP/buffer ownership or allocation.
 * The driver uses operations/stats only. Private fields are visible solely for
 * in-place extension allocation; host limits tests may seed them directly. */
#define WIN_CACHE_P04_CONTEXTS 64u
#define WIN_CACHE_P04_RECORDS 256u
#define WIN_CACHE_P04_UNARMED 0u
#define WIN_CACHE_P04_RUNNING 1u
#define WIN_CACHE_P04_STOPPING 2u
#define WIN_CACHE_P04_SEALED 3u
#define WIN_CACHE_P04_TRACKING_LOST 0x001u
#define WIN_CACHE_P04_PROTOCOL_FAULT 0x002u
#define WIN_CACHE_P04_COUNTER_EXHAUSTED 0x004u
#define WIN_CACHE_P04_IDENTITY_EXHAUSTED 0x008u
#define WIN_CACHE_P04_UNSUPPORTED_OBSERVATION 0x010u
#define WIN_CACHE_P04_CONSUMER_LOST 0x020u
#define WIN_CACHE_P04_SKIPPED_EXHAUSTED 0x040u
#define WIN_CACHE_P04_POPPED_EXHAUSTED 0x080u
#define WIN_CACHE_P04_USAGE_VETO 0x100u
#define WIN_CACHE_P04_KNOWN_COUPLED 1u
#define WIN_CACHE_P04_KNOWN_POPPED 2u
#define WIN_CACHE_P04_KNOWN_SKIPPED 4u

typedef struct { WinCacheP02U32 slot; WinCacheP02U64 request_id; } WinCacheP04Token;
typedef struct {
    WinCacheP02U64 seq, request_id;
    WinCacheP02U32 kind;
    WinCacheP02Submit submit;
    WinCacheP02Completion complete;
} WinCacheP04Record;
typedef struct {
    WinCacheP02U64 session_id;
    WinCacheP02U32 observer_state, active_contexts, queued_records, known_mask, fault_mask;
    WinCacheP02U64 attempted, dropped, popped, skipped;
} WinCacheP04Stats;
typedef struct {
    WinCacheP02U64 request_id;
    WinCacheP02Submit submit;
} WinCacheP04Context;
typedef struct {
    WinCacheP04Stats state;
    WinCacheP02U64 next_request_id;
    WinCacheP02U32 head;
    WinCacheP04Context contexts[WIN_CACHE_P04_CONTEXTS];
    WinCacheP04Record records[WIN_CACHE_P04_RECORDS];
} WinCacheP04Recorder;

_Static_assert(sizeof(WinCacheP04Recorder) <= 40960, "resident recorder budget");
void win_cache_p04_init(WinCacheP04Recorder *value);
int win_cache_p04_begin(WinCacheP04Recorder *value, WinCacheP02U64 session_id);
int win_cache_p04_submit(WinCacheP04Recorder *value, const WinCacheP02Submit *submit,
                         WinCacheP04Token *token);
int win_cache_p04_saved_submit(const WinCacheP04Recorder *value, WinCacheP04Token token,
                               WinCacheP02Submit *submit);
void win_cache_p04_complete(WinCacheP04Recorder *value, WinCacheP04Token token,
                            const WinCacheP02Completion *complete);
void win_cache_p04_stop(WinCacheP04Recorder *value);
int win_cache_p04_seal(WinCacheP04Recorder *value);
void win_cache_p04_skip(WinCacheP04Recorder *value, WinCacheP02U32 usage_veto);
void win_cache_p04_consumer_lost(WinCacheP04Recorder *value);
void win_cache_p04_stats(const WinCacheP04Recorder *value, WinCacheP04Stats *stats);
int win_cache_p04_pop(WinCacheP04Recorder *value, WinCacheP04Record *record);
#endif
