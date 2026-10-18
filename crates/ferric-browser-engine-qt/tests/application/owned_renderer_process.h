#pragma once

#include <QByteArray>
#include <QScopeGuard>
#include <QString>
#include <cerrno>
#include <csignal>
#include <cstring>
#include <fcntl.h>
#include <limits>
#include <sys/syscall.h>
#include <unistd.h>

namespace FerricTest {

// The caller obtains the renderer PID from its owned WebEngineView. A forked
// Chromium renderer may retain its zygote's command line, so argv is not an
// identity check. Verify the process ancestry and signal the captured process
// instance through a pidfd, never a potentially reused numeric PID.
inline QString terminateOwnedRenderer(qint64 rendererPid) {
    const auto owner = ::getpid();
    if (rendererPid <= 1 || rendererPid == owner ||
        rendererPid > std::numeric_limits<pid_t>::max())
        return QStringLiteral("Refusing an invalid renderer PID");
    const int pidfd = static_cast<int>(::syscall(SYS_pidfd_open, rendererPid, 0));
    if (pidfd < 0)
        return QStringLiteral("Cannot capture renderer process: ") +
               QString::fromLocal8Bit(std::strerror(errno));
    const auto closePidfd = qScopeGuard([pidfd] { ::close(pidfd); });
    qint64 current = rendererPid;
    for (int depth = 0; depth < 64 && current > 1; ++depth) {
        const auto path = QStringLiteral("/proc/%1/status").arg(current).toLocal8Bit();
        const int fd = ::open(path.constData(), O_RDONLY | O_CLOEXEC);
        if (fd < 0)
            return QStringLiteral("Cannot inspect renderer ancestry: ") +
                   QString::fromLocal8Bit(std::strerror(errno));
        QByteArray status;
        char buffer[4096];
        while (status.size() < 16384) {
            const auto count = ::read(fd, buffer, sizeof(buffer));
            if (count < 0 && errno == EINTR) continue;
            if (count <= 0) break;
            status.append(buffer, count);
        }
        ::close(fd);
        qint64 parent = 0;
        for (const auto &line : status.split('\n')) {
            if (!line.startsWith("PPid:")) continue;
            bool valid = false;
            parent = line.mid(5).trimmed().toLongLong(&valid);
            if (!valid) parent = 0;
            break;
        }
        if (parent == owner) {
            if (::syscall(SYS_pidfd_send_signal, pidfd, SIGTERM, nullptr, 0) == 0)
                return {};
            return QStringLiteral("Cannot terminate owned renderer: ") +
                   QString::fromLocal8Bit(std::strerror(errno));
        }
        if (parent <= 1 || parent == current) break;
        current = parent;
    }
    return QStringLiteral("Renderer is not a descendant of this test process");
}

} // namespace FerricTest
