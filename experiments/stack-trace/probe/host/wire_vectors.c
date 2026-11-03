#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "p04_wire.h"

#define CHECK(v) do { if (!(v)) { fprintf(stderr, "P04_CHECK_FAILED line=%d\n", __LINE__); exit(1); } } while (0)
static const unsigned char begin_bytes[48] = {
    0x57,0x43,0x48,0x50,0x30,0x34,0x52,0x51, 1,0,0,0, 48,0,0,0,
    7,0,0,0,0,0,0,0, 1,0,0,0,0,0,0,0, 1,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0
};
static void lengths(void) {
    WinCacheP04Request r;
    CHECK(WIN_CACHE_P04_REQUEST_BYTES == 48 && WIN_CACHE_P04_HEADER_BYTES == 144);
    CHECK(WIN_CACHE_P04_RECORD_BYTES == 96 && WIN_CACHE_P04_DRAIN_BYTES == 1680);
    CHECK(WIN_CACHE_P04_IOCTL_BEGIN == 0x8337e000u && WIN_CACHE_P04_IOCTL_STATUS == 0x8337e004u);
    CHECK(WIN_CACHE_P04_IOCTL_DRAIN == 0x8337e008u && WIN_CACHE_P04_IOCTL_STOP == 0x8337e00cu);
    CHECK(win_cache_p04_decode_request(0x8337e000u, begin_bytes, 48, 144, &r) == 0);
    CHECK(r.command == 1 && r.session_id == 7 && r.target_id == 1 && r.call_id == 1 && r.max_records == 0);
}
static void header_literal(void) {
    const unsigned char expected[144] = {
        [0]=0x57,[1]=0x43,[2]=0x48,[3]=0x50,[4]=0x30,[5]=0x34,[6]=0x52,[7]=0x53,
        [8]=1,[12]=144,[16]=0x50,[17]=1,[20]=96,[24]=2,[28]=3,
        [32]=7,[40]=1,[48]=2,[56]=1,[60]=1,[72]=7,[80]=2,[96]=2,[112]=16
    };
    WinCacheP04Request request = { 3, 7, 1, 2, 16 };
    WinCacheP04Stats stats = { 7, 1, 0, 0, 7, 0, 2, 0, 2, 0 };
    unsigned char guarded[146];
    memset(guarded, 0xa5, sizeof guarded);
    win_cache_p04_encode_header(&request, &stats, 1, 2, guarded + 1);
    CHECK(guarded[0] == 0xa5 && guarded[145] == 0xa5);
    CHECK(memcmp(guarded + 1, expected, 144) == 0);
}
static void record_literal(void) {
    const unsigned char expected_submit[96] = {
        [8]=1,[16]=1,[24]=3,[40]=1,[49]=0x10,[57]=2
    };
    const unsigned char expected_complete[96] = {
        [0]=1,[8]=1,[16]=2,[68]=1,[73]=2
    };
    WinCacheP04Record record = {0};
    unsigned char guarded[98];
    record.request_id = 1; record.kind = 1;
    record.submit.version = 1; record.submit.major = 3;
    record.submit.range_known = 1; record.submit.offset = 4096; record.submit.length = 512;
    memset(guarded, 0xa5, sizeof guarded);
    win_cache_p04_encode_record(&record, guarded + 1);
    CHECK(memcmp(guarded + 1, expected_submit, 96) == 0);
    CHECK(guarded[0] == 0xa5 && guarded[97] == 0xa5);
    memset(&record, 0, sizeof record);
    record.seq = 1; record.request_id = 1; record.kind = 2;
    record.complete.version = 1; record.complete.information_known = 1; record.complete.information = 512;
    win_cache_p04_encode_record(&record, guarded + 1);
    CHECK(memcmp(guarded + 1, expected_complete, 96) == 0);
    CHECK(guarded[0] == 0xa5 && guarded[97] == 0xa5);
}
static void rejected(void) {
    unsigned char bytes[49]; WinCacheP04Request out, before;
    unsigned int n;
    memset(&out, 0xa5, sizeof out); memcpy(&before, &out, sizeof out);
    memcpy(bytes, begin_bytes, 48); bytes[48] = 0;
    for (n = 0; n != 48; ++n) {
        CHECK(win_cache_p04_decode_request(0x8337e000u, bytes, n, 144, &out) == 2);
        CHECK(memcmp(&out, &before, sizeof out) == 0);
    }
    CHECK(win_cache_p04_decode_request(0x8337e000u, bytes, 49, 144, &out) == 2);
    CHECK(win_cache_p04_decode_request(0x8337e001u, bytes, 48, 144, &out) == 1);
    CHECK(win_cache_p04_decode_request(0x83372000u, bytes, 48, 144, &out) == 1);
    CHECK(win_cache_p04_decode_request(0x8337e000u, bytes, 48, 143, &out) == 3);
    CHECK(win_cache_p04_decode_request(0x8337e000u, bytes, 48, 145, &out) == 3);
    CHECK(win_cache_p04_decode_request(0x8337e000u, NULL, 48, 144, &out) == 2);
    CHECK(win_cache_p04_decode_request(0x8337e000u, bytes, 48, 144, NULL) == 2);
    for (n = 0; n != 48; ++n) {
        /* Every fixed byte except nonzero session bytes must be exact. */
        if ((n >= 16 && n < 24)) continue;
        memcpy(bytes, begin_bytes, 48); bytes[n] ^= 0x80;
        CHECK(win_cache_p04_decode_request(0x8337e000u, bytes, 48, 144, &out) == 2);
    }
    memcpy(bytes, begin_bytes, 48); bytes[16] = 0;
    CHECK(win_cache_p04_decode_request(0x8337e000u, bytes, 48, 144, &out) == 2);
    CHECK(memcmp(&out, &before, sizeof out) == 0);
}
static void drain_limits(void) {
    unsigned char bytes[48]; WinCacheP04Request r;
    memcpy(bytes, begin_bytes, 48); bytes[32] = 2;
    for (unsigned int n = 1; n <= 16; ++n) {
        bytes[40] = (unsigned char)n;
        CHECK(win_cache_p04_decode_request(0x8337e008u, bytes, 48, 1680, &r) == 0);
        CHECK(r.command == 3 && r.max_records == n && r.call_id == 2);
    }
    bytes[40] = 0; CHECK(win_cache_p04_decode_request(0x8337e008u, bytes, 48, 1680, &r) == 2);
    bytes[40] = 17; CHECK(win_cache_p04_decode_request(0x8337e008u, bytes, 48, 1680, &r) == 2);
    bytes[40] = 1; CHECK(win_cache_p04_decode_request(0x8337e008u, bytes, 48, 144, &r) == 3);
    CHECK(win_cache_p04_decode_request(0x8337e008u, bytes, 48, 1679, &r) == 3);
    CHECK(win_cache_p04_decode_request(0x8337e008u, bytes, 48, 1681, &r) == 3);
    bytes[40] = 0;
    CHECK(win_cache_p04_decode_request(0x8337e004u, bytes, 48, 144, &r) == 0 && r.command == 2);
    CHECK(win_cache_p04_decode_request(0x8337e00cu, bytes, 48, 144, &r) == 0 && r.command == 4);
}
static void unknown_not_zero(void) {
    WinCacheP04Record r = {0}; unsigned char unknown[96], known[96];
    r.request_id = 1; r.kind = 2; r.complete.version = 1;
    win_cache_p04_encode_record(&r, unknown);
    r.complete.information_known = 1;
    win_cache_p04_encode_record(&r, known);
    CHECK(unknown[68] == 0 && known[68] == 1 && memcmp(unknown, known, 96) != 0);
    for (unsigned int n = 72; n < 80; ++n) CHECK(unknown[n] == 0 && known[n] == 0);
}
static void full_width(void) {
    unsigned char bytes[48], header[144]; WinCacheP04Request request;
    WinCacheP04Stats stats = { 0x0807060504030201ULL, 1, 0, 0, 7, 0,
        0x0807060504030201ULL, 0, 0x0807060504030201ULL, 0 };
    memcpy(bytes, begin_bytes, 48);
    for (unsigned int n = 0; n < 8; ++n) { bytes[16+n] = (unsigned char)(n+1); bytes[32+n] = 255; }
    CHECK(win_cache_p04_decode_request(0x8337e004u, bytes, 48, 144, &request) == 0);
    CHECK(request.session_id == 0x0807060504030201ULL && request.call_id == 0xffffffffffffffffULL);
    win_cache_p04_encode_header(&request, &stats, 1, 0, header);
    for (unsigned int n = 0; n < 8; ++n) {
        CHECK(header[32+n] == n+1 && header[48+n] == 255);
        CHECK(header[80+n] == n+1 && header[96+n] == n+1);
    }
}
#define RUN(fn, name) do { fn(); puts("P04_CASE_PASS " name); ++cases; } while (0)
int main(void) {
    unsigned int cases = 0;
    RUN(lengths, "wire_lengths_and_ioctl_values"); RUN(header_literal, "wire_header_literal_and_bounds");
    RUN(record_literal, "wire_known_read_pair"); RUN(rejected, "wire_binding_and_reserved_rejected");
    RUN(drain_limits, "wire_actual_returned_bytes_not_capacity"); RUN(unknown_not_zero, "wire_unknown_information_distinct_from_zero");
    RUN(full_width, "wire_full_width_little_endian");
    printf("P04_SUITE_PASS suite=wire cases=%u\n", cases); return 0;
}
