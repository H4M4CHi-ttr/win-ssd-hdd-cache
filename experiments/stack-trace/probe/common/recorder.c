#include "p04_recorder.h"

static void exhaust(WinCacheP04Recorder *r) {
    r->state.known_mask &= ~WIN_CACHE_P04_KNOWN_COUPLED;
    r->state.attempted=0; r->state.dropped=0;
    r->state.fault_mask |= WIN_CACHE_P04_COUNTER_EXHAUSTED;
}
static int attempt(WinCacheP04Recorder *r, WinCacheP02U64 *seq) {
    if(!(r->state.known_mask & WIN_CACHE_P04_KNOWN_COUPLED)) return 0;
    if(r->state.attempted==WIN_CACHE_P02_MAX_U64) { exhaust(r); return 0; }
    *seq=r->state.attempted++;
    return 1;
}
static void drop_record(WinCacheP04Recorder *r) {
    if(!(r->state.known_mask & WIN_CACHE_P04_KNOWN_COUPLED)) return;
    if(r->state.dropped==WIN_CACHE_P02_MAX_U64) exhaust(r);
    else ++r->state.dropped;
}
static void retain(WinCacheP04Recorder *r, int known_seq, const WinCacheP04Record *record) {
    if(!known_seq) return;
    if(r->state.queued_records==WIN_CACHE_P04_RECORDS) { drop_record(r); return; }
    r->records[(r->head+r->state.queued_records)%WIN_CACHE_P04_RECORDS]=*record;
    ++r->state.queued_records;
}
void win_cache_p04_init(WinCacheP04Recorder *r) {
    /* Initialize caller-owned storage in place. Never return/copy the whole
     * recorder by value or place a 40 KiB temporary on a kernel stack. */
    WinCacheP02U8 *bytes=(WinCacheP02U8 *)r;
    for(WinCacheP02U32 i=0;i<(WinCacheP02U32)sizeof *r;++i) bytes[i]=0;
    r->state.known_mask=7; r->next_request_id=1;
}
int win_cache_p04_begin(WinCacheP04Recorder *r, WinCacheP02U64 session_id) {
    if(r->state.observer_state!=WIN_CACHE_P04_UNARMED || session_id==0
        || (r->state.fault_mask & WIN_CACHE_P04_USAGE_VETO)) return 0;
    r->state.session_id=session_id; r->state.observer_state=WIN_CACHE_P04_RUNNING;
    return 1;
}
int win_cache_p04_submit(WinCacheP04Recorder *r, const WinCacheP02Submit *submit, WinCacheP04Token *token) {
    WinCacheP04Record record={0};
    WinCacheP02U32 slot; int known_seq;
    if(r->state.observer_state!=WIN_CACHE_P04_RUNNING) return 0;
    if(token==NULL || !win_cache_p02_submit_valid(submit)) {
        r->state.fault_mask |= WIN_CACHE_P04_PROTOCOL_FAULT | WIN_CACHE_P04_TRACKING_LOST;
        return 0;
    }
    known_seq=attempt(r,&record.seq);
    for(slot=0;slot<WIN_CACHE_P04_CONTEXTS;++slot) if(r->contexts[slot].request_id==0) break;
    if(slot==WIN_CACHE_P04_CONTEXTS || r->next_request_id==0) {
        drop_record(r); r->state.fault_mask |= WIN_CACHE_P04_TRACKING_LOST; return 0;
    }
    record.request_id=r->next_request_id;
    if(r->next_request_id==WIN_CACHE_P02_MAX_U64) {
        r->next_request_id=0; r->state.fault_mask |= WIN_CACHE_P04_IDENTITY_EXHAUSTED;
    } else ++r->next_request_id;
    r->contexts[slot].request_id=record.request_id; r->contexts[slot].submit=*submit;
    ++r->state.active_contexts;
    record.kind=1; record.submit=*submit; retain(r,known_seq,&record);
    token->slot=slot; token->request_id=record.request_id; return 1;
}
int win_cache_p04_saved_submit(const WinCacheP04Recorder *r, WinCacheP04Token token, WinCacheP02Submit *submit) {
    if(submit==NULL || token.slot>=WIN_CACHE_P04_CONTEXTS || token.request_id==0
        || r->contexts[token.slot].request_id!=token.request_id) return 0;
    *submit=r->contexts[token.slot].submit; return 1;
}
void win_cache_p04_complete(WinCacheP04Recorder *r, WinCacheP04Token token, const WinCacheP02Completion *complete) {
    WinCacheP04Record record={0}; WinCacheP02Submit saved; int known_seq;
    if(r->state.observer_state==WIN_CACHE_P04_SEALED || r->state.observer_state==WIN_CACHE_P04_UNARMED) return;
    if(!win_cache_p04_saved_submit(r,token,&saved)) {
        r->state.fault_mask |= WIN_CACHE_P04_PROTOCOL_FAULT | WIN_CACHE_P04_TRACKING_LOST; return;
    }
    known_seq=attempt(r,&record.seq);
    /* Retire metadata only after the caller's genuine completion notification,
     * even when snapshot validation fails. This releases no Windows resource. */
    r->contexts[token.slot].request_id=0; --r->state.active_contexts;
    if(!win_cache_p02_completion_valid(&saved,complete)) {
        drop_record(r); r->state.fault_mask |= WIN_CACHE_P04_PROTOCOL_FAULT; return;
    }
    record.request_id=token.request_id; record.kind=2; record.complete=*complete;
    retain(r,known_seq,&record);
}
void win_cache_p04_stop(WinCacheP04Recorder *r) {
    if(r->state.observer_state==WIN_CACHE_P04_RUNNING) r->state.observer_state=WIN_CACHE_P04_STOPPING;
}
int win_cache_p04_seal(WinCacheP04Recorder *r) {
    if(r->state.observer_state==WIN_CACHE_P04_SEALED) return 1;
    if(r->state.observer_state!=WIN_CACHE_P04_STOPPING || r->state.active_contexts!=0) return 0;
    r->state.observer_state=WIN_CACHE_P04_SEALED; return 1;
}
void win_cache_p04_skip(WinCacheP04Recorder *r, WinCacheP02U32 usage_veto) {
    if(r->state.observer_state==WIN_CACHE_P04_UNARMED) {
        if(usage_veto) r->state.fault_mask |= WIN_CACHE_P04_USAGE_VETO;
        return;
    }
    if(r->state.observer_state!=WIN_CACHE_P04_RUNNING) return;
    r->state.fault_mask |= WIN_CACHE_P04_UNSUPPORTED_OBSERVATION;
    if(usage_veto) r->state.fault_mask |= WIN_CACHE_P04_USAGE_VETO;
    if(r->state.known_mask & WIN_CACHE_P04_KNOWN_SKIPPED) {
        if(r->state.skipped==WIN_CACHE_P02_MAX_U64) {
            r->state.known_mask &= ~WIN_CACHE_P04_KNOWN_SKIPPED; r->state.skipped=0;
            r->state.fault_mask |= WIN_CACHE_P04_SKIPPED_EXHAUSTED;
        } else ++r->state.skipped;
    }
}
void win_cache_p04_consumer_lost(WinCacheP04Recorder *r) {
    if(r->state.observer_state==WIN_CACHE_P04_UNARMED
        || (r->state.observer_state==WIN_CACHE_P04_SEALED && r->state.queued_records==0)) return;
    r->state.fault_mask |= WIN_CACHE_P04_CONSUMER_LOST; win_cache_p04_stop(r);
}
void win_cache_p04_stats(const WinCacheP04Recorder *r, WinCacheP04Stats *s) { *s=r->state; }
int win_cache_p04_pop(WinCacheP04Recorder *r, WinCacheP04Record *record) {
    if(record==NULL || r->state.queued_records==0) return 0;
    *record=r->records[r->head]; r->head=(r->head+1)%WIN_CACHE_P04_RECORDS; --r->state.queued_records;
    if(r->state.known_mask & WIN_CACHE_P04_KNOWN_POPPED) {
        if(r->state.popped==WIN_CACHE_P02_MAX_U64) {
            r->state.popped=0; r->state.known_mask &= ~WIN_CACHE_P04_KNOWN_POPPED;
            r->state.fault_mask |= WIN_CACHE_P04_POPPED_EXHAUSTED;
        } else ++r->state.popped;
    }
    return 1;
}
