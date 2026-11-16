// Loaded only by the owned Settings durability fixture. Fail the final
// directory sync for one exact temporary profile path; file syncs still work.
#include <cerrno>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <dlfcn.h>
#include <sys/stat.h>
#include <sys/prctl.h>
#include <unistd.h>

extern "C" int fsync(int descriptor) {
    static const auto real = reinterpret_cast<int (*)(int)>(dlsym(RTLD_NEXT, "fsync"));
    if (!real) { errno = ENOSYS; return -1; }
    const auto root = std::getenv("FERRIC_SETTINGS_SYNC_FAULT_PATH");
    struct stat info;
    const auto holdRoot = std::getenv("FERRIC_SETTINGS_SYNC_HOLD_PATH");
    if ((!root || !*root) && (!holdRoot || !*holdRoot)) return real(descriptor);
    if (fstat(descriptor, &info) != 0 || !S_ISDIR(info.st_mode))
        return real(descriptor);
    char link[64], path[4096];
    std::snprintf(link, sizeof(link), "/proc/self/fd/%d", descriptor);
    const auto length = readlink(link, path, sizeof(path) - 1);
    if (length <= 0) return real(descriptor);
    path[length] = 0;
    bool heldTarget = false;
    const auto holdFile = std::getenv("FERRIC_SETTINGS_SYNC_HOLD_FILE");
    if (holdRoot && std::strcmp(path, holdRoot) == 0
            && (!holdFile || access(holdFile, F_OK) == 0)) {
        char threadName[16] {};
        const auto expectedThread = std::getenv("FERRIC_SETTINGS_SYNC_HOLD_THREAD");
        const bool named = prctl(PR_GET_NAME, threadName, 0, 0, 0) == 0;
        if (expectedThread && (!named || std::strcmp(threadName, expectedThread) != 0))
            return real(descriptor);
        heldTarget = true;
        const auto readyFacts = std::getenv("FERRIC_SETTINGS_SYNC_READY_FACTS");
        if (readyFacts) if (auto *file = std::fopen(readyFacts, "w")) {
            std::fprintf(file, "%s\n%s\n", threadName, path);
            std::fclose(file);
        }
        const auto ready = std::getenv("FERRIC_SETTINGS_SYNC_READY_FILE");
        const auto release = std::getenv("FERRIC_SETTINGS_SYNC_RELEASE_FILE");
        if (ready && release) {
            const std::string readyFile(ready), releaseFile(release);
            if (auto *file = std::fopen(readyFile.c_str(), "w")) std::fclose(file);
            // Bounded fixture hold; an assertion failure must never strand a
            // writer or test teardown indefinitely.
            for (int attempt = 0; attempt < 10000 && access(releaseFile.c_str(), F_OK) != 0; ++attempt)
                usleep(1000);
            if (access(releaseFile.c_str(), F_OK) != 0) {
                const auto expired = std::getenv("FERRIC_SETTINGS_SYNC_HOLD_EXPIRED_FILE");
                if (expired) if (auto *file = std::fopen(expired, "w")) std::fclose(file);
            }
        }
    }
    const auto complete = [&](int outcome) {
        const int actualErrno = errno;
        if (heldTarget) {
            const auto completed = std::getenv("FERRIC_SETTINGS_SYNC_COMPLETED_FACTS");
            if (completed) if (auto *file = std::fopen(completed, "w")) {
                std::fprintf(file, "%d\n%s\n", outcome, path);
                std::fclose(file);
            }
        }
        errno = actualErrno;
        return outcome;
    };
    if (!root || std::strcmp(path, root) != 0) return complete(real(descriptor));
    const auto faultFile = std::getenv("FERRIC_SETTINGS_SYNC_FAULT_FILE");
    if (faultFile && access(faultFile, F_OK) != 0) return complete(real(descriptor));
    errno = EIO;
    return complete(-1);
}
