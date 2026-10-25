#include "credential_process.h"

#include <QCoreApplication>
#include <QPointer>
#include <QProcess>
#include <QTimer>

#include <algorithm>
#include <utility>
#include <vector>

#ifdef Q_OS_UNIX
#include <signal.h>
#include <sys/types.h>
#include <unistd.h>
#endif

namespace ferric::credentials {
namespace {
constexpr qsizetype MaxRequestBytes = 1024 * 1024;
constexpr qsizetype MaxOutputBytes = 4 * 1024 * 1024;
constexpr qsizetype MaxFrameBytes = 32 * 1024;
constexpr qsizetype MaxStderrBytes = 64 * 1024;
constexpr int DeadlineMs = 180'000;
constexpr int TerminateGraceMs = 1'000;

void signalProcessGroup(QProcess &process, int signalNumber) {
#ifdef Q_OS_UNIX
  const auto pid = static_cast<pid_t>(process.processId());
  if (pid > 0)
    ::kill(-pid, signalNumber);
#else
  Q_UNUSED(process);
  Q_UNUSED(signalNumber);
#endif
}
} // namespace

struct CredentialProcess::Run {
  explicit Run(quint64 value) : token(value) {}

  quint64 token;
  QProcess process;
  QTimer deadline;
  QTimer grace;
  QByteArray pending;
  qsizetype outputBytes = 0;
  qsizetype stderrBytes = 0;
  Failure failure = Failure::None;
  bool terminalAcknowledged = false;
  bool terminalDelivered = false;
  bool finishing = false;
  bool terminating = false;
};

CredentialProcess::CredentialProcess(QObject *parent) : QObject(parent) {}

CredentialProcess::~CredentialProcess() {
  if (!active_)
    return;
  static std::vector<std::shared_ptr<Run>> reaper;
  auto run = std::move(active_);
  run->deadline.stop();
  signalProcessGroup(run->process, SIGKILL);
  if (run->process.state() != QProcess::NotRunning) {
    auto *app = QCoreApplication::instance();
    if (app) {
      auto *raw = run.get();
      reaper.push_back(run);
      connect(&run->process,
              qOverload<int, QProcess::ExitStatus>(&QProcess::finished), app,
              [raw] {
                QTimer::singleShot(0, QCoreApplication::instance(), [raw] {
                  const auto retained =
                      std::remove_if(reaper.begin(), reaper.end(),
                                     [raw](const auto &candidate) {
                                       return candidate.get() == raw;
                                     });
                  reaper.erase(retained, reaper.end());
                });
              });
    }
    run->process.kill();
  }
}

void CredentialProcess::setEvents(Events events) {
  events_ = std::move(events);
}

quint64 CredentialProcess::start(const QByteArray &request) {
  const auto helper = QCoreApplication::applicationDirPath() +
                      QStringLiteral("/ferric-phone-auth-helper");
  return startConfigured(request, helper, {});
}

quint64 CredentialProcess::startConfigured(const QByteArray &request,
                                           const QString &program,
                                           const QStringList &arguments) {
  if (active_ || nextToken_ == 0 || request.isEmpty() ||
      request.size() > MaxRequestBytes || request.back() != '\n' ||
      request.left(request.size() - 1).contains('\n') || program.isEmpty())
    return 0;

  const quint64 token = nextToken_++;
  auto run = std::make_shared<Run>(token);
  active_ = run;
  run->process.setProgram(program);
  run->process.setArguments(arguments);
  run->process.setProcessChannelMode(QProcess::SeparateChannels);
#ifdef Q_OS_UNIX
  QProcess::UnixProcessParameters parameters;
  parameters.flags |= QProcess::UnixProcessFlag::CreateNewSession;
  parameters.flags |= QProcess::UnixProcessFlag::CloseFileDescriptors;
  run->process.setUnixProcessParameters(parameters);
#endif
  run->deadline.setSingleShot(true);
  run->grace.setSingleShot(true);

  const std::weak_ptr<Run> weak = run;
  connect(&run->process, &QProcess::started, this, [this, weak, request] {
    const auto current = weak.lock();
    if (!current || !isCurrent(current) || current->terminating)
      return;
    if (current->process.write(request) != request.size()) {
      fail(current, Failure::WriteFailure);
      return;
    }
    current->process.closeWriteChannel();
    QPointer<CredentialProcess> guard(this);
    const auto callback = events_.started;
    if (callback)
      callback(current->token);
    if (!guard)
      return;
  });
  connect(&run->process, &QProcess::readyReadStandardOutput, this,
          [this, weak] {
            if (const auto current = weak.lock())
              drainStdout(current);
          });
  connect(&run->process, &QProcess::readyReadStandardError, this, [this, weak] {
    if (const auto current = weak.lock())
      drainStderr(current);
  });
  connect(&run->process, &QProcess::errorOccurred, this,
          [this, weak](QProcess::ProcessError error) {
            const auto current = weak.lock();
            if (!current || !isCurrent(current))
              return;
            if (error == QProcess::FailedToStart) {
              current->failure = Failure::StartFailure;
              finish(current, -1, false);
            } else if (current->failure == Failure::None) {
              fail(current, Failure::ProcessFailure);
            }
          });
  connect(&run->process,
          qOverload<int, QProcess::ExitStatus>(&QProcess::finished), this,
          [this, weak](int exitCode, QProcess::ExitStatus status) {
            if (const auto current = weak.lock())
              finish(current, exitCode, status == QProcess::CrashExit);
          });
  connect(&run->deadline, &QTimer::timeout, this, [this, weak] {
    if (const auto current = weak.lock())
      fail(current, Failure::Deadline);
  });
  connect(&run->grace, &QTimer::timeout, this, [this, weak] {
    const auto current = weak.lock();
    if (!current || !isCurrent(current) ||
        current->process.state() == QProcess::NotRunning)
      return;
    signalProcessGroup(current->process, SIGKILL);
    current->process.kill();
  });

  run->deadline.start(DeadlineMs);
  run->process.start();
  return token;
}

void CredentialProcess::cancel(quint64 token) {
  if (active_ && active_->token == token)
    fail(active_, Failure::Cancelled);
}

void CredentialProcess::acknowledgeTerminal(quint64 token) {
  if (active_ && active_->token == token && !active_->terminalAcknowledged)
    active_->terminalAcknowledged = true;
}

quint64 CredentialProcess::activeToken() const {
  return active_ ? active_->token : 0;
}

bool CredentialProcess::isCurrent(const std::shared_ptr<Run> &run) const {
  return run && active_ == run;
}

void CredentialProcess::drainStdout(const std::shared_ptr<Run> &run) {
  if (!isCurrent(run))
    return;
  const QByteArray bytes = run->process.readAllStandardOutput();
  if (bytes.isEmpty())
    return;
  if (run->terminating)
    return;
  run->outputBytes += bytes.size();
  if (run->outputBytes > MaxOutputBytes) {
    fail(run, Failure::OutputTooLarge);
    return;
  }
  for (const char byte : bytes) {
    if (!isCurrent(run))
      return;
    if (run->terminalAcknowledged) {
      fail(run, Failure::LateOutput);
      return;
    }
    if (byte == '\n') {
      QPointer<CredentialProcess> guard(this);
      const auto callback = events_.frame;
      if (callback)
        callback(run->token, run->pending);
      if (!guard)
        return;
      run->pending.clear();
      if (!guard->isCurrent(run) || run->terminating)
        return;
    } else {
      run->pending.append(byte);
      if (run->pending.size() > MaxFrameBytes) {
        fail(run, Failure::FrameTooLarge);
        return;
      }
    }
  }
}

void CredentialProcess::drainStderr(const std::shared_ptr<Run> &run) {
  if (!isCurrent(run))
    return;
  const QByteArray bytes = run->process.readAllStandardError();
  if (run->terminating)
    return;
  run->stderrBytes += bytes.size();
  if (run->stderrBytes > MaxStderrBytes)
    fail(run, Failure::StderrTooLarge);
}

void CredentialProcess::fail(const std::shared_ptr<Run> &run, Failure failure) {
  if (!isCurrent(run) || run->failure != Failure::None)
    return;
  run->failure = failure;
  terminate(run);
}

void CredentialProcess::terminate(const std::shared_ptr<Run> &run) {
  if (!isCurrent(run) || run->terminating)
    return;
  run->terminating = true;
  run->deadline.stop();
  if (run->process.state() == QProcess::NotRunning) {
    finish(run, -1, false);
    return;
  }
  signalProcessGroup(run->process, SIGTERM);
  run->process.terminate();
  run->grace.start(TerminateGraceMs);
}

void CredentialProcess::finish(const std::shared_ptr<Run> &run, int exitCode,
                               bool crashed) {
  if (!isCurrent(run) || run->terminalDelivered || run->finishing)
    return;
  run->finishing = true;
  drainStdout(run);
  drainStderr(run);
  if (!run->pending.isEmpty() && run->failure == Failure::None)
    run->failure = Failure::TruncatedFrame;
  if (run->failure == Failure::None) {
    if (crashed)
      run->failure = Failure::ProcessFailure;
    else if (exitCode != 0)
      run->failure = Failure::NonzeroExit;
  }
  run->deadline.stop();
  run->grace.stop();
  run->terminalDelivered = true;
  QTimer::singleShot(0, this, [this, weak = std::weak_ptr<Run>(run)] {
    const auto current = weak.lock();
    if (current && isCurrent(current) &&
        current->process.state() == QProcess::NotRunning)
      active_.reset();
  });
  const auto callback = events_.finished;
  if (callback)
    callback(run->token, run->failure, exitCode);
}

} // namespace ferric::credentials
