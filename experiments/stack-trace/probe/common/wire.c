#include "p04_wire.h"

static WinCacheP02U32 get32(const WinCacheP02U8 *p) {
    return (WinCacheP02U32)p[0] | ((WinCacheP02U32)p[1] << 8)
        | ((WinCacheP02U32)p[2] << 16) | ((WinCacheP02U32)p[3] << 24);
}
static WinCacheP02U64 get64(const WinCacheP02U8 *p) {
    return (WinCacheP02U64)get32(p) | ((WinCacheP02U64)get32(p+4) << 32);
}
static void put32(WinCacheP02U8 *p, WinCacheP02U32 n) {
    for (WinCacheP02U32 i=0;i<4;++i) p[i]=(WinCacheP02U8)(n >> (i*8));
}
static void put64(WinCacheP02U8 *p, WinCacheP02U64 n) {
    for (WinCacheP02U32 i=0;i<8;++i) p[i]=(WinCacheP02U8)(n >> (i*8));
}
WinCacheP04WireResult win_cache_p04_decode_request(WinCacheP02U32 ioctl,
    const WinCacheP02U8 *bytes, WinCacheP02U32 input_bytes,
    WinCacheP02U32 output_capacity, WinCacheP04Request *out) {
    const WinCacheP02U8 magic[8]={'W','C','H','P','0','4','R','Q'};
    WinCacheP04Request request;
    switch(ioctl) {
        case WIN_CACHE_P04_IOCTL_BEGIN: request.command=WIN_CACHE_P04_BEGIN; break;
        case WIN_CACHE_P04_IOCTL_STATUS: request.command=WIN_CACHE_P04_STATUS; break;
        case WIN_CACHE_P04_IOCTL_DRAIN: request.command=WIN_CACHE_P04_DRAIN; break;
        case WIN_CACHE_P04_IOCTL_STOP: request.command=WIN_CACHE_P04_STOP; break;
        default: return WIN_CACHE_P04_WIRE_BAD_IOCTL;
    }
    if (bytes==NULL || out==NULL || input_bytes!=48) return WIN_CACHE_P04_WIRE_BAD_INPUT;
    if (output_capacity != (request.command==WIN_CACHE_P04_DRAIN ? 1680u : 144u)) return WIN_CACHE_P04_WIRE_BAD_OUTPUT;
    for (WinCacheP02U32 i=0;i<8;++i) if(bytes[i]!=magic[i]) return WIN_CACHE_P04_WIRE_BAD_INPUT;
    if (get32(bytes+8)!=1 || get32(bytes+12)!=48 || get32(bytes+44)!=0) return WIN_CACHE_P04_WIRE_BAD_INPUT;
    request.session_id=get64(bytes+16); request.target_id=get64(bytes+24); request.call_id=get64(bytes+32);
    request.max_records=get32(bytes+40);
    if (request.session_id==0 || request.target_id!=1 || request.call_id==0
        || (request.command==WIN_CACHE_P04_BEGIN && request.call_id!=1)) return WIN_CACHE_P04_WIRE_BAD_INPUT;
    if (request.command==WIN_CACHE_P04_DRAIN) {
        if(request.max_records==0 || request.max_records>16) return WIN_CACHE_P04_WIRE_BAD_INPUT;
    } else if(request.max_records!=0) return WIN_CACHE_P04_WIRE_BAD_INPUT;
    /* Do not mutate output before every input and capacity check succeeded. */
    *out=request;
    return WIN_CACHE_P04_WIRE_OK;
}
void win_cache_p04_encode_header(const WinCacheP04Request *request,
    const WinCacheP04Stats *stats, WinCacheP02U32 pnp_state,
    WinCacheP02U32 record_count, WinCacheP02U8 output[144]) {
    const WinCacheP02U8 magic[8]={'W','C','H','P','0','4','R','S'};
    for(WinCacheP02U32 i=0;i<144;++i) output[i]=0;
    for(WinCacheP02U32 i=0;i<8;++i) output[i]=magic[i];
    put32(output+8,1); put32(output+12,144); put32(output+16,144+96*record_count);
    put32(output+20,96); put32(output+24,record_count); put32(output+28,request->command);
    put64(output+32,stats->session_id); put64(output+40,1); put64(output+48,request->call_id);
    put32(output+56,stats->observer_state); put32(output+60,pnp_state); put32(output+64,stats->active_contexts);
    put32(output+68,stats->queued_records); put32(output+72,stats->known_mask); put32(output+76,stats->fault_mask);
    if(stats->known_mask & WIN_CACHE_P04_KNOWN_COUPLED) { put64(output+80,stats->attempted); put64(output+88,stats->dropped); }
    if(stats->known_mask & WIN_CACHE_P04_KNOWN_POPPED) put64(output+96,stats->popped);
    if(stats->known_mask & WIN_CACHE_P04_KNOWN_SKIPPED) put64(output+104,stats->skipped);
    put32(output+112,request->max_records);
}
void win_cache_p04_encode_record(const WinCacheP04Record *record, WinCacheP02U8 output[96]) {
    for(WinCacheP02U32 i=0;i<96;++i) output[i]=0;
    put64(output,record->seq); put64(output+8,record->request_id); put32(output+16,record->kind);
    if(record->kind==1) {
        put32(output+24,record->submit.major); put32(output+28,record->submit.minor); put32(output+32,record->submit.flags);
        put32(output+36,record->submit.control_known); put32(output+40,record->submit.range_known);
        if(record->submit.control_known) put32(output+44,record->submit.control_code);
        if(record->submit.range_known) { put64(output+48,record->submit.offset); put64(output+56,record->submit.length); }
    } else {
        put32(output+64,record->complete.status); put32(output+68,record->complete.information_known);
        if(record->complete.information_known) put64(output+72,record->complete.information);
    }
}
