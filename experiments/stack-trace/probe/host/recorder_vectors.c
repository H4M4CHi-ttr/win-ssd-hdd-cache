#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "p04_recorder.h"
#define CHECK(v) do { if (!(v)) { fprintf(stderr,"P04_CHECK_FAILED line=%d\n",__LINE__); exit(1); } } while (0)
static WinCacheP04Recorder r;
static const WinCacheP02Submit read_request = {1,3,0,0,0,1,0,0,4096,512};
static const WinCacheP02Completion read_done = {1,0,1,0,512};
static void start(void) { win_cache_p04_init(&r); CHECK(win_cache_p04_begin(&r,7)); }
static WinCacheP04Stats stats(void) { WinCacheP04Stats s; win_cache_p04_stats(&r,&s); return s; }
static void pair(void) { WinCacheP04Token t; CHECK(win_cache_p04_submit(&r,&read_request,&t)); win_cache_p04_complete(&r,t,&read_done); }
static void contexts(void) {
    WinCacheP04Token tokens[64], unavailable = {99,99}; start();
    for (unsigned int n=0;n<64;++n) { CHECK(win_cache_p04_submit(&r,&read_request,&tokens[n])); CHECK(tokens[n].slot==n && tokens[n].request_id==n+1); }
    CHECK(!win_cache_p04_submit(&r,&read_request,&unavailable));
    CHECK(unavailable.slot==99 && unavailable.request_id==99);
    WinCacheP04Stats s=stats(); CHECK(s.attempted==65 && s.dropped==1 && s.active_contexts==64 && s.queued_records==64 && s.popped==0 && s.fault_mask==1);
    for (unsigned int n=0;n<64;++n) win_cache_p04_complete(&r,tokens[n],&read_done);
    s=stats(); CHECK(s.active_contexts==0 && s.attempted==129 && s.dropped==1 && s.queued_records==128);
}
static void ring_drop(void) {
    WinCacheP04Record record; start(); for (unsigned int n=0;n<129;++n) pair();
    WinCacheP04Stats s=stats(); CHECK(s.attempted==258 && s.dropped==2 && s.active_contexts==0 && s.queued_records==256 && s.fault_mask==0);
    for (unsigned int n=0;n<256;++n) { CHECK(win_cache_p04_pop(&r,&record)); CHECK(record.seq==n && record.request_id==n/2+1 && record.kind==n%2+1); }
    CHECK(!win_cache_p04_pop(&r,&record)); s=stats(); CHECK(s.popped==256 && s.queued_records==0);
}
static void late(void) {
    WinCacheP04Token t, no; start(); CHECK(win_cache_p04_submit(&r,&read_request,&t));
    win_cache_p04_stop(&r); CHECK(!win_cache_p04_seal(&r)); CHECK(!win_cache_p04_submit(&r,&read_request,&no));
    CHECK(stats().active_contexts==1); win_cache_p04_complete(&r,t,&read_done); CHECK(win_cache_p04_seal(&r));
    WinCacheP04Stats s=stats(); CHECK(s.observer_state==3 && s.attempted==2 && s.dropped==0 && s.active_contexts==0 && s.queued_records==2);
    win_cache_p04_complete(&r,t,&read_done); CHECK(stats().fault_mask==0); CHECK(!win_cache_p04_begin(&r,8));
}
static void duplicate(void) {
    WinCacheP04Token a,b; start(); CHECK(win_cache_p04_submit(&r,&read_request,&a)); CHECK(win_cache_p04_submit(&r,&read_request,&b));
    win_cache_p04_complete(&r,a,&read_done); win_cache_p04_complete(&r,a,&read_done);
    WinCacheP04Stats s=stats(); CHECK(s.active_contexts==1 && s.attempted==3 && s.dropped==0 && s.fault_mask==3);
    win_cache_p04_complete(&r,b,&read_done); CHECK(stats().active_contexts==0);
}
static void invalid_snapshot(void) {
    WinCacheP04Token t; WinCacheP02Submit bad=read_request; start(); bad.version=2;
    CHECK(!win_cache_p04_submit(&r,&bad,&t)); CHECK(stats().attempted==0 && stats().fault_mask==3);
    start(); CHECK(win_cache_p04_submit(&r,&read_request,&t)); win_cache_p04_complete(&r,t,NULL);
    WinCacheP04Stats s=stats(); CHECK(s.attempted==2 && s.dropped==1 && s.active_contexts==0 && s.queued_records==1 && s.fault_mask==2);
    start(); WinCacheP02Submit flush={1,9,0,0,0,0,0,0,0,0}; CHECK(win_cache_p04_submit(&r,&flush,&t));
    win_cache_p04_complete(&r,t,&read_done); s=stats(); CHECK(s.active_contexts==0 && s.dropped==1 && s.fault_mask==2);
}
static void pop_not_retire(void) {
    WinCacheP04Token t; WinCacheP04Record record; start(); CHECK(win_cache_p04_submit(&r,&read_request,&t));
    CHECK(win_cache_p04_pop(&r,&record)); CHECK(stats().active_contexts==1 && stats().queued_records==0 && stats().popped==1);
    win_cache_p04_stop(&r); CHECK(!win_cache_p04_seal(&r)); win_cache_p04_complete(&r,t,&read_done); CHECK(win_cache_p04_seal(&r));
}
static void counter_overflow(void) {
    WinCacheP04Token t; start(); r.state.attempted=WIN_CACHE_P02_MAX_U64;
    CHECK(win_cache_p04_submit(&r,&read_request,&t)); WinCacheP04Stats s=stats();
    CHECK(s.known_mask==6 && s.attempted==0 && s.dropped==0 && s.active_contexts==1 && s.queued_records==0 && s.fault_mask==4);
    win_cache_p04_complete(&r,t,NULL); s=stats(); CHECK(s.active_contexts==0 && s.known_mask==6 && s.fault_mask==6);
    start(); for (unsigned int n=0;n<128;++n) pair(); r.state.dropped=WIN_CACHE_P02_MAX_U64;
    CHECK(win_cache_p04_submit(&r,&read_request,&t)); s=stats(); CHECK(s.known_mask==6 && s.attempted==0 && s.dropped==0 && s.queued_records==256);
    win_cache_p04_complete(&r,t,&read_done); CHECK(stats().active_contexts==0 && stats().known_mask==6);
}
static void last_ordinal(void) {
    WinCacheP04Token t; WinCacheP04Record record; start(); r.state.attempted=WIN_CACHE_P02_MAX_U64-1;
    CHECK(win_cache_p04_submit(&r,&read_request,&t)); CHECK(stats().attempted==WIN_CACHE_P02_MAX_U64 && stats().known_mask==7);
    win_cache_p04_complete(&r,t,&read_done); CHECK(stats().known_mask==6 && stats().active_contexts==0);
    CHECK(win_cache_p04_pop(&r,&record)); CHECK(record.seq==WIN_CACHE_P02_MAX_U64-1 && record.kind==1); CHECK(!win_cache_p04_pop(&r,&record));
}
static void last_identity(void) {
    WinCacheP04Token t,no; start(); r.next_request_id=WIN_CACHE_P02_MAX_U64;
    CHECK(win_cache_p04_submit(&r,&read_request,&t) && t.request_id==WIN_CACHE_P02_MAX_U64);
    CHECK(stats().fault_mask==8); CHECK(!win_cache_p04_submit(&r,&read_request,&no));
    WinCacheP04Stats s=stats(); CHECK(s.attempted==2 && s.dropped==1 && s.active_contexts==1 && s.fault_mask==9);
    win_cache_p04_complete(&r,t,&read_done); s=stats(); CHECK(s.attempted==3 && s.active_contexts==0 && s.queued_records==2);
}
static void independent_overflow(void) {
    WinCacheP04Record record; start(); pair(); r.state.popped=WIN_CACHE_P02_MAX_U64;
    CHECK(win_cache_p04_pop(&r,&record)); WinCacheP04Stats s=stats(); CHECK(s.known_mask==5 && s.popped==0 && s.fault_mask==128);
    CHECK(win_cache_p04_pop(&r,&record)); CHECK(stats().known_mask==5 && stats().popped==0);
    start(); r.state.skipped=WIN_CACHE_P02_MAX_U64; win_cache_p04_skip(&r,0); s=stats();
    CHECK(s.known_mask==3 && s.skipped==0 && s.fault_mask==(16|64)); win_cache_p04_skip(&r,1); CHECK(stats().skipped==0 && stats().known_mask==3 && stats().fault_mask==(16|64|256));
}
static void slot_reuse(void) {
    WinCacheP04Token a,b; WinCacheP02Submit copy; start(); CHECK(win_cache_p04_submit(&r,&read_request,&a));
    CHECK(win_cache_p04_saved_submit(&r,a,&copy) && memcmp(&copy,&read_request,sizeof copy)==0);
    win_cache_p04_complete(&r,a,&read_done); CHECK(win_cache_p04_submit(&r,&read_request,&b)); CHECK(a.slot==b.slot && a.request_id!=b.request_id);
    CHECK(!win_cache_p04_saved_submit(&r,a,&copy)); win_cache_p04_complete(&r,a,&read_done); CHECK(stats().active_contexts==1);
    a.slot=64; win_cache_p04_complete(&r,a,&read_done); CHECK(stats().active_contexts==1); win_cache_p04_complete(&r,b,&read_done); CHECK(stats().active_contexts==0);
}
static void prebegin_cleanup(void) {
    win_cache_p04_init(&r); CHECK(!win_cache_p04_begin(&r,0)); CHECK(stats().observer_state==0 && stats().session_id==0);
    win_cache_p04_consumer_lost(&r); win_cache_p04_stop(&r); CHECK(!win_cache_p04_seal(&r));
    CHECK(stats().observer_state==0 && stats().fault_mask==0 && stats().known_mask==7); CHECK(win_cache_p04_begin(&r,7)); CHECK(!win_cache_p04_begin(&r,8));
    win_cache_p04_init(&r); win_cache_p04_skip(&r,0); CHECK(stats().fault_mask==0 && stats().skipped==0);
    win_cache_p04_skip(&r,2); CHECK(stats().fault_mask==256 && stats().skipped==0); CHECK(!win_cache_p04_begin(&r,7));
}
static void begun_cleanup(void) {
    WinCacheP04Token t; WinCacheP04Record record; start(); CHECK(win_cache_p04_submit(&r,&read_request,&t)); win_cache_p04_consumer_lost(&r);
    CHECK(stats().observer_state==2 && stats().fault_mask==32 && stats().active_contexts==1);
    win_cache_p04_complete(&r,t,&read_done); CHECK(win_cache_p04_seal(&r));
    CHECK(win_cache_p04_pop(&r,&record) && win_cache_p04_pop(&r,&record));
    start(); win_cache_p04_stop(&r); CHECK(win_cache_p04_seal(&r)); win_cache_p04_consumer_lost(&r); CHECK(stats().fault_mask==0);
    start(); pair(); win_cache_p04_stop(&r); CHECK(win_cache_p04_seal(&r)); win_cache_p04_consumer_lost(&r); CHECK(stats().fault_mask==32);
}
static void skipped_window(void) {
    start(); win_cache_p04_skip(&r,0); win_cache_p04_skip(&r,2); CHECK(stats().skipped==2 && stats().fault_mask==(16|256));
    win_cache_p04_stop(&r); win_cache_p04_skip(&r,1); CHECK(stats().skipped==2); CHECK(win_cache_p04_seal(&r)); win_cache_p04_skip(&r,1); CHECK(stats().skipped==2);
}
static void fifo_wrap(void) {
    WinCacheP04Record record; start(); for (unsigned int n=0;n<128;++n) pair();
    for (unsigned int n=0;n<200;++n) { CHECK(win_cache_p04_pop(&r,&record)); CHECK(record.seq==n); }
    for (unsigned int n=0;n<100;++n) pair();
    for (unsigned int n=200;n<456;++n) { CHECK(win_cache_p04_pop(&r,&record)); CHECK(record.seq==n); }
    WinCacheP04Stats s=stats(); CHECK(s.attempted==456 && s.popped==456 && s.dropped==0 && s.queued_records==0 && s.active_contexts==0);
}
#define RUN(fn,name) do { fn(); puts("P04_CASE_PASS " name); ++cases; } while(0)
int main(void) {
    unsigned int cases=0;
    _Static_assert(sizeof r <= 40960, "bounded recorder size");
    RUN(contexts,"producer_65th_submit"); RUN(ring_drop,"producer_ring_drop_pair"); RUN(late,"producer_stop_late_completion");
    RUN(duplicate,"producer_duplicate_token"); RUN(invalid_snapshot,"producer_invalid_snapshot_retirement"); RUN(pop_not_retire,"producer_pop_not_context_retirement");
    RUN(counter_overflow,"producer_counter_overflow"); RUN(last_ordinal,"producer_last_ordinal_once"); RUN(last_identity,"producer_last_id_once");
    RUN(independent_overflow,"producer_popped_and_skipped_overflow"); RUN(slot_reuse,"producer_old_token_cannot_retire_reused_slot");
    RUN(prebegin_cleanup,"producer_unarmed_cleanup_and_usage_veto"); RUN(begun_cleanup,"producer_consumer_loss_window"); RUN(skipped_window,"producer_skipped_window"); RUN(fifo_wrap,"producer_fifo_wrap");
    printf("P04_SUITE_PASS suite=recorder cases=%u\n",cases); return 0;
}
