#include "p02_snapshot.h"

#include <stdio.h>
#include <string.h>

/* Hand-derived values from capture-producer-v1, not produced by the constructors.
 * These host scalar tests do not construct, dereference or execute an IRP/WDK
 * callback. The WDK wrapper's forbidden-field access branches need source review.
 */
static unsigned fixed_vectors;
static unsigned strict_cases;

static int fail(const char *name, const char *field)
{
    (void)fprintf(stderr, "P02_C_FAILURE: %s: %s\n", name, field);
    return 0;
}

static int equal_submit(const char *name, const WinCacheP02Submit *got,
                        const WinCacheP02Submit *want)
{
#define EQ_SUBMIT(field) if (got->field != want->field) return fail(name, #field)
    EQ_SUBMIT(version);
    EQ_SUBMIT(major);
    EQ_SUBMIT(minor);
    EQ_SUBMIT(flags);
    EQ_SUBMIT(control_known);
    EQ_SUBMIT(range_known);
    EQ_SUBMIT(control_code);
    EQ_SUBMIT(reserved);
    EQ_SUBMIT(offset);
    EQ_SUBMIT(length);
#undef EQ_SUBMIT
    return 1;
}

static int equal_completion(const char *name, const WinCacheP02Completion *got,
                            const WinCacheP02Completion *want)
{
#define EQ_COMPLETE(field) if (got->field != want->field) return fail(name, #field)
    EQ_COMPLETE(version);
    EQ_COMPLETE(status);
    EQ_COMPLETE(information_known);
    EQ_COMPLETE(reserved);
    EQ_COMPLETE(information);
#undef EQ_COMPLETE
    return 1;
}

static void fixed_pass(const char *name)
{
    ++fixed_vectors;
    (void)printf("P02 C vector %02u %s PASS\n", fixed_vectors, name);
}

static void strict_pass(const char *name)
{
    ++strict_cases;
    (void)printf("P02 C strict %02u %s PASS\n", strict_cases, name);
}

static int submit_vectors(void)
{
    /* Catch wrong union classification, signed addition, loss of zero presence,
     * or dropped major/minor/flags. Ignored scalar inputs are deliberate poison.
     */
    static const struct {
        const char *name;
        WinCacheP02U8 major;
        WinCacheP02U8 minor;
        WinCacheP02U8 flags;
        WinCacheP02I64 offset;
        WinCacheP02U32 length;
        WinCacheP02U32 control;
        WinCacheP02Submit want;
    } vectors[] = {
        { "read-range-flags", 3, 0, 0xa5, 4096, 8192, 0xffffffffu,
          { 1, 3, 0, 0xa5, 0, 1, 0, 0, 4096, 8192 } },
        { "write-zero-known-range", 4, 0, 0, 0, 0, 0xffffffffu,
          { 1, 4, 0, 0, 0, 1, 0, 0, 0, 0 } },
        { "unsigned-exclusive-end", 3, 0, 0, INT64_MAX, 0xffffffffu, 0,
          { 1, 3, 0, 0, 0, 1, 0, 0, UINT64_C(0x7fffffffffffffff), UINT64_C(0xffffffff) } },
        { "negative-read-offset", 3, 0, 0, -1, 4096, 0,
          { 1, 3, 0, 0, 0, 0, 0, 0, 0, 0 } },
        { "minimum-write-offset", 4, 0, 0, INT64_MIN, 0xffffffffu, 0,
          { 1, 4, 0, 0, 0, 0, 0, 0, 0, 0 } },
        { "read-special-minor", 3, 1, 0x80, 8192, 512, 0xdeadbeefu,
          { 1, 3, 1, 0x80, 0, 0, 0, 0, 0, 0 } },
        { "flush-no-range", 9, 0, 0xff, 8192, 512, 0xdeadbeefu,
          { 1, 9, 0, 0xff, 0, 0, 0, 0, 0, 0 } },
        { "public-control-known-zero", 0x0e, 0, 0, 8192, 512, 0,
          { 1, 0x0e, 0, 0, 1, 0, 0, 0, 0, 0 } },
        { "internal-scsi-not-public-control", 0x0f, 0, 0, 8192, 512, 0xdeadbeefu,
          { 1, 0x0f, 0, 0, 0, 0, 0, 0, 0, 0 } },
        { "pnp-other", 0x1b, 7, 0x80, 8192, 512, 0xdeadbeefu,
          { 1, 0x1b, 7, 0x80, 0, 0, 0, 0, 0, 0 } }
    };
    size_t index;
    for (index = 0; index < sizeof(vectors) / sizeof(vectors[0]); ++index) {
        WinCacheP02Submit got;
        (void)memset(&got, 0xa5, sizeof(got));
        if (win_cache_p02_submit_scalars(vectors[index].major, vectors[index].minor,
                vectors[index].flags, vectors[index].offset, vectors[index].length,
                vectors[index].control, &got) != 0) return fail(vectors[index].name, "result");
        if (!equal_submit(vectors[index].name, &got, &vectors[index].want)) return 0;
        if (!win_cache_p02_submit_valid(&got)) return fail(vectors[index].name, "valid");
        if (index == 2 && got.offset + got.length != UINT64_C(0x80000000fffffffe))
            return fail(vectors[index].name, "unsigned-end");
        fixed_pass(vectors[index].name);
    }
    return 1;
}

static int completion_vectors(void)
{
    /* Catch NT_SUCCESS instead of exact zero, clipping Information, range-known
     * gating, or treating PnP/control/flush/special-minor metadata as byte counts.
     */
    static const struct {
        const char *name;
        WinCacheP02Submit saved;
        WinCacheP02U32 status;
        WinCacheP02U64 information;
        WinCacheP02Completion want;
    } vectors[] = {
        { "read-large-information", { 1, 3, 0, 0xa5, 0, 1, 0, 0, 4096, 8192 },
          0, UINT64_C(0x0000000100000001), { 1, 0, 1, 0, UINT64_C(0x0000000100000001) } },
        { "unknown-range-known-zero-information", { 1, 3, 0, 0, 0, 0, 0, 0, 0, 0 },
          0, 0, { 1, 0, 1, 0, 0 } },
        { "special-minor-information-absent", { 1, 3, 1, 0x80, 0, 0, 0, 0, 0, 0 },
          0, UINT64_C(0xffff8123456789ab), { 1, 0, 0, 0, 0 } },
        { "pending-is-not-success-zero", { 1, 3, 0, 0, 0, 1, 0, 0, 4096, 8192 },
          0x103, UINT64_C(0xffff8123456789ab), { 1, 0x103, 0, 0, 0 } },
        { "failed-write-preserves-status", { 1, 4, 0, 0, 0, 1, 0, 0, 0, 0 },
          0xc0000185u, UINT64_C(0xffff8123456789ab), { 1, 0xc0000185u, 0, 0, 0 } },
        { "pnp-pointer-information-absent", { 1, 0x1b, 7, 0x80, 0, 0, 0, 0, 0, 0 },
          0, UINT64_C(0xffff8123456789ab), { 1, 0, 0, 0, 0 } },
        { "flush-information-absent", { 1, 9, 0, 0xff, 0, 0, 0, 0, 0, 0 },
          0, UINT64_C(0xffff8123456789ab), { 1, 0, 0, 0, 0 } },
        { "public-control-information-absent", { 1, 0x0e, 0, 0, 1, 0, 0, 0, 0, 0 },
          0, 4096, { 1, 0, 0, 0, 0 } }
    };
    const WinCacheP02Completion zero = { 0, 0, 0, 0, 0 };
    const WinCacheP02Submit invalid = { 1, 3, 0, 0, 0, 1, 0, 0,
                                      UINT64_C(0x8000000000000000), 1 };
    WinCacheP02Completion got;
    size_t index;
    for (index = 0; index < sizeof(vectors) / sizeof(vectors[0]); ++index) {
        (void)memset(&got, 0xa5, sizeof(got));
        if (win_cache_p02_complete_scalars(&vectors[index].saved, vectors[index].status,
                vectors[index].information, &got) != 0) return fail(vectors[index].name, "result");
        if (!equal_completion(vectors[index].name, &got, &vectors[index].want)) return 0;
        if (!win_cache_p02_completion_valid(&vectors[index].saved, &got))
            return fail(vectors[index].name, "valid");
        fixed_pass(vectors[index].name);
    }
    (void)memset(&got, 0xa5, sizeof(got));
    if (win_cache_p02_complete_scalars(&invalid, 0, 1, &got) != 2)
        return fail("invalid-saved-range", "result");
    if (!equal_completion("invalid-saved-range", &got, &zero)) return 0;
    fixed_pass("invalid-saved-range");
    (void)memset(&got, 0xa5, sizeof(got));
    if (win_cache_p02_complete_scalars(NULL, 0, 1, &got) != 1)
        return fail("null-saved-zero-output", "result");
    if (!equal_completion("null-saved-zero-output", &got, &zero)) return 0;
    fixed_pass("null-saved-zero-output");
    return 1;
}

static int reject_submit(const char *name, const WinCacheP02Submit *bad)
{
    const WinCacheP02Completion zero = { 0, 0, 0, 0, 0 };
    WinCacheP02Completion got;
    if (win_cache_p02_submit_valid(bad)) return fail(name, "accepted-invalid-submit");
    (void)memset(&got, 0xa5, sizeof(got));
    if (win_cache_p02_complete_scalars(bad, 0, UINT64_C(0xffff8123456789ab), &got) != 2)
        return fail(name, "invalid-saved-result");
    if (!equal_completion(name, &got, &zero)) return 0;
    strict_pass(name);
    return 1;
}

static int strict_submit_cases(void)
{
    const WinCacheP02Submit base = { 1, 3, 0, 0, 0, 1, 0, 0, 4096, 8192 };
    WinCacheP02Submit bad;
#define BAD_SUBMIT(name, field, value) do { \
    bad = base; bad.field = (value); if (!reject_submit(name, &bad)) return 0; \
} while (0)
    BAD_SUBMIT("version-zero", version, 0);
    BAD_SUBMIT("version-future", version, 2);
    BAD_SUBMIT("major-width", major, 256);
    BAD_SUBMIT("minor-width", minor, 256);
    BAD_SUBMIT("flags-width", flags, 256);
    BAD_SUBMIT("reserved-submit", reserved, 1);
    BAD_SUBMIT("control-presence-not-bool", control_known, 2);
    BAD_SUBMIT("range-presence-not-bool", range_known, 2);
    BAD_SUBMIT("inactive-control-storage", control_code, 1);
    BAD_SUBMIT("cross-kind-control", control_known, 1);
    BAD_SUBMIT("inactive-range-storage", range_known, 0);
    BAD_SUBMIT("offset-outside-signed-source", offset, UINT64_C(0x8000000000000000));
    BAD_SUBMIT("length-width", length, UINT64_C(0x100000000));
    BAD_SUBMIT("special-minor-range", minor, 1);
    BAD_SUBMIT("flush-range", major, 9);
    BAD_SUBMIT("public-control-range", major, 0x0e);
    BAD_SUBMIT("other-range", major, 0x1b);
#undef BAD_SUBMIT
    bad = (WinCacheP02Submit){ 1, 3, 0, 0, 0, 0, 0, 0, 0, 1 };
    if (!reject_submit("inactive-range-length-only", &bad)) return 0;
    bad = (WinCacheP02Submit){ 1, 3, 0, 0, 0, 0, 0, 0, 1, 0 };
    if (!reject_submit("inactive-range-offset-only", &bad)) return 0;
    bad = (WinCacheP02Submit){ 1, 0x0e, 1, 0, 1, 0, 1, 0, 0, 0 };
    if (!reject_submit("special-minor-control", &bad)) return 0;
    bad = (WinCacheP02Submit){ 1, 0x0f, 0, 0, 1, 0, 1, 0, 0, 0 };
    if (!reject_submit("internal-control-field", &bad)) return 0;
    return 1;
}

static int strict_completion_cases(void)
{
    const WinCacheP02Submit read = { 1, 3, 0, 0, 0, 1, 0, 0, 0, 1 };
    const WinCacheP02Submit other = { 1, 0x1b, 0, 0, 0, 0, 0, 0, 0, 0 };
    const WinCacheP02Submit invalid = { 2, 3, 0, 0, 0, 0, 0, 0, 0, 0 };
    const WinCacheP02Completion base = { 1, 0, 1, 0, 1 };
    const WinCacheP02Completion high = { 1, 0, 1, 0, UINT64_C(0xffff8123456789ab) };
    const WinCacheP02Completion absent = { 1, 0, 0, 0, 0 };
    WinCacheP02Completion bad;
    WinCacheP02Completion got;
#define BAD_COMPLETION(name, field, value) do { \
    bad = base; bad.field = (value); \
    if (win_cache_p02_completion_valid(&read, &bad)) return fail(name, "accepted-invalid-completion"); \
    strict_pass(name); \
} while (0)
    BAD_COMPLETION("completion-version", version, 2);
    BAD_COMPLETION("completion-reserved", reserved, 1);
    BAD_COMPLETION("information-presence-not-bool", information_known, 2);
    BAD_COMPLETION("inactive-information-storage", information_known, 0);
    BAD_COMPLETION("failure-known-information", status, 0xc0000185u);
    BAD_COMPLETION("pending-known-information", status, 0x103);
#undef BAD_COMPLETION
    if (win_cache_p02_completion_valid(&other, &high)) return fail("other-known-information", "accepted");
    strict_pass("other-known-information");
    if (!win_cache_p02_completion_valid(&read, &absent)) return fail("success-can-stay-unknown", "rejected");
    strict_pass("success-can-stay-unknown");
    if (win_cache_p02_complete_scalars(&read, 0, UINT64_C(0xffff8123456789ab), &got) != 0)
        return fail("allowed-high-information-preserved", "result");
    if (!equal_completion("allowed-high-information-preserved", &got, &high)) return 0;
    strict_pass("allowed-high-information-preserved");
    if (win_cache_p02_submit_valid(NULL) || win_cache_p02_completion_valid(NULL, &base)
            || win_cache_p02_completion_valid(&read, NULL)) return fail("null-validator-input", "accepted");
    strict_pass("null-validator-input");
    if (win_cache_p02_submit_scalars(3, 0, 0, 0, 0, 0, NULL) != 1)
        return fail("null-submit-output", "result");
    strict_pass("null-submit-output");
    if (win_cache_p02_complete_scalars(&invalid, 0, 1, NULL) != 1)
        return fail("null-before-invalid-saved", "result");
    strict_pass("null-before-invalid-saved");
    return 1;
}

static int strict_constructor_cases(void)
{
    /* Preserve a nonzero code, not just its presence bit. The two special-minor
     * cases catch normalization that accidentally handles reads differently
     * from writes or omits the minor guard on public control.
     */
    const WinCacheP02Submit control = { 1, 0x0e, 0, 0x40, 1, 0, 0xdeadbeefu, 0, 0, 0 };
    const WinCacheP02Submit special_write = { 1, 4, 0xff, 0x80, 0, 0, 0, 0, 0, 0 };
    const WinCacheP02Submit special_control = { 1, 0x0e, 1, 0x80, 0, 0, 0, 0, 0, 0 };
    WinCacheP02Submit got;
    if (win_cache_p02_submit_scalars(0x0e, 0, 0x40, 1, 1, 0xdeadbeefu, &got) != 0)
        return fail("nonzero-public-control-preserved", "result");
    if (!equal_submit("nonzero-public-control-preserved", &got, &control)) return 0;
    strict_pass("nonzero-public-control-preserved");
    if (win_cache_p02_submit_scalars(4, 0xff, 0x80, 4096, 8192, 0xdeadbeefu, &got) != 0)
        return fail("special-write-minor-normalized", "result");
    if (!equal_submit("special-write-minor-normalized", &got, &special_write)) return 0;
    strict_pass("special-write-minor-normalized");
    if (win_cache_p02_submit_scalars(0x0e, 1, 0x80, 4096, 8192, 0xdeadbeefu, &got) != 0)
        return fail("special-control-minor-normalized", "result");
    if (!equal_submit("special-control-minor-normalized", &got, &special_control)) return 0;
    strict_pass("special-control-minor-normalized");
    return 1;
}

int main(void)
{
    if (!submit_vectors() || !completion_vectors() || !strict_submit_cases()
            || !strict_completion_cases() || !strict_constructor_cases()) return 1;
    if (fixed_vectors != 20 || strict_cases != 36) {
        (void)fail("nonzero-case-count", "count");
        return 1;
    }
    (void)printf("P02_C_SCALAR_VECTORS_PASSED fixed=%u strict=%u\n", fixed_vectors, strict_cases);
    return 0;
}
