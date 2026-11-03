#include "credential_process.h"
#include "ferric-browser-engine-qt/src/bridge.cxxqt.h"

#include <QCoreApplication>
#include <QEventLoop>
#include <QPointer>
#include <QTemporaryDir>
#include <QTimer>
#include <QtTest/QtTest>

#include <cerrno>
#include <chrono>
#include <csignal>
#include <fstream>
#include <functional>
#include <iostream>
#include <memory>
#include <string>
#include <thread>
#include <utility>
#ifdef Q_OS_UNIX
#include <sys/types.h>
#include <unistd.h>
#endif

namespace ferric::credentials {
class CredentialProcessTestAccess final {
public:
  static quint64 start(CredentialProcess &process, const QByteArray &request,
                       const QString &program, const QStringList &arguments,
                       bool shortDeadline = false) {
    return shortDeadline
               ? process.startConfiguredForTest(request, program, arguments)
               : process.startWithFrameLimit(request, program, arguments,
                                             1024 * 1024);
  }
  static CredentialProcess::Failure failure(const CredentialProcess &process) {
    return process.lastFailure_;
  }
  static std::uint64_t snapshot(const CredentialProcess &process) {
    return process.lifecycleSnapshot();
  }
  static std::uint64_t progressSnapshot(const CredentialProcess &process) {
    return process.lifecycleProgressSnapshot();
  }
  static std::uint64_t pendingBytes(const CredentialProcess &process) {
    return process.lifecyclePendingBytes();
  }
  static std::shared_ptr<CredentialLifecycle>
  lifecycle(const CredentialProcess &process) {
    return process.lifecycle_;
  }
  static std::uint64_t
  snapshot(const CredentialProcess &process,
           const std::shared_ptr<CredentialLifecycle> &lifecycle) {
    return process.lifecycleSnapshot(lifecycle);
  }
  static qint64 pid(const CredentialProcess &process) {
    return process.activeProcessId();
  }
  static void afterAdmission(CredentialProcess &process,
                             std::function<void(quint64)> hook) {
    auto original = process.events_.admitFrame;
    auto pending =
        std::make_shared<std::function<void(quint64)>>(std::move(hook));
    process.events_.admitFrame = [original = std::move(original), pending](
                                     quint64 token, const QByteArray &frame) {
      const int result = original(token, frame);
      if (*pending) {
        auto callback = std::move(*pending);
        callback(token);
      }
      return result;
    };
  }
  static void deferStdoutUntilFinish(CredentialProcess &process) {
    process.deferStdoutUntilFinishForTest();
  }
  static void armDeadline(CredentialProcess &process, quint64 token) {
    process.armDeadlineForTest(token);
  }
  static bool graceActive(const CredentialProcess &process, quint64 token) {
    return process.graceActiveForTest(token);
  }
  static void captureStartedPid(CredentialProcess &process, qint64 *pid) {
    QPointer<CredentialProcess> guard(&process);
    const auto started = process.events_.started;
    process.events_.started = [guard, pid, started](quint64 token) {
      if (started)
        started(token);
      if (!guard || guard->activeToken() != token)
        return;
      const auto currentPid = guard->activeProcessId();
      if (currentPid > 0)
        *pid = currentPid;
    };
  }
};
} // namespace ferric::credentials

static std::string padded(std::string frame, std::size_t size) {
  frame.pop_back();
  if (frame.size() + 1 < size)
    frame.append(size - frame.size() - 1, ' ');
  frame.push_back('}');
  return frame;
}

static std::string qrFrame() {
  std::string frame =
      R"({"v":1,"id":"r1","kind":"qr","body":{"width":21,"modules":[)";
  for (int i = 0; i < 441; ++i) {
    if (i != 0)
      frame.push_back(',');
    frame.push_back('0');
  }
  frame += "]}}";
  return frame;
}

static void writeLine(const std::string &frame) { std::cout << frame << '\n'; }

static int fixture(const char *mode, const char *controlPath = nullptr) {
  std::string ignored;
  std::getline(std::cin, ignored);
  const std::string name(mode);
  if (name == "valid-large-terminal") {
    writeLine(
        R"({"body":{"status":"submitted"},"kind":"status","id":"r1","v":1})");
    writeLine(padded(
        R"({"v":1,"id":"r1","kind":"error","body":{"code":"authenticator_failed"}})",
        40 * 1024));
  } else if (name == "oversized-status") {
    writeLine(padded(
        R"({"v":1,"id":"r1","kind":"status","body":{"status":"submitted"}})",
        33 * 1024));
  } else if (name == "oversized-qr") {
    writeLine(padded(qrFrame(), 33 * 1024));
  } else if (name == "oversized-terminal") {
    writeLine(padded(
        R"({"v":1,"id":"r1","kind":"error","body":{"code":"authenticator_failed"}})",
        1024 * 1024 + 1));
  } else if (name == "mixed-burst") {
    std::string batch;
    for (int i = 0; i < 11; ++i)
      batch +=
          R"({"v":1,"id":"r1","kind":"status","body":{"status":"submitted"}})"
          "\n";
    batch += qrFrame() + "\n";
    batch +=
        R"({"v":1,"id":"r1","kind":"status","body":{"status":"completed"}})"
        "\n";
    batch += qrFrame() + "\n";
    batch +=
        R"({"v":1,"id":"r1","kind":"error","body":{"code":"authenticator_failed"}})"
        "\n";
    std::cout.write(batch.data(), static_cast<std::streamsize>(batch.size()));
    std::cout.flush();
  } else if (name == "rate-progress") {
    if (controlPath == nullptr)
      return 41;
    for (int delivered = 1; delivered <= 10; ++delivered) {
      writeLine(
          R"({"v":1,"id":"r1","kind":"status","body":{"status":"submitted"}})");
      std::cout.flush();
      bool acknowledged = false;
      for (int attempt = 0; attempt < 3000; ++attempt) {
        std::ifstream control(controlPath);
        int value = 0;
        if (control >> value; value >= delivered) {
          acknowledged = true;
          break;
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(1));
      }
      if (!acknowledged)
        return 42;
    }
    writeLine(
        R"({"v":1,"id":"r1","kind":"status","body":{"status":"completed"}})");
    writeLine(
        R"({"v":1,"id":"r1","kind":"error","body":{"code":"authenticator_failed"}})");
    std::cout.flush();
  } else if (name == "secret-result") {
    writeLine(
        R"({"v":1,"id":"r1","kind":"result","body":{"id":"fixture-secret","rawId":"fixture-secret","response":{"clientDataJSON":"fixture-secret"},"clientExtensionResults":{},"type":"public-key"}})");
  } else if (name == "secret-get") {
    writeLine(
        R"({"v":1,"id":"r1","kind":"result","body":{"assertions":[{"id":"fixture-secret","rawId":"fixture-secret","response":{"clientDataJSON":"fixture-secret"},"clientExtensionResults":{},"type":"public-key"}]}})");
  } else if (name == "duplicate-terminal") {
    writeLine(
        R"({"v":1,"id":"r1","kind":"error","body":{"code":"authenticator_failed"}})");
    writeLine(
        R"({"v":1,"id":"r1","kind":"error","body":{"code":"authenticator_failed"}})");
  } else if (name == "final-only-terminal") {
    writeLine(
        R"({"v":1,"id":"r1","kind":"error","body":{"code":"authenticator_failed"}})");
  } else if (name == "status-hang" || name == "hang") {
    if (name == "status-hang")
      writeLine(
          R"({"v":1,"id":"r1","kind":"status","body":{"status":"submitted"}})");
    std::cout.flush();
    std::this_thread::sleep_for(std::chrono::seconds(5));
  } else if (name == "ignore-term-hang") {
    std::signal(SIGTERM, SIG_IGN);
    writeLine(
        R"({"v":1,"id":"r1","kind":"status","body":{"status":"submitted"}})");
    std::cout.flush();
    std::this_thread::sleep_for(std::chrono::seconds(5));
  } else if (name == "qr-hang") {
    writeLine(qrFrame());
    std::cout.flush();
    std::this_thread::sleep_for(std::chrono::seconds(5));
  }
  std::cout.flush();
  return 0;
}

static bool waitFor(QCoreApplication &app,
                    ferric::credentials::CredentialProcess &process,
                    int timeoutMs = 5000, qint64 *childPid = nullptr) {
  QEventLoop loop;
  QTimer poll;
  poll.setInterval(5);
  QObject::connect(&poll, &QTimer::timeout, &loop, [&] {
    const qint64 pid =
        ferric::credentials::CredentialProcessTestAccess::pid(process);
    if (childPid != nullptr && pid > 0)
      *childPid = pid;
    if (process.activeToken() == 0)
      loop.quit();
  });
  QTimer::singleShot(timeoutMs, &loop, &QEventLoop::quit);
  poll.start();
  loop.exec();
  app.processEvents();
  return process.activeToken() == 0;
}

static quint64 start(ferric::credentials::CredentialProcess &process,
                     BrowserUi &ui, const char *mode,
                     bool shortDeadline = false, bool isCreate = true,
                     quint64 incarnation = 1, qint64 *startedPid = nullptr) {
  using namespace ferric::credentials;
  auto events = create_credential_events(ui, "r1", isCreate, incarnation);
  install_rust_events(process, std::move(events));
  if (startedPid != nullptr)
    CredentialProcessTestAccess::captureStartedPid(process, startedPid);
  return CredentialProcessTestAccess::start(
      process, QByteArray("{}\n"), QCoreApplication::applicationFilePath(),
      {QStringLiteral("--fixture-helper"), QString::fromLatin1(mode)},
      shortDeadline);
}

static quint64 startSame(ferric::credentials::CredentialProcess &process,
                         const char *mode) {
  using namespace ferric::credentials;
  return CredentialProcessTestAccess::start(
      process, QByteArray("{}\n"), QCoreApplication::applicationFilePath(),
      {QStringLiteral("--fixture-helper"), QString::fromLatin1(mode)});
}

static bool pidGone(qint64 pid) {
#ifdef Q_OS_UNIX
  return pid > 0 && (::kill(static_cast<pid_t>(pid), 0) != 0 && errno == ESRCH);
#else
  Q_UNUSED(pid);
  return true;
#endif
}

static bool pidAlive(qint64 pid) {
#ifdef Q_OS_UNIX
  if (pid <= 0)
    return false;
  const int result = ::kill(static_cast<pid_t>(pid), 0);
  return result == 0 || errno == EPERM;
#else
  Q_UNUSED(pid);
  return false;
#endif
}

static std::uint32_t sinkCount(std::uint64_t snapshot) {
  return static_cast<std::uint32_t>((snapshot >> 40) & 0x00ff'ffffU);
}
static std::uint8_t sinkTerminals(std::uint64_t snapshot) {
  return static_cast<std::uint8_t>((snapshot >> 32) & 0xffU);
}
static std::uint8_t sinkKind(std::uint64_t snapshot) {
  return static_cast<std::uint8_t>((snapshot >> 28) & 0x0fU);
}
static std::uint16_t sinkQrLength(std::uint64_t snapshot) {
  return static_cast<std::uint16_t>((snapshot >> 4) & 0xffffU);
}
static std::uint8_t progressCount(std::uint64_t snapshot) {
  return static_cast<std::uint8_t>(snapshot & 0xffU);
}
static std::uint8_t progressKind(std::uint64_t snapshot) {
  return static_cast<std::uint8_t>((snapshot >> 8) & 0x0fU);
}
static std::uint16_t progressQrLength(std::uint64_t snapshot) {
  return static_cast<std::uint16_t>((snapshot >> 12) & 0xffffU);
}
static std::uint8_t acceptedProgress(std::uint64_t snapshot) {
  return static_cast<std::uint8_t>((snapshot >> 28) & 0x0fU);
}

static bool runCase(QCoreApplication &app, BrowserUi &ui, const char *mode,
                    ferric::credentials::CredentialProcess::Failure expected,
                    std::uint32_t count, std::uint8_t terminals,
                    bool shortDeadline = false, bool isCreate = true) {
  using namespace ferric::credentials;
  CredentialProcess process;
  const auto token = start(process, ui, mode, shortDeadline, isCreate);
  qint64 childPid = 0;
  if (token == 0 || !waitFor(app, process, 5000, &childPid))
    return false;
  const auto snapshot = CredentialProcessTestAccess::snapshot(process);
  return (!shortDeadline || pidGone(childPid)) &&
         CredentialProcessTestAccess::failure(process) == expected &&
         sinkCount(snapshot) == count && sinkTerminals(snapshot) == terminals;
}

static bool mixedProgress(QCoreApplication &app, BrowserUi &ui) {
  using namespace ferric::credentials;
  CredentialProcess process;
  if (start(process, ui, "mixed-burst") == 0 || !waitFor(app, process))
    return false;
  const auto progress = CredentialProcessTestAccess::progressSnapshot(process);
  const auto result = CredentialProcessTestAccess::snapshot(process);
  return CredentialProcessTestAccess::failure(process) ==
             CredentialProcess::Failure::None &&
         sinkTerminals(result) == 1 && progressCount(progress) == 1 &&
         progressKind(progress) == 2 && progressQrLength(progress) == 441 &&
         acceptedProgress(progress) > 0 && acceptedProgress(progress) <= 10;
}

static bool rateLimitedProgressThroughRustSink(QCoreApplication &app,
                                               BrowserUi &ui) {
  using namespace ferric::credentials;
  QTemporaryDir controlDir;
  if (!controlDir.isValid())
    return false;
  const std::string controlPath =
      controlDir.filePath(QStringLiteral("ack-counter")).toStdString();
  {
    std::ofstream control(controlPath, std::ios::trunc);
    control << 0;
    if (!control)
      return false;
  }

  CredentialProcess process;
  auto events = create_credential_events(ui, "r1", true, 7);
  install_rust_events(process, std::move(events));
  const auto token = CredentialProcessTestAccess::start(
      process, QByteArray("{}\n"), QCoreApplication::applicationFilePath(),
      {QStringLiteral("--fixture-helper"), QStringLiteral("rate-progress"),
       QString::fromStdString(controlPath)});
  if (token == 0)
    return false;

  int acknowledged = 0;
  QEventLoop loop;
  QTimer poll;
  poll.setInterval(2);
  QObject::connect(&poll, &QTimer::timeout, &loop, [&] {
    const auto progress =
        CredentialProcessTestAccess::progressSnapshot(process);
    const int delivered = progressCount(progress);
    if (delivered > acknowledged) {
      acknowledged = delivered;
      std::ofstream control(controlPath, std::ios::trunc);
      control << acknowledged;
    }
    if (process.activeToken() == 0)
      loop.quit();
  });
  QTimer::singleShot(5000, &loop, &QEventLoop::quit);
  const auto startedAt = std::chrono::steady_clock::now();
  poll.start();
  loop.exec();
  app.processEvents();
  const auto elapsed = std::chrono::steady_clock::now() - startedAt;
  const auto progress = CredentialProcessTestAccess::progressSnapshot(process);
  const auto terminal = CredentialProcessTestAccess::snapshot(process);
  return acknowledged == 10 && progressCount(progress) == 10 &&
         acceptedProgress(progress) == 10 && sinkTerminals(terminal) == 1 &&
         elapsed < std::chrono::milliseconds(900);
}

static bool deadlineGraceAndReap(QCoreApplication &app, BrowserUi &ui) {
  using namespace ferric::credentials;
  CredentialProcess process;
  bool ready = false;
  qint64 childPid = 0;
  const auto token =
      start(process, ui, "ignore-term-hang", true, true, 1, &childPid);
  if (token == 0)
    return false;
  CredentialProcessTestAccess::afterAdmission(
      process, [&ready](quint64) { ready = true; });
  QEventLoop readyLoop;
  QTimer readyPoll;
  readyPoll.setInterval(2);
  QObject::connect(&readyPoll, &QTimer::timeout, &readyLoop, [&] {
    if (ready && childPid > 0)
      readyLoop.quit();
  });
  QTimer::singleShot(3000, &readyLoop, &QEventLoop::quit);
  readyPoll.start();
  readyLoop.exec();
  if (!ready || childPid <= 0 || process.activeToken() != token)
    return false;

  const auto armedAt = std::chrono::steady_clock::now();
  CredentialProcessTestAccess::armDeadline(process, token);
  bool observedAliveDuringGrace = false;
  QEventLoop finishLoop;
  QTimer finishPoll;
  finishPoll.setInterval(2);
  QObject::connect(&finishPoll, &QTimer::timeout, &finishLoop, [&] {
    if (CredentialProcessTestAccess::graceActive(process, token) &&
        pidAlive(childPid))
      observedAliveDuringGrace = true;
    if (process.activeToken() == 0)
      finishLoop.quit();
  });
  QTimer::singleShot(3000, &finishLoop, &QEventLoop::quit);
  finishPoll.start();
  finishLoop.exec();
  app.processEvents();
  const auto elapsed = std::chrono::steady_clock::now() - armedAt;
  return observedAliveDuringGrace && pidGone(childPid) &&
         CredentialProcessTestAccess::failure(process) ==
             CredentialProcess::Failure::Deadline &&
         elapsed >= std::chrono::milliseconds(400);
}

static bool cancelAndRestart(QCoreApplication &app, BrowserUi &ui) {
  using namespace ferric::credentials;
  CredentialProcess process;
  if (start(process, ui, "status-hang") == 0)
    return false;
  CredentialProcessTestAccess::afterAdmission(
      process, [&process](quint64 token) { process.cancel(token); });
  qint64 childPid = 0;
  if (!waitFor(app, process, 5000, &childPid) || !pidGone(childPid) ||
      CredentialProcessTestAccess::snapshot(process) != 0)
    return false;
  if (startSame(process, "valid-large-terminal") == 0 || !waitFor(app, process))
    return false;
  const auto snapshot = CredentialProcessTestAccess::snapshot(process);
  return sinkCount(snapshot) == 2 && sinkTerminals(snapshot) == 1;
}

static bool qrSinkDeliveryAndCancel(QCoreApplication &app, BrowserUi &ui) {
  using namespace ferric::credentials;
  CredentialProcess process;
  const auto token = start(process, ui, "qr-hang");
  if (token == 0)
    return false;
  qint64 childPid = 0;
  QEventLoop loop;
  QTimer poll;
  poll.setInterval(5);
  bool observed = false;
  QObject::connect(&poll, &QTimer::timeout, &loop, [&] {
    if (const auto pid = CredentialProcessTestAccess::pid(process); pid > 0)
      childPid = pid;
    const auto value = CredentialProcessTestAccess::snapshot(process);
    if (!observed && sinkKind(value) == 2 && sinkQrLength(value) == 441) {
      observed = true;
      process.cancel(token);
    }
    if (process.activeToken() == 0)
      loop.quit();
  });
  QTimer::singleShot(5000, &loop, &QEventLoop::quit);
  poll.start();
  loop.exec();
  app.processEvents();
  const auto value = CredentialProcessTestAccess::snapshot(process);
  return observed && process.activeToken() == 0 && pidGone(childPid) &&
         sinkCount(value) == 1 && sinkTerminals(value) == 0;
}

static bool ownerDestroyedDuringAdmission(QCoreApplication &app) {
  using namespace ferric::credentials;
  auto ui = std::make_unique<BrowserUi>();
  CredentialProcess process;
  if (start(process, *ui, "status-hang") == 0)
    return false;
  std::uint64_t pendingBeforeDrop = 0;
  CredentialProcessTestAccess::afterAdmission(
      process, [&ui, &process, &pendingBeforeDrop](quint64 token) {
        pendingBeforeDrop = CredentialProcessTestAccess::pendingBytes(process);
        ui.reset();
        process.cancel(token);
      });
  if (!waitFor(app, process))
    return false;
  return pendingBeforeDrop > 0 &&
         CredentialProcessTestAccess::snapshot(process) == 0 &&
         CredentialProcessTestAccess::pendingBytes(process) == 0;
}

static bool lifecycleReplacementInvalidates(QCoreApplication &app,
                                            BrowserUi &ui) {
  using namespace ferric::credentials;
  CredentialProcess process;
  std::shared_ptr<CredentialLifecycle> replaced;
  if (start(process, ui, "status-hang") == 0)
    return false;
  CredentialProcessTestAccess::afterAdmission(
      process, [&process, &ui, &replaced](quint64 token) {
        replaced = CredentialProcessTestAccess::lifecycle(process);
        auto next = create_credential_events(ui, "r1", true, 2);
        install_rust_events(process, std::move(next));
        process.cancel(token);
      });
  if (!waitFor(app, process))
    return false;
  app.processEvents();
  return replaced &&
         CredentialProcessTestAccess::snapshot(process, replaced) == 0 &&
         CredentialProcessTestAccess::pendingBytes(process) == 0 &&
         CredentialProcessTestAccess::snapshot(process) == 0;
}

static bool deleteProcessDuringFinalDrain(QCoreApplication &app,
                                          BrowserUi &ui) {
  using namespace ferric::credentials;
  auto process = std::make_unique<CredentialProcess>();
  auto events = create_credential_events(ui, "r1", true, 3);
  install_rust_events(*process, std::move(events));
  qint64 childPid = 0;
  CredentialProcessTestAccess::captureStartedPid(*process, &childPid);
  CredentialProcessTestAccess::afterAdmission(
      *process, [&process](quint64) { process.reset(); });
  const auto token = CredentialProcessTestAccess::start(
      *process, QByteArray("{}\n"), QCoreApplication::applicationFilePath(),
      {QStringLiteral("--fixture-helper"),
       QStringLiteral("final-only-terminal")});
  if (token == 0)
    return false;
  CredentialProcessTestAccess::deferStdoutUntilFinish(*process);
  QEventLoop loop;
  QTimer poll;
  poll.setInterval(5);
  QObject::connect(&poll, &QTimer::timeout, &loop, [&] {
    if (!process && pidGone(childPid))
      loop.quit();
  });
  QTimer::singleShot(3000, &loop, &QEventLoop::quit);
  poll.start();
  loop.exec();
  return !process && childPid > 0 && pidGone(childPid);
}

static bool deletedDuringAdmissionReaps(QCoreApplication &app, BrowserUi &ui) {
  using namespace ferric::credentials;
  auto process = std::make_unique<CredentialProcess>();
  auto events = create_credential_events(ui, "r1", true, 99);
  install_rust_events(*process, std::move(events));
  qint64 childPid = 0;
  CredentialProcessTestAccess::captureStartedPid(*process, &childPid);
  CredentialProcessTestAccess::afterAdmission(
      *process, [&process](quint64) { process.reset(); });
  const auto token = CredentialProcessTestAccess::start(
      *process, QByteArray("{}\n"), QCoreApplication::applicationFilePath(),
      {QStringLiteral("--fixture-helper"), QStringLiteral("status-hang")});
  if (token == 0)
    return false;
  QEventLoop loop;
  QTimer poll;
  poll.setInterval(10);
  QObject::connect(&poll, &QTimer::timeout, &loop, [&] {
    if (!process && pidGone(childPid))
      loop.quit();
  });
  QTimer::singleShot(2500, &loop, &QEventLoop::quit);
  poll.start();
  loop.exec();
  return !process && childPid > 0 && pidGone(childPid);
}

static bool runBridgeCases(QCoreApplication &app) {
  BrowserUi ui;
  using Failure = ferric::credentials::CredentialProcess::Failure;
  const bool largeTerminal =
      runCase(app, ui, "valid-large-terminal", Failure::None, 2, 1);
  const bool oversizedStatus =
      runCase(app, ui, "oversized-status", Failure::ProtocolFailure, 0, 0);
  const bool oversizedQr =
      runCase(app, ui, "oversized-qr", Failure::ProtocolFailure, 0, 0);
  const bool oversizedTerminal =
      runCase(app, ui, "oversized-terminal", Failure::FrameTooLarge, 0, 0);
  const bool mixedBurst = mixedProgress(app, ui);
  const bool rateLimit = rateLimitedProgressThroughRustSink(app, ui);
  const bool secretResult =
      runCase(app, ui, "secret-result", Failure::None, 1, 1);
  const bool secretGet =
      runCase(app, ui, "secret-get", Failure::None, 1, 1, false, false);
  const bool duplicateTerminal =
      runCase(app, ui, "duplicate-terminal", Failure::LateOutput, 1, 1);
  const bool deadline = deadlineGraceAndReap(app, ui);
  const bool restart = cancelAndRestart(app, ui);
  const bool ownerDrop = ownerDestroyedDuringAdmission(app);
  const bool deleteDuringAdmit = deletedDuringAdmissionReaps(app, ui);
  const bool finalDrainDelete = deleteProcessDuringFinalDrain(app, ui);
  const bool lifecycleReplacement = lifecycleReplacementInvalidates(app, ui);
  const bool qrSink = qrSinkDeliveryAndCancel(app, ui);
  const std::pair<const char *, bool> cases[] = {
      {"large_terminal", largeTerminal},
      {"oversized_status", oversizedStatus},
      {"oversized_qr", oversizedQr},
      {"oversized_terminal", oversizedTerminal},
      {"mixed_progress", mixedBurst},
      {"progress_rate_cap", rateLimit},
      {"secret_create", secretResult},
      {"secret_get", secretGet},
      {"duplicate_terminal", duplicateTerminal},
      {"deadline_grace_reap", deadline},
      {"cancel_restart", restart},
      {"owner_drop_budget", ownerDrop},
      {"delete_during_admit", deleteDuringAdmit},
      {"delete_during_final_drain", finalDrainDelete},
      {"lifecycle_replacement", lifecycleReplacement},
      {"qr_sink_cancel", qrSink},
  };
  bool passed = true;
  for (const auto &[name, result] : cases) {
    if (!result) {
      std::cerr << "T061c_case=" << name << '\n';
      passed = false;
    }
  }
  return passed;
}

class CredentialProcessBridgeTests final : public QObject {
  Q_OBJECT

private slots:
  void nativeRustBridge() {
    QVERIFY2(runBridgeCases(*QCoreApplication::instance()),
             "one or more fixed T061c cases failed; see T061c_case labels");
  }
};

int main(int argc, char **argv) {
  if ((argc == 3 || argc == 4) && std::string(argv[1]) == "--fixture-helper")
    return fixture(argv[2], argc == 4 ? argv[3] : nullptr);
  QCoreApplication app(argc, argv);
  CredentialProcessBridgeTests tests;
  return QTest::qExec(&tests, argc, argv);
}

#include "tst_credential_process_bridge.moc"
