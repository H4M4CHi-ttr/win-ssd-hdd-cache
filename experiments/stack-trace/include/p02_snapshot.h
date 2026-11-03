#ifndef WIN_CACHE_P02_SNAPSHOT_H
#define WIN_CACHE_P02_SNAPSHOT_H

/* Original scalar-only representation for capture-producer-v1.
 * Keep natural platform packing. No raw structure-byte serialization/FFI call
 * bridge, ownership transfer or runtime synchronization is provided here.
 */
#if defined(_KERNEL_MODE)
#include <ntddk.h>
typedef UCHAR WinCacheP02U8;
typedef ULONG WinCacheP02U32;
typedef ULONGLONG WinCacheP02U64;
typedef LONGLONG WinCacheP02I64;
#define WIN_CACHE_P02_OFFSET_OF(type, field) FIELD_OFFSET(type, field)
#else
#include <stdint.h>
#include <stddef.h>
typedef uint8_t WinCacheP02U8;
typedef uint32_t WinCacheP02U32;
typedef uint64_t WinCacheP02U64;
typedef int64_t WinCacheP02I64;
#define WIN_CACHE_P02_OFFSET_OF(type, field) offsetof(type, field)
#endif

#define WIN_CACHE_P02_OK 0u
#define WIN_CACHE_P02_NULL 1u
#define WIN_CACHE_P02_INVALID_SAVED 2u
#define WIN_CACHE_P02_MAX_U64 ((WinCacheP02U64)0xffffffffffffffffULL)
#define WIN_CACHE_P02_MAX_I64 ((WinCacheP02U64)0x7fffffffffffffffULL)
#define WIN_CACHE_P02_MAX_U32 ((WinCacheP02U64)0xffffffffULL)

_Static_assert(sizeof(WinCacheP02U8) == 1, "u8 width");
_Static_assert(sizeof(WinCacheP02U32) == 4, "u32 width");
_Static_assert(sizeof(WinCacheP02U64) == 8, "u64 width");
_Static_assert(sizeof(WinCacheP02I64) == 8, "i64 width");
_Static_assert((WinCacheP02I64)-1 < 0, "signed source offset");

typedef struct WinCacheP02Submit {
    WinCacheP02U32 version;
    WinCacheP02U32 major;
    WinCacheP02U32 minor;
    WinCacheP02U32 flags;
    WinCacheP02U32 control_known;
    WinCacheP02U32 range_known;
    WinCacheP02U32 control_code;
    WinCacheP02U32 reserved;
    WinCacheP02U64 offset;
    WinCacheP02U64 length;
} WinCacheP02Submit;

typedef struct WinCacheP02Completion {
    WinCacheP02U32 version;
    WinCacheP02U32 status;
    WinCacheP02U32 information_known;
    WinCacheP02U32 reserved;
    WinCacheP02U64 information;
} WinCacheP02Completion;

_Static_assert(sizeof(WinCacheP02Submit) == 48, "submit size");
_Static_assert(_Alignof(WinCacheP02Submit) == 8, "submit alignment");
_Static_assert(WIN_CACHE_P02_OFFSET_OF(WinCacheP02Submit, version) == 0, "submit version");
_Static_assert(WIN_CACHE_P02_OFFSET_OF(WinCacheP02Submit, major) == 4, "submit major");
_Static_assert(WIN_CACHE_P02_OFFSET_OF(WinCacheP02Submit, minor) == 8, "submit minor");
_Static_assert(WIN_CACHE_P02_OFFSET_OF(WinCacheP02Submit, flags) == 12, "submit flags");
_Static_assert(WIN_CACHE_P02_OFFSET_OF(WinCacheP02Submit, control_known) == 16, "submit control presence");
_Static_assert(WIN_CACHE_P02_OFFSET_OF(WinCacheP02Submit, range_known) == 20, "submit range presence");
_Static_assert(WIN_CACHE_P02_OFFSET_OF(WinCacheP02Submit, control_code) == 24, "submit control");
_Static_assert(WIN_CACHE_P02_OFFSET_OF(WinCacheP02Submit, reserved) == 28, "submit reserved");
_Static_assert(WIN_CACHE_P02_OFFSET_OF(WinCacheP02Submit, offset) == 32, "submit offset");
_Static_assert(WIN_CACHE_P02_OFFSET_OF(WinCacheP02Submit, length) == 40, "submit length");
_Static_assert(sizeof(WinCacheP02Completion) == 24, "completion size");
_Static_assert(_Alignof(WinCacheP02Completion) == 8, "completion alignment");
_Static_assert(WIN_CACHE_P02_OFFSET_OF(WinCacheP02Completion, version) == 0, "completion version");
_Static_assert(WIN_CACHE_P02_OFFSET_OF(WinCacheP02Completion, status) == 4, "completion status");
_Static_assert(WIN_CACHE_P02_OFFSET_OF(WinCacheP02Completion, information_known) == 8, "completion presence");
_Static_assert(WIN_CACHE_P02_OFFSET_OF(WinCacheP02Completion, reserved) == 12, "completion reserved");
_Static_assert(WIN_CACHE_P02_OFFSET_OF(WinCacheP02Completion, information) == 16, "completion information");

static inline int win_cache_p02_ordinary_transfer(WinCacheP02U32 major, WinCacheP02U32 minor)
{
    return minor == 0 && (major == 0x03 || major == 0x04);
}

static inline int win_cache_p02_submit_valid(const WinCacheP02Submit *value)
{
    if (value == NULL || value->version != 1 || value->major > 255
            || value->minor > 255 || value->flags > 255 || value->reserved != 0
            || value->control_known > 1 || value->range_known > 1) return 0;
    if (value->control_known == 0) {
        if (value->control_code != 0) return 0;
    } else if (value->major != 0x0e || value->minor != 0) {
        return 0;
    }
    if (value->range_known == 0) {
        if (value->offset != 0 || value->length != 0) return 0;
    } else {
        if (!win_cache_p02_ordinary_transfer(value->major, value->minor)
                || value->offset > WIN_CACHE_P02_MAX_I64
                || value->length > WIN_CACHE_P02_MAX_U32
                || value->length > WIN_CACHE_P02_MAX_U64 - value->offset) return 0;
    }
    return 1;
}

static inline int win_cache_p02_information_allowed(const WinCacheP02Submit *saved,
                                                  WinCacheP02U32 status)
{
    return win_cache_p02_submit_valid(saved) && status == 0
        && win_cache_p02_ordinary_transfer(saved->major, saved->minor);
}

static inline int win_cache_p02_completion_valid(const WinCacheP02Submit *saved,
                                               const WinCacheP02Completion *value)
{
    if (!win_cache_p02_submit_valid(saved) || value == NULL || value->version != 1
            || value->reserved != 0 || value->information_known > 1) return 0;
    if (value->information_known == 0) return value->information == 0;
    return win_cache_p02_information_allowed(saved, value->status);
}

/* Constructors accept already-copied scalars, never WDK objects. In the WDK
 * wrapper, branch BEFORE reading any request-dependent union/Information field.
 * Passing a forbidden field here and redacting it inside is not permitted.
 * Output must not alias any source/saved object; all pointers are call-local.
 */
static inline WinCacheP02U32 win_cache_p02_submit_scalars(
    WinCacheP02U8 major, WinCacheP02U8 minor, WinCacheP02U8 flags,
    WinCacheP02I64 signed_offset, WinCacheP02U32 length, WinCacheP02U32 control,
    WinCacheP02Submit *output)
{
    const WinCacheP02Submit empty = { 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 };
    if (output == NULL) return WIN_CACHE_P02_NULL;
    *output = empty;
    output->version = 1;
    output->major = major;
    output->minor = minor;
    output->flags = flags;
    if (win_cache_p02_ordinary_transfer(major, minor) && signed_offset >= 0) {
        const WinCacheP02U64 offset = (WinCacheP02U64)signed_offset;
        const WinCacheP02U64 wide_length = (WinCacheP02U64)length;
        if (wide_length <= WIN_CACHE_P02_MAX_U64 - offset) {
            output->range_known = 1;
            output->offset = offset;
            output->length = wide_length;
        }
    } else if (major == 0x0e && minor == 0) {
        output->control_known = 1;
        output->control_code = control;
    }
    return WIN_CACHE_P02_OK;
}

static inline WinCacheP02U32 win_cache_p02_complete_scalars(
    const WinCacheP02Submit *saved, WinCacheP02U32 status,
    WinCacheP02U64 information, WinCacheP02Completion *output)
{
    const WinCacheP02Completion empty = { 0, 0, 0, 0, 0 };
    if (output == NULL) return WIN_CACHE_P02_NULL;
    *output = empty;
    if (saved == NULL) return WIN_CACHE_P02_NULL;
    if (!win_cache_p02_submit_valid(saved)) return WIN_CACHE_P02_INVALID_SAVED;
    output->version = 1;
    output->status = status;
    if (win_cache_p02_information_allowed(saved, status)) {
        output->information_known = 1;
        output->information = information;
    }
    return WIN_CACHE_P02_OK;
}

#if defined(_KERNEL_MODE)
/* Ordinary synchronous helpers, NOT dispatch/completion callbacks. The future
 * caller independently proves residency, <= DISPATCH_LEVEL, stable live inputs,
 * same-request saved classification and no aliasing. No pointer is retained.
 * Result values concern snapshot production only and are NOT NTSTATUS.
 */
_IRQL_requires_max_(DISPATCH_LEVEL)
WinCacheP02U32 win_cache_p02_snapshot_submit(
    _In_opt_ const IO_STACK_LOCATION *input,
    _Out_opt_ WinCacheP02Submit *output);

_IRQL_requires_max_(DISPATCH_LEVEL)
WinCacheP02U32 win_cache_p02_snapshot_complete(
    _In_opt_ const IO_STATUS_BLOCK *input,
    _In_opt_ const WinCacheP02Submit *saved_submission,
    _Out_opt_ WinCacheP02Completion *output);
#endif

#endif
