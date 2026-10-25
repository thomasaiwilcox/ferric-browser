#include "credential_process.h"
#include "ferric-browser-engine-qt/src/bridge.cxxqt.h"

#include <QCoreApplication>
#include <QPointer>
#include <QProcess>
#include <QTimer>

#include <algorithm>
#include <cstdint>
#include <memory>
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
constexpr int TestDeadlineMs = 250;
constexpr int TestGraceMs = 200;

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
  qsizetype maxFrameBytes = MaxFrameBytes;
  qsizetype outputBytes = 0;
  qsizetype stderrBytes = 0;
  Failure failure = Failure::None;
  bool typedV1 = false;
  int deadlineMs = DeadlineMs;
  int graceMs = TerminateGraceMs;
  bool requestSubmitted = false;
  bool terminalAcknowledged = false;
  bool terminalDelivered = false;
  bool finishing = false;
  bool terminating = false;
  bool deferStdoutUntilFinish = false;
  bool deadlineStartsImmediately = true;
};

class CredentialLifecycle final {
public:
  explicit CredentialLifecycle(rust::Box<CredentialEvents> events)
      : retained_(
            std::make_shared<rust::Box<CredentialEvents>>(std::move(events))) {}

  void invalidate(quint64 token) const {
    (*retained_)->process_invalidated(token);
  }
  std::uint64_t snapshot() const { return (*retained_)->process_snapshot(); }
  std::uint64_t progressSnapshot() const {
    return (*retained_)->process_progress_snapshot();
  }
  std::uint64_t pendingBytes() const {
    return (*retained_)->process_pending_bytes();
  }

  const std::shared_ptr<rust::Box<CredentialEvents>> &retained() const {
    return retained_;
  }

private:
  std::shared_ptr<rust::Box<CredentialEvents>> retained_;
};

CredentialProcess::CredentialProcess(QObject *parent) : QObject(parent) {}

CredentialProcess::~CredentialProcess() {
  if (lifecycle_)
    lifecycle_->invalidate(active_ ? active_->token : 0);
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
  if (lifecycle_)
    lifecycle_->invalidate(active_ ? active_->token : 0);
  lifecycle_.reset();
  events_ = std::move(events);
}

void CredentialProcess::replaceBridgeEvents(
    Events events, std::shared_ptr<CredentialLifecycle> lifecycle) {
  if (lifecycle_)
    lifecycle_->invalidate(active_ ? active_->token : 0);
  events_ = std::move(events);
  lifecycle_ = std::move(lifecycle);
}

std::uint64_t CredentialProcess::lifecycleSnapshot() const {
  return lifecycle_ ? lifecycle_->snapshot() : 0;
}

std::uint64_t CredentialProcess::lifecycleSnapshot(
    const std::shared_ptr<CredentialLifecycle> &lifecycle) const {
  return lifecycle ? lifecycle->snapshot() : 0;
}

std::uint64_t CredentialProcess::lifecycleProgressSnapshot() const {
  return lifecycle_ ? lifecycle_->progressSnapshot() : 0;
}

std::uint64_t CredentialProcess::lifecyclePendingBytes() const {
  return lifecycle_ ? lifecycle_->pendingBytes() : 0;
}

void CredentialProcess::deferStdoutUntilFinishForTest() {
  if (active_)
    active_->deferStdoutUntilFinish = true;
}

quint64 CredentialProcess::start(const QByteArray &request) {
  const auto helper = QCoreApplication::applicationDirPath() +
                      QStringLiteral("/ferric-phone-auth-helper");
  return startConfigured(request, helper, {});
}

quint64 CredentialProcess::startConfigured(const QByteArray &request,
                                           const QString &program,
                                           const QStringList &arguments) {
  return startWithFrameLimit(request, program, arguments, MaxFrameBytes);
}

quint64 CredentialProcess::startTypedV1(const QByteArray &request) {
  const auto helper = QCoreApplication::applicationDirPath() +
                      QStringLiteral("/ferric-phone-auth-helper");
  return startWithFrameLimit(request, helper, {}, MaxRequestBytes);
}

quint64 CredentialProcessBridge::start(CredentialProcess &process,
                                       const QByteArray &request) {
  return process.startTypedV1(request);
}

quint64 CredentialProcess::startWithFrameLimit(const QByteArray &request,
                                               const QString &program,
                                               const QStringList &arguments,
                                               qsizetype maxFrameBytes) {
  return startWithPolicy(request, program, arguments, maxFrameBytes, DeadlineMs,
                         TerminateGraceMs);
}

quint64
CredentialProcess::startConfiguredForTest(const QByteArray &request,
                                          const QString &program,
                                          const QStringList &arguments) {
  return startWithPolicy(request, program, arguments, MaxRequestBytes,
                         TestDeadlineMs, TestGraceMs, true);
}

quint64 CredentialProcess::startWithPolicy(const QByteArray &request,
                                           const QString &program,
                                           const QStringList &arguments,
                                           qsizetype maxFrameBytes,
                                           int deadlineMs, int graceMs,
                                           bool armDeadlineOnReady) {
  if (active_ || nextToken_ == 0 || request.isEmpty() ||
      request.size() > MaxRequestBytes || request.back() != '\n' ||
      request.left(request.size() - 1).contains('\n') || program.isEmpty() ||
      maxFrameBytes < MaxFrameBytes || maxFrameBytes > MaxRequestBytes ||
      deadlineMs <= 0 || graceMs <= 0)
    return 0;

  const quint64 token = nextToken_++;
  lastFailure_ = Failure::None;
  auto run = std::make_shared<Run>(token);
  run->maxFrameBytes = maxFrameBytes;
  run->typedV1 = maxFrameBytes > MaxFrameBytes;
  run->deadlineMs = deadlineMs;
  run->graceMs = graceMs;
  run->deadlineStartsImmediately = !armDeadlineOnReady;
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
    QPointer<CredentialProcess> guard(this);
    const auto callback = events_.started;
    if (callback)
      callback(current->token);
    if (!guard)
      return;
    if (!guard->isCurrent(current) || current->terminating)
      return;
    const qint64 written = current->process.write(request);
    current->requestSubmitted = written > 0;
    if (written != request.size()) {
      guard->fail(current, Failure::WriteFailure);
      return;
    }
    current->process.closeWriteChannel();
  });
  connect(&run->process, &QProcess::readyReadStandardOutput, this,
          [this, weak] {
            if (const auto current = weak.lock();
                current && !current->deferStdoutUntilFinish)
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

  if (run->deadlineStartsImmediately)
    run->deadline.start(run->deadlineMs);
  run->process.start();
  return token;
}

void CredentialProcess::armDeadlineForTest(quint64 token) {
  if (active_ && active_->token == token &&
      !active_->deadlineStartsImmediately && !active_->deadline.isActive() &&
      active_->process.state() == QProcess::Running)
    active_->deadline.start(active_->deadlineMs);
}

bool CredentialProcess::graceActiveForTest(quint64 token) const {
  return active_ && active_->token == token &&
         active_->failure == Failure::Deadline && active_->grace.isActive() &&
         active_->process.state() == QProcess::Running;
}

void CredentialProcess::cancel(quint64 token) {
  if (!active_ || active_->token != token)
    return;
  if (lifecycle_)
    lifecycle_->invalidate(token);
  if (!active_ || active_->token != token)
    return;
  fail(active_, Failure::Cancelled);
}

void CredentialProcess::acknowledgeTerminal(quint64 token) {
  if (active_ && active_->token == token && !active_->terminalAcknowledged)
    active_->terminalAcknowledged = true;
}

quint64 CredentialProcess::activeToken() const {
  return active_ ? active_->token : 0;
}

qint64 CredentialProcess::activeProcessId() const {
  return active_ ? active_->process.processId() : 0;
}

bool CredentialProcess::requestSubmitted() const {
  return active_ && active_->requestSubmitted;
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
      const auto admit = events_.admitFrame;
      const auto callback = events_.frame;
      if (admit) {
        const int decision = admit(run->token, run->pending);
        if (!guard)
          return;
        if (!guard->isCurrent(run) || run->terminating)
          return;
        if (decision == 0 || (decision != 1 && decision != 2)) {
          guard->fail(run, Failure::ProtocolFailure);
          return;
        }
        if (decision == 2)
          guard->acknowledgeTerminal(run->token);
      } else if (callback) {
        callback(run->token, run->pending);
        if (!guard)
          return;
        if (!guard->isCurrent(run) || run->terminating)
          return;
      }
      if (!guard)
        return;
      run->pending.clear();
      if (!guard->isCurrent(run) || run->terminating)
        return;
    } else {
      run->pending.append(byte);
      if (run->pending.size() > run->maxFrameBytes) {
        fail(run, Failure::FrameTooLarge);
        return;
      }
    }
  }
}

std::unique_ptr<CredentialProcess> new_credential_process() {
  return std::make_unique<CredentialProcess>();
}

void install_rust_events(CredentialProcess &process,
                         rust::Box<CredentialEvents> events) {
  auto lifecycle = std::make_shared<CredentialLifecycle>(std::move(events));
  const auto retained = lifecycle->retained();
  CredentialProcess::Events callbacks;
  callbacks.started = [retained](quint64 token) {
    (*retained)->process_started(token);
  };
  callbacks.admitFrame = [retained](quint64 token, const QByteArray &frame) {
    const auto *data =
        reinterpret_cast<const std::uint8_t *>(frame.constData());
    return (*retained)->process_frame(
        token, rust::Slice<const std::uint8_t>(
                   data, static_cast<std::size_t>(frame.size())));
  };
  callbacks.finished = [retained](quint64 token,
                                  CredentialProcess::Failure failure, int) {
    (*retained)->process_finished(token, static_cast<std::int32_t>(failure));
  };
  CredentialProcessBridge::install(process, std::move(callbacks),
                                   std::move(lifecycle));
}

void CredentialProcessBridge::install(
    CredentialProcess &process, CredentialProcess::Events events,
    std::shared_ptr<CredentialLifecycle> lifecycle) {
  process.replaceBridgeEvents(std::move(events), std::move(lifecycle));
}

std::uint64_t
start_credential_process(CredentialProcess &process,
                         rust::Slice<const std::uint8_t> request) {
  if (request.size() > static_cast<std::size_t>(MaxRequestBytes))
    return 0;
  const QByteArray bytes(reinterpret_cast<const char *>(request.data()),
                         static_cast<qsizetype>(request.size()));
  return CredentialProcessBridge::start(process, bytes);
}

void cancel_credential_process(CredentialProcess &process,
                               std::uint64_t token) {
  process.cancel(token);
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
  run->grace.start(run->graceMs);
}

void CredentialProcess::finish(const std::shared_ptr<Run> &run, int exitCode,
                               bool crashed) {
  if (!isCurrent(run) || run->terminalDelivered || run->finishing)
    return;
  run->finishing = true;
  QPointer<CredentialProcess> guard(this);
  guard->drainStdout(run);
  if (!guard || !guard->isCurrent(run))
    return;
  guard->drainStderr(run);
  if (!guard || !guard->isCurrent(run))
    return;
  if (!run->pending.isEmpty() && run->failure == Failure::None)
    run->failure = Failure::TruncatedFrame;
  if (run->typedV1 && !run->terminalAcknowledged &&
      run->failure == Failure::None)
    run->failure = Failure::ProtocolFailure;
  if (run->failure == Failure::None) {
    if (crashed)
      run->failure = Failure::ProcessFailure;
    else if (exitCode != 0)
      run->failure = Failure::NonzeroExit;
  }
  run->deadline.stop();
  run->grace.stop();
  run->terminalDelivered = true;
  lastFailure_ = run->failure;
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
