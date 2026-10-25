#pragma once

#include <QByteArray>
#include <QObject>
#include <QStringList>
#include <QtGlobal>

#include <functional>
#include <memory>

class QProcess;
class QTimer;

namespace ferric::credentials {

class CredentialProcessTestAccess;

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
  };

  struct Events {
    std::function<void(quint64)> started;
    std::function<void(quint64, const QByteArray &)> frame;
    std::function<void(quint64, Failure, int)> finished;
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
  void drainStdout(const std::shared_ptr<Run> &run);
  void drainStderr(const std::shared_ptr<Run> &run);
  void fail(const std::shared_ptr<Run> &run, Failure failure);
  void terminate(const std::shared_ptr<Run> &run);
  void finish(const std::shared_ptr<Run> &run, int exitCode, bool crashed);
  bool isCurrent(const std::shared_ptr<Run> &run) const;

  Events events_;
  std::shared_ptr<Run> active_;
  quint64 nextToken_ = 1;

  friend class CredentialProcessTestAccess;
};

} // namespace ferric::credentials
