/* This is an ordinary user-mode host TU even when the shared compiler command
 * defines _KERNEL_MODE for the actual driver and fake WDK declarations. */
#if defined(_KERNEL_MODE)
#undef _KERNEL_MODE
#endif
#if !defined(_WIN32)
#define _GNU_SOURCE
#endif
#include "platform.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
struct P04HostMutex { CRITICAL_SECTION lock; };
struct P04HostCond { CONDITION_VARIABLE value; };
struct P04HostThread { HANDLE handle; void (*function)(void *); void *context; };
#else
#include <errno.h>
#include <pthread.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>
struct P04HostMutex { pthread_mutex_t lock; };
struct P04HostCond { pthread_cond_t value; };
struct P04HostThread { pthread_t handle; void (*function)(void *); void *context; };
#endif

_Noreturn void p04_host_fail(const char *reason)
{
    fprintf(stderr, "P04_HOST_ASSERT %s\n", reason);
    fflush(stderr);
    abort();
}
static void *p04_host_allocate(size_t bytes)
{
    void *result = calloc(1, bytes);
    if (result == NULL) p04_host_fail("allocation");
    return result;
}
P04HostMutex *p04_host_mutex_new(void)
{
    P04HostMutex *result = p04_host_allocate(sizeof(*result));
#if defined(_WIN32)
    InitializeCriticalSection(&result->lock);
#else
    if (pthread_mutex_init(&result->lock, NULL) != 0) p04_host_fail("mutex_init");
#endif
    return result;
}
void p04_host_mutex_free(P04HostMutex *value)
{
#if defined(_WIN32)
    DeleteCriticalSection(&value->lock);
#else
    if (pthread_mutex_destroy(&value->lock) != 0) p04_host_fail("mutex_destroy");
#endif
    free(value);
}
void p04_host_mutex_lock(P04HostMutex *value)
{
#if defined(_WIN32)
    EnterCriticalSection(&value->lock);
#else
    if (pthread_mutex_lock(&value->lock) != 0) p04_host_fail("mutex_lock");
#endif
}
void p04_host_mutex_unlock(P04HostMutex *value)
{
#if defined(_WIN32)
    LeaveCriticalSection(&value->lock);
#else
    if (pthread_mutex_unlock(&value->lock) != 0) p04_host_fail("mutex_unlock");
#endif
}
P04HostCond *p04_host_cond_new(void)
{
    P04HostCond *result = p04_host_allocate(sizeof(*result));
#if defined(_WIN32)
    InitializeConditionVariable(&result->value);
#else
    if (pthread_cond_init(&result->value, NULL) != 0) p04_host_fail("cond_init");
#endif
    return result;
}
void p04_host_cond_free(P04HostCond *value)
{
#if !defined(_WIN32)
    if (pthread_cond_destroy(&value->value) != 0) p04_host_fail("cond_destroy");
#endif
    free(value);
}
void p04_host_cond_signal(P04HostCond *value)
{
#if defined(_WIN32)
    WakeAllConditionVariable(&value->value);
#else
    if (pthread_cond_broadcast(&value->value) != 0) p04_host_fail("cond_signal");
#endif
}
void p04_host_cond_wait(P04HostCond *value, P04HostMutex *mutex)
{
#if defined(_WIN32)
    if (!SleepConditionVariableCS(&value->value, &mutex->lock, 10000))
        p04_host_fail("condition_watchdog");
#else
    struct timespec deadline;
    if (clock_gettime(CLOCK_REALTIME, &deadline) != 0) p04_host_fail("clock");
    deadline.tv_sec += 10;
    if (pthread_cond_timedwait(&value->value, &mutex->lock, &deadline) != 0)
        p04_host_fail("condition_watchdog");
#endif
}
#if defined(_WIN32)
static DWORD WINAPI p04_host_thread_entry(LPVOID context)
#else
static void *p04_host_thread_entry(void *context)
#endif
{
    P04HostThread *thread = context;
    thread->function(thread->context);
#if defined(_WIN32)
    return 0;
#else
    return NULL;
#endif
}
P04HostThread *p04_host_thread_start(void (*function)(void *), void *context)
{
    P04HostThread *result = p04_host_allocate(sizeof(*result));
    result->function = function;
    result->context = context;
#if defined(_WIN32)
    result->handle = CreateThread(NULL, 0, p04_host_thread_entry, result, 0, NULL);
    if (result->handle == NULL) p04_host_fail("thread_start");
#else
    if (pthread_create(&result->handle, NULL, p04_host_thread_entry, result) != 0)
        p04_host_fail("thread_start");
#endif
    return result;
}
void p04_host_thread_join(P04HostThread *value)
{
#if defined(_WIN32)
    if (WaitForSingleObject(value->handle, 10000) != WAIT_OBJECT_0)
        p04_host_fail("thread_join_watchdog");
    if (!CloseHandle(value->handle)) p04_host_fail("thread_close");
#else
    struct timespec deadline;
    if (clock_gettime(CLOCK_REALTIME, &deadline) != 0) p04_host_fail("clock");
    deadline.tv_sec += 10;
    if (pthread_timedjoin_np(value->handle, NULL, &deadline) != 0)
        p04_host_fail("thread_join_watchdog");
#endif
    free(value);
}
P04HostRegion p04_host_region_new(size_t bytes)
{
    P04HostRegion result = {0};
    size_t page;
#if defined(_WIN32)
    SYSTEM_INFO info;
    GetSystemInfo(&info);
    page = info.dwPageSize;
#else
    const long system_page = sysconf(_SC_PAGESIZE);
    if (system_page <= 0) p04_host_fail("page_size");
    page = (size_t)system_page;
#endif
    if (bytes == 0 || bytes > 1048576) p04_host_fail("guard_region_bounds");
    result.accessible_bytes = ((bytes + page - 1) / page) * page;
    result.mapping_bytes = result.accessible_bytes + 2 * page;
#if defined(_WIN32)
    result.mapping = VirtualAlloc(NULL, result.mapping_bytes, MEM_RESERVE, PAGE_NOACCESS);
    if (result.mapping == NULL) p04_host_fail("guard_reserve");
    result.accessible = (unsigned char *)result.mapping + page;
    if (VirtualAlloc(result.accessible, result.accessible_bytes, MEM_COMMIT, PAGE_READWRITE)
            != result.accessible) p04_host_fail("guard_commit");
#else
    result.mapping = mmap(NULL, result.mapping_bytes, PROT_NONE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (result.mapping == MAP_FAILED) p04_host_fail("guard_reserve");
    result.accessible = (unsigned char *)result.mapping + page;
    if (mprotect(result.accessible, result.accessible_bytes, PROT_READ | PROT_WRITE) != 0)
        p04_host_fail("guard_commit");
#endif
    /* Right-align so even a one-byte overrun enters the trailing guard page. */
    result.bytes = (unsigned char *)result.accessible + result.accessible_bytes - bytes;
    memset(result.bytes, 0, bytes);
    return result;
}
void p04_host_region_protect(P04HostRegion *value)
{
#if defined(_WIN32)
    DWORD previous;
    if (!VirtualProtect(value->accessible, value->accessible_bytes, PAGE_NOACCESS, &previous))
        p04_host_fail("guard_protect");
#else
    if (mprotect(value->accessible, value->accessible_bytes, PROT_NONE) != 0)
        p04_host_fail("guard_protect");
#endif
}
void p04_host_region_free(P04HostRegion *value)
{
    if (value->mapping == NULL) return;
#if defined(_WIN32)
    if (!VirtualFree(value->mapping, 0, MEM_RELEASE)) p04_host_fail("guard_release");
#else
    if (munmap(value->mapping, value->mapping_bytes) != 0) p04_host_fail("guard_release");
#endif
    memset(value, 0, sizeof(*value));
}
