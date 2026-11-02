#include "probe.h"
#include <stdio.h>
#include <stdlib.h>

__declspec(noreturn) void win_cache_probe_panic(void) { abort(); }

int main(void) {
    const uint64_t cases[][6] = {
        {0,0,0,0,0,0}, {4,2,6,0,4,6}, {6,0,6,0,6,6}, {7,9,6,1,0,0},
        {4,3,6,2,0,0}, {UINT64_MAX,0,UINT64_MAX,0,UINT64_MAX,UINT64_MAX},
        {UINT64_MAX,1,UINT64_MAX,2,0,0}, {1,UINT64_MAX,UINT64_MAX,2,0,0}
    };
    for (size_t i = 0; i < sizeof(cases)/sizeof(cases[0]); ++i) {
        WinCacheRangeReply r = win_cache_probe_range(cases[i][0], cases[i][1], cases[i][2]);
        if (r.status != cases[i][3] || r.reserved != 0 || r.start != cases[i][4] || r.end != cases[i][5]) {
            fprintf(stderr, "ABI vector %zu failed\n", i); return 1;
        }
    }
    puts("D09 C-to-Rust ABI: 8 vectors passed; layout 24/8 and offsets 0/4/8/16 checked");
    return 0;
}

