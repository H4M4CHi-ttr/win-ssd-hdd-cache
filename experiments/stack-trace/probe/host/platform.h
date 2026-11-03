#ifndef WIN_CACHE_P04_HOST_PLATFORM_H
#define WIN_CACHE_P04_HOST_PLATFORM_H
#include <stddef.h>
#include <stdint.h>

/* Host-only synchronization and guard pages. Never included by the WDK build. */
typedef struct P04HostMutex P04HostMutex;
typedef struct P04HostCond P04HostCond;
typedef struct P04HostThread P04HostThread;
typedef struct P04HostRegion {
    void *mapping;
    size_t mapping_bytes;
    void *accessible;
    size_t accessible_bytes;
    void *bytes;
} P04HostRegion;
P04HostMutex *p04_host_mutex_new(void);
void p04_host_mutex_free(P04HostMutex *value);
void p04_host_mutex_lock(P04HostMutex *value);
void p04_host_mutex_unlock(P04HostMutex *value);
P04HostCond *p04_host_cond_new(void);
void p04_host_cond_free(P04HostCond *value);
void p04_host_cond_signal(P04HostCond *value);
void p04_host_cond_wait(P04HostCond *value, P04HostMutex *mutex);
P04HostThread *p04_host_thread_start(void (*function)(void *), void *context);
void p04_host_thread_join(P04HostThread *value);
P04HostRegion p04_host_region_new(size_t bytes);
void p04_host_region_protect(P04HostRegion *value);
void p04_host_region_free(P04HostRegion *value);
_Noreturn void p04_host_fail(const char *reason);
#endif
