// Loaded only by the owned Settings durability fixture. Fail the final
// directory sync for one exact temporary profile path; file syncs still work.
#include <cerrno>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <dlfcn.h>
#include <sys/stat.h>
#include <unistd.h>

extern "C" int fsync(int descriptor) {
    static const auto real = reinterpret_cast<int (*)(int)>(dlsym(RTLD_NEXT, "fsync"));
    if (!real) { errno = ENOSYS; return -1; }
    const auto root = std::getenv("FERRIC_SETTINGS_SYNC_FAULT_PATH");
    struct stat info;
    if (!root || !*root || fstat(descriptor, &info) != 0 || !S_ISDIR(info.st_mode))
        return real(descriptor);
    char link[64], path[4096];
    std::snprintf(link, sizeof(link), "/proc/self/fd/%d", descriptor);
    const auto length = readlink(link, path, sizeof(path) - 1);
    if (length <= 0) return real(descriptor);
    path[length] = 0;
    if (std::strcmp(path, root) != 0) return real(descriptor);
    errno = EIO;
    return -1;
}
