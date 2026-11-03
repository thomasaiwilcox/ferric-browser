#pragma once
#ifndef FERRIC_CREDENTIAL_PROCESS_H
#define FERRIC_CREDENTIAL_PROCESS_H

#include <QByteArray>
#include <QObject>
#include <QStringList>
#include <QtGlobal>

#include <rust/cxx.h>

#include <cstdint>
#include <functional>
#include <memory>

class QProcess;
class QTimer;

namespace ferric::credentials {

class CredentialProcessTestAccess;
class CredentialProcessBridge;
class CredentialLifecycle;
struct CredentialEvents;

// Private native seam for the later credential adapter.  It has no QML type,
// properties, invokables, or page-controlled process configuration.
class CredentialProcess final : public QObject {
public:
  enum class Failure {
    None,
    InvalidRequest,
    Busy,
    StartFailure,
    ProcessFailure,
    NonzeroExit,
    Cancelled,
    Deadline,
    FrameTooLarge,
    OutputTooLarge,
    StderrTooLarge,
    TruncatedFrame,
    LateOutput,
    WriteFailure,
    ProtocolFailure,
  };

  struct Events {
    std::function<void(quint64)> started;
    std::function<void(quint64, const QByteArray &)> frame;
    std::function<void(quint64, Failure, int)> finished;
    // Used only by the private typed-v1 bridge. The legacy observer above
    // retains its original 32 KiB framing and callback contract.
    std::function<int(quint64, const QByteArray &)> admitFrame;
  };

  explicit CredentialProcess(QObject *parent = nullptr);
  ~CredentialProcess() override;

  void setEvents(Events events);
  quint64 start(const QByteArray &request);
  void cancel(quint64 token);
  void acknowledgeTerminal(quint64 token);
  quint64 activeToken() const;

private:
  struct Run;
  qint64 activeProcessId() const;
  bool requestSubmitted() const;
  quint64 startConfigured(const QByteArray &request, const QString &program,
                          const QStringList &arguments);
  quint64 startTypedV1(const QByteArray &request);
  quint64 startWithFrameLimit(const QByteArray &request, const QString &program,
                              const QStringList &arguments,
                              qsizetype maxFrameBytes);
  quint64 startConfiguredForTest(const QByteArray &request,
                                 const QString &program,
                                 const QStringList &arguments);
  quint64 startWithPolicy(const QByteArray &request, const QString &program,
                          const QStringList &arguments, qsizetype maxFrameBytes,
                          int deadlineMs, int graceMs,
                          bool armDeadlineOnReady = false);
  void armDeadlineForTest(quint64 token);
  bool graceActiveForTest(quint64 token) const;
  void drainStdout(const std::shared_ptr<Run> &run);
  void drainStderr(const std::shared_ptr<Run> &run);
  void fail(const std::shared_ptr<Run> &run, Failure failure);
  void terminate(const std::shared_ptr<Run> &run);
  void finish(const std::shared_ptr<Run> &run, int exitCode, bool crashed);
  bool isCurrent(const std::shared_ptr<Run> &run) const;
  void replaceBridgeEvents(Events events,
                           std::shared_ptr<CredentialLifecycle> lifecycle);
  void deferStdoutUntilFinishForTest();
  std::uint64_t lifecycleSnapshot() const;
  std::uint64_t lifecycleSnapshot(
      const std::shared_ptr<CredentialLifecycle> &lifecycle) const;
  std::uint64_t lifecycleProgressSnapshot() const;
  std::uint64_t lifecyclePendingBytes() const;

  Events events_;
  std::shared_ptr<CredentialLifecycle> lifecycle_;
  std::shared_ptr<Run> active_;
  quint64 nextToken_ = 1;
  Failure lastFailure_ = Failure::None;

  friend class CredentialProcessTestAccess;
  friend class CredentialProcessBridge;
};

class CredentialProcessBridge final {
public:
  static quint64 start(CredentialProcess &process, const QByteArray &request);
  static void install(CredentialProcess &process,
                      CredentialProcess::Events events,
                      std::shared_ptr<CredentialLifecycle> lifecycle);
};

std::unique_ptr<CredentialProcess> new_credential_process();
void install_rust_events(CredentialProcess &process,
                         rust::Box<CredentialEvents> events);
std::uint64_t start_credential_process(CredentialProcess &process,
                                       rust::Slice<const std::uint8_t> request);
void cancel_credential_process(CredentialProcess &process, std::uint64_t token);

} // namespace ferric::credentials
#endif // FERRIC_CREDENTIAL_PROCESS_H
