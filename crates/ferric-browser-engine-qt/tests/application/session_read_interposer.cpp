// Loaded only by the owned recovery fixture. Hold one exact checkpoint read
// on the storage worker so cancellation is tested before completion arrives.
#include <atomic>
#include <chrono>
#include <cstdio>
#include <cstring>
#include <dlfcn.h>
#include <mutex>
#include <string>
#include <thread>
#include <unistd.h>

static std::string target;
static std::mutex targetMutex;
static std::atomic<bool> armed{false}, entered{false}, released{true};

extern "C" void ferric_session_read_arm(const char *path) {
    const std::lock_guard<std::mutex> lock(targetMutex);
    target = path;
    entered.store(false);
    released.store(false);
    armed.store(true, std::memory_order_release);
}
extern "C" bool ferric_session_read_entered() { return entered.load(); }
extern "C" void ferric_session_read_release() { released.store(true); }

extern "C" ssize_t read(int descriptor, void *buffer, size_t size) {
    static const auto real = reinterpret_cast<ssize_t (*)(int, void *, size_t)>(dlsym(RTLD_NEXT, "read"));
    const auto result = real(descriptor, buffer, size);
    if (result <= 0 || !armed.load(std::memory_order_acquire)) return result;
    char link[64], path[4096];
    std::snprintf(link, sizeof(link), "/proc/self/fd/%d", descriptor);
    const auto length = readlink(link, path, sizeof(path)-1);
    if (length <= 0) return result;
    path[length] = 0;
    bool expected = true;
    {
        const std::lock_guard<std::mutex> lock(targetMutex);
        if (target != path || !armed.compare_exchange_strong(expected, false)) return result;
    }
    entered.store(true);
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(15);
    while (!released.load() && std::chrono::steady_clock::now() < deadline)
        std::this_thread::sleep_for(std::chrono::milliseconds(2));
    return result;
}
