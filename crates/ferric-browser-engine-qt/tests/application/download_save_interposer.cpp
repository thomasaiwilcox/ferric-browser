// Test-only interposer, loaded solely into the owned download fixture.
#include <dlfcn.h>
#include <fcntl.h>
#include <sys/syscall.h>
#include <unistd.h>
#include <cstdlib>
#include <cerrno>
#include <cstdio>
#include <cstring>
#include <string>

extern "C" int fsync(int descriptor) {
    static const auto real = reinterpret_cast<int (*)(int)>(dlsym(RTLD_NEXT, "fsync"));
    if (!real) { errno = ENOSYS; return -1; }
    const auto root = std::getenv("FERRIC_TEST_SAVE_BARRIER_ROOT");
    char link[64], path[4096];
    std::snprintf(link, sizeof(link), "/proc/self/fd/%d", descriptor);
    const auto length = readlink(link, path, sizeof(path)-1);
    if (!root || !*root || length <= 0) return real(descriptor);
    path[length] = 0;
    if (!std::strstr(path, "/.ferric-download-")) return real(descriptor);
    const std::string base(root);
    const int prefixFd = open((base+"/prefix").c_str(), O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
    if (prefixFd < 0) return real(descriptor);
    char prefix[4096];
    const auto size = read(prefixFd, prefix, sizeof(prefix)-1);
    close(prefixFd);
    if (size <= 0) return real(descriptor);
    prefix[size] = 0;
    if (std::strncmp(path, prefix, size) != 0) return real(descriptor);
    const int entered = open((base+"/entered").c_str(), O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0600);
    if (entered >= 0) {
        char thread[64];
        const auto count = std::snprintf(thread, sizeof(thread), "%ld", syscall(SYS_gettid));
        const auto ignored = write(entered, thread, count);
        (void)ignored;
        close(entered);
    }
    // The external fixture watchdog releases this barrier even if the GUI
    // stalls or a test assertion fails. There is no production test hook.
    while (access((base+"/release").c_str(), F_OK) != 0) usleep(2000);
    const int left = open((base+"/left").c_str(), O_WRONLY | O_CREAT | O_CLOEXEC, 0600);
    if (left >= 0) close(left);
    return real(descriptor);
}
