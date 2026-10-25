#include "../../src/credential_process.h"

#include <QCoreApplication>
#include <QPointer>
#include <QProcess>
#include <QtTest>

#include <cerrno>
#include <cstdio>
#include <cstring>

#ifdef Q_OS_UNIX
#include <signal.h>
#include <sys/types.h>
#include <unistd.h>
#endif

namespace ferric::credentials {

class CredentialProcessTestAccess final {
public:
  static quint64 startMode(CredentialProcess &process,
                           const QByteArray &request, const QString &mode) {
    return process.startConfigured(request,
                                   QCoreApplication::applicationFilePath(),
                                   {QStringLiteral("--fixture-helper"), mode});
  }

  static qint64 childPid(const CredentialProcess &process) {
    return process.active_ ? process.active_->process.processId() : 0;
  }
};

} // namespace ferric::credentials

namespace {
using ferric::credentials::CredentialProcess;
using ferric::credentials::CredentialProcessTestAccess;

int helperMain(const QString &mode) {
  if (mode == QStringLiteral("clean")) {
    std::fputs("progress\nterminal\n", stdout);
    std::fflush(stdout);
    return 0;
  }
  if (mode == QStringLiteral("late")) {
    std::fputs("terminal\nlate\n", stdout);
    std::fflush(stdout);
    return 0;
  }
  if (mode == QStringLiteral("truncated")) {
    std::fputs("partial", stdout);
    std::fflush(stdout);
    return 0;
  }
  if (mode == QStringLiteral("line")) {
    for (int i = 0; i < 40 * 1024; ++i)
      std::fputc('x', stdout);
    std::fputc('\n', stdout);
    std::fflush(stdout);
    return 0;
  }
  if (mode == QStringLiteral("total")) {
    QByteArray line(4095, 'x');
    line.append('\n');
    for (int i = 0; i < 1100; ++i)
      if (std::fwrite(line.constData(), 1, size_t(line.size()), stdout) !=
          size_t(line.size()))
        return 3;
    std::fflush(stdout);
    return 0;
  }
  if (mode == QStringLiteral("stderr")) {
    for (int i = 0; i < 70 * 1024; ++i)
      std::fputc('x', stderr);
    std::fflush(stderr);
    return 0;
  }
  if (mode == QStringLiteral("crash")) {
#ifdef Q_OS_UNIX
    ::raise(SIGKILL);
#endif
    return 4;
  }
  if (mode == QStringLiteral("hang")) {
    for (;;)
      ::pause();
  }
  if (mode == QStringLiteral("frame-hang")) {
    std::fputs("progress\n", stdout);
    std::fflush(stdout);
    for (;;)
      ::pause();
  }
  if (mode == QStringLiteral("tree")) {
#ifdef Q_OS_UNIX
    const pid_t child = ::fork();
    if (child < 0)
      return 5;
    if (child == 0) {
      for (;;)
        ::pause();
    }
    std::printf("%ld\n", static_cast<long>(child));
    std::fflush(stdout);
    for (;;)
      ::pause();
#endif
    return 6;
  }
  if (mode == QStringLiteral("sentinel")) {
#ifdef Q_OS_UNIX
    for (;;)
      ::pause();
#endif
    return 7;
  }
  return 2;
}

bool processIsGone(qint64 pid) {
#ifdef Q_OS_UNIX
  errno = 0;
  return ::kill(static_cast<pid_t>(pid), 0) == -1 && errno == ESRCH;
#else
  Q_UNUSED(pid);
  return true;
#endif
}
} // namespace

class CredentialProcessTests final : public QObject {
  Q_OBJECT
private slots:
  void helperSupervisor() {
    using Failure = CredentialProcess::Failure;
    CredentialProcess supervisor;
    const QByteArray request("{\"fixture\":true}\n");
    QVERIFY(CredentialProcessTestAccess::startMode(
                supervisor, {}, QStringLiteral("clean")) == 0);
    QVERIFY(CredentialProcessTestAccess::startMode(
                supervisor, QByteArray("one\ntwo\n"),
                QStringLiteral("clean")) == 0);
    QVERIFY(CredentialProcessTestAccess::startMode(
                supervisor, QByteArray(1024 * 1024 + 1, 'x') + '\n',
                QStringLiteral("clean")) == 0);

    quint64 previousToken = 0;
    auto runCase = [&](const QString &mode, Failure expected) {
      bool done = false;
      bool sawProgress = false;
      bool sawStarted = false;
      bool callbacksMatch = true;
      quint64 token = 0;
      const quint64 staleToken = previousToken;
      Failure result = Failure::None;
      supervisor.setEvents(
          {[&](quint64 callbackToken) {
             callbacksMatch = callbacksMatch && callbackToken == token;
             sawStarted = true;
           },
           [&](quint64 callbackToken, const QByteArray &frame) {
             callbacksMatch = callbacksMatch && callbackToken == token;
             if (frame == "progress")
               sawProgress = true;
             if (frame == "terminal")
               supervisor.acknowledgeTerminal(callbackToken);
             if (mode == QStringLiteral("tree")) {
               supervisor.acknowledgeTerminal(callbackToken);
               supervisor.cancel(callbackToken);
             }
           },
           [&](quint64 callbackToken, Failure failure, int) {
             callbacksMatch = callbacksMatch && callbackToken == token;
             result = failure;
             done = true;
           }});
      token = CredentialProcessTestAccess::startMode(supervisor, request, mode);
      QVERIFY(token != 0);
      supervisor.cancel(staleToken);
      supervisor.acknowledgeTerminal(staleToken);
      QCOMPARE(supervisor.activeToken(), token);
      QCOMPARE(CredentialProcessTestAccess::startMode(supervisor, request,
                                                      QStringLiteral("clean")),
               quint64(0));
      QTRY_VERIFY_WITH_TIMEOUT(done, 10'000);
      QVERIFY(callbacksMatch);
      QVERIFY(sawStarted);
      QCOMPARE(result, expected);
      if (mode == QStringLiteral("clean"))
        QVERIFY(sawProgress);
      QCOMPARE(supervisor.activeToken(), quint64(0));
      previousToken = token;
    };

    runCase(QStringLiteral("clean"), Failure::None);
    runCase(QStringLiteral("late"), Failure::LateOutput);
    runCase(QStringLiteral("truncated"), Failure::TruncatedFrame);
    runCase(QStringLiteral("line"), Failure::FrameTooLarge);
    runCase(QStringLiteral("total"), Failure::OutputTooLarge);
    runCase(QStringLiteral("stderr"), Failure::StderrTooLarge);
    runCase(QStringLiteral("crash"), Failure::ProcessFailure);

    QProcess sentinel;
    sentinel.setProgram(QCoreApplication::applicationFilePath());
    sentinel.setArguments(
        {QStringLiteral("--fixture-helper"), QStringLiteral("sentinel")});
    sentinel.start();
    QTRY_VERIFY_WITH_TIMEOUT(sentinel.state() == QProcess::Running, 5000);
    qint64 descendantPid = -1;
    bool done = false;
    bool callbacksMatch = true;
    Failure result = Failure::None;
    quint64 token = 0;
    supervisor.setEvents(
        {{},
         [&](quint64 callbackToken, const QByteArray &frame) {
           callbacksMatch = callbacksMatch && callbackToken == token;
           bool ok = false;
           descendantPid = frame.toLongLong(&ok);
           callbacksMatch = callbacksMatch && ok && descendantPid > 0;
           supervisor.cancel(callbackToken);
         },
         [&](quint64 callbackToken, Failure failure, int) {
           callbacksMatch = callbacksMatch && callbackToken == token;
           result = failure;
           done = true;
         }});
    token = CredentialProcessTestAccess::startMode(supervisor, request,
                                                   QStringLiteral("tree"));
    QVERIFY(token != 0);
    QTRY_VERIFY_WITH_TIMEOUT(done, 10'000);
    QVERIFY(callbacksMatch);
    QCOMPARE(result, Failure::Cancelled);
    QVERIFY(descendantPid > 0);
    QTRY_VERIFY_WITH_TIMEOUT(
        ::kill(static_cast<pid_t>(descendantPid), 0) == -1 && errno == ESRCH,
        5000);
    QCOMPARE(sentinel.state(), QProcess::Running);
    sentinel.kill();
    QVERIFY(sentinel.waitForFinished(5000));

    int callbacks = 0;
    qint64 childPid = 0;
    auto *startedOwner = new CredentialProcess;
    QPointer<CredentialProcess> startedGuard(startedOwner);
    startedOwner->setEvents({[&](quint64) {
                               ++callbacks;
                               childPid = CredentialProcessTestAccess::childPid(
                                   *startedOwner);
                               delete startedOwner;
                               startedOwner = nullptr;
                             },
                             [&](quint64, const QByteArray &) { ++callbacks; },
                             [&](quint64, Failure, int) { ++callbacks; }});
    QVERIFY(CredentialProcessTestAccess::startMode(
                *startedOwner, request, QStringLiteral("hang")) != 0);
    QTRY_VERIFY_WITH_TIMEOUT(startedGuard.isNull(), 5000);
    QVERIFY(childPid > 0);
    QTRY_VERIFY_WITH_TIMEOUT(processIsGone(childPid), 5000);
    QCOMPARE(callbacks, 1);

    callbacks = 0;
    childPid = 0;
    bool expectedFrame = false;
    auto *frameOwner = new CredentialProcess;
    QPointer<CredentialProcess> frameGuard(frameOwner);
    frameOwner->setEvents({[&](quint64) { ++callbacks; },
                           [&](quint64, const QByteArray &frame) {
                             ++callbacks;
                             expectedFrame = frame == "progress";
                             childPid = CredentialProcessTestAccess::childPid(
                                 *frameOwner);
                             delete frameOwner;
                             frameOwner = nullptr;
                           },
                           [&](quint64, Failure, int) { ++callbacks; }});
    QVERIFY(CredentialProcessTestAccess::startMode(
                *frameOwner, request, QStringLiteral("frame-hang")) != 0);
    QTRY_VERIFY_WITH_TIMEOUT(frameGuard.isNull(), 5000);
    QVERIFY(childPid > 0);
    QTRY_VERIFY_WITH_TIMEOUT(processIsGone(childPid), 5000);
    QCOMPARE(callbacks, 2);
    QVERIFY(expectedFrame);

    callbacks = 0;
    Failure finishFailure = Failure::ProcessFailure;
    auto *finishedOwner = new CredentialProcess;
    QPointer<CredentialProcess> finishedGuard(finishedOwner);
    finishedOwner->setEvents({[&](quint64) { ++callbacks; },
                              [&](quint64, const QByteArray &) { ++callbacks; },
                              [&](quint64, Failure failure, int) {
                                ++callbacks;
                                finishFailure = failure;
                                delete finishedOwner;
                                finishedOwner = nullptr;
                              }});
    QVERIFY(CredentialProcessTestAccess::startMode(
                *finishedOwner, request, QStringLiteral("clean")) != 0);
    QTRY_VERIFY_WITH_TIMEOUT(finishedGuard.isNull(), 5000);
    QCOMPARE(callbacks, 4);
    QCOMPARE(finishFailure, Failure::None);
  }
};

int main(int argc, char **argv) {
  QCoreApplication app(argc, argv);
  const QStringList arguments = app.arguments();
  if (arguments.size() == 3 && arguments.at(1) == "--fixture-helper")
    return helperMain(arguments.at(2));
  CredentialProcessTests tests;
  return QTest::qExec(&tests, argc, argv);
}

#include "tst_credential_process.moc"
