#pragma once
/* Included after the unmodified WinSpd ABI header. One private session per broker. */
typedef BOOL (*SdkRustExchange)(BYTE *frame, DWORD size, DWORD *reply_size);
typedef BOOL (*SdkRustClose)(DWORD *origin_error);
DWORD sdk_start(const SPD_IOCTL_STORAGE_UNIT_PARAMS *params, SPD_IOCTL_STORAGE_UNIT_PARAMS *geometry,
                SdkRustExchange transfer, SdkRustClose close_flush, BOOL occupy_inner);
BOOL sdk_request(BYTE *frame, DWORD size, DWORD *response_size, BOOL mismatch);
DWORD sdk_stop(BOOL orderly);
