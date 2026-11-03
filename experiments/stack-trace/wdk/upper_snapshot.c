/* Compile-only upper-candidate scalar snapshots. No entry, callback,
 * registration, attach, forward, complete, cancel, transport or image link.
 */
#if !defined(_KERNEL_MODE)
#error P02_WDK_OBJECT_REQUIRES_KERNEL_COMPILER_MODE
#endif
#include "p02_snapshot.h"

_Static_assert(IRP_MJ_READ == 0x03, "read major");
_Static_assert(IRP_MJ_WRITE == 0x04, "write major");
_Static_assert(IRP_MJ_FLUSH_BUFFERS == 0x09, "flush major");
_Static_assert(IRP_MJ_DEVICE_CONTROL == 0x0e, "public control major");
_Static_assert(IRP_MJ_INTERNAL_DEVICE_CONTROL == 0x0f, "internal major excluded");
_Static_assert(sizeof(NTSTATUS) == 4, "status bits");
_Static_assert(sizeof(ULONG_PTR) == 8, "AMD64 information width");
_Static_assert(sizeof(((IO_STACK_LOCATION *)0)->Parameters.Read.Length) == 4, "read length width");
_Static_assert(sizeof(((IO_STACK_LOCATION *)0)->Parameters.Write.Length) == 4, "write length width");
_Static_assert(sizeof(((IO_STACK_LOCATION *)0)->Parameters.Read.ByteOffset.QuadPart) == 8, "read offset width");
_Static_assert(sizeof(((IO_STACK_LOCATION *)0)->Parameters.Write.ByteOffset.QuadPart) == 8, "write offset width");

_IRQL_requires_max_(DISPATCH_LEVEL)
WinCacheP02U32 win_cache_p02_snapshot_submit(
    _In_opt_ const IO_STACK_LOCATION *input,
    _Out_opt_ WinCacheP02Submit *output)
{
    const WinCacheP02Submit empty = { 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 };
    WinCacheP02U8 major;
    WinCacheP02U8 minor;
    WinCacheP02U8 flags;
    WinCacheP02I64 offset = 0;
    WinCacheP02U32 length = 0;
    WinCacheP02U32 control = 0;
    if (output != NULL) *output = empty;
    if (input == NULL || output == NULL) return WIN_CACHE_P02_NULL;
    major = input->MajorFunction;
    minor = input->MinorFunction;
    flags = input->Flags;

    /* Inspect only the matching public union arm. Major0x0f is also SCSI and
     * must never be treated as the public DeviceIoControl arm here.
     */
    if (minor == 0) {
        if (major == IRP_MJ_READ) {
            offset = input->Parameters.Read.ByteOffset.QuadPart;
            length = input->Parameters.Read.Length;
        } else if (major == IRP_MJ_WRITE) {
            offset = input->Parameters.Write.ByteOffset.QuadPart;
            length = input->Parameters.Write.Length;
        } else if (major == IRP_MJ_DEVICE_CONTROL) {
            control = input->Parameters.DeviceIoControl.IoControlCode;
        }
    }
    return win_cache_p02_submit_scalars(major, minor, flags, offset, length, control, output);
}

_IRQL_requires_max_(DISPATCH_LEVEL)
WinCacheP02U32 win_cache_p02_snapshot_complete(
    _In_opt_ const IO_STATUS_BLOCK *input,
    _In_opt_ const WinCacheP02Submit *saved_submission,
    _Out_opt_ WinCacheP02Completion *output)
{
    const WinCacheP02Completion empty = { 0, 0, 0, 0, 0 };
    WinCacheP02U32 status;
    WinCacheP02U64 information = 0;
    if (output != NULL) *output = empty;
    if (input == NULL || saved_submission == NULL || output == NULL) return WIN_CACHE_P02_NULL;
    if (!win_cache_p02_submit_valid(saved_submission)) return WIN_CACHE_P02_INVALID_SAVED;
    status = (WinCacheP02U32)input->Status;

    /* Branch BEFORE argument evaluation. Do not copy IO_STATUS_BLOCK, use
     * NT_SUCCESS, or inspect Information for PnP/control/flush/failure/minors.
     * A copied range's presence is unrelated to this ordinary-transfer check.
     */
    if (win_cache_p02_information_allowed(saved_submission, status)) {
        information = (WinCacheP02U64)input->Information;
    }
    return win_cache_p02_complete_scalars(saved_submission, status, information, output);
}
