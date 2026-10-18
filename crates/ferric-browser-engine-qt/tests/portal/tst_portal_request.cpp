#include "../../src/portal_file_request.h"
#include <QCoreApplication>
#include <QDBusArgument>
#include <QDBusConnectionInterface>
#include <QDBusInterface>
#include <QDBusPendingCallWatcher>
#include <QDBusReply>
#include <QPointer>
#include <QSignalSpy>
#include <QtTest>

class Tests final : public QObject {
  Q_OBJECT
  std::unique_ptr<QDBusInterface> control;
  QVariantList requests() {
    QDBusReply<QVariantList> reply = control->call("Requests");
    if (!reply.isValid())
      qFatal("owned requests failed");
    return reply.value();
  }
  QVariantMap row(int index) {
    return qdbus_cast<QVariantMap>(requests().at(index));
  }
  void configure(const QVariantMap &settings = {}) {
    auto reply = control->call("Configure", settings);
    if (reply.type() == QDBusMessage::ErrorMessage)
      qFatal("owned configure failed");
  }
  void respond(const QString &path, uint code, const QStringList &uris = {}) {
    auto reply =
        control->call("Respond", path, code, QVariantMap{{"uris", uris}});
    if (reply.type() == QDBusMessage::ErrorMessage)
      qFatal("owned response failed");
  }
  void release(const QString &path, bool error = false) {
    auto reply = control->call("Release", path, error);
    if (reply.type() == QDBusMessage::ErrorMessage)
      qFatal("owned release failed");
  }
  FerricPortalFileRequest::Options options(
      FerricPortalFileRequest::Mode mode = FerricPortalFileRequest::OpenFile) {
    return {mode,
            "wayland:owned-parent",
            "Owned file choice",
            {"Text files (*.txt)", "All files (*)"},
            QUrl::fromLocalFile("/tmp/owned folder"),
            QUrl::fromLocalFile("/tmp/-owned result.txt")};
  }
  bool replyProcessed(const QPointer<FerricPortalFileRequest> &request) {
    return request &&
           request->findChildren<QDBusPendingCallWatcher *>().isEmpty();
  }
private slots:
  void initTestCase() {
    QVERIFY(qEnvironmentVariable("FERRIC_PORTAL_TEST_BUS") == "isolated");
    QDBusReply<QString> owner =
        QDBusConnection::sessionBus().interface()->serviceOwner(
            "org.freedesktop.portal.Desktop");
    QVERIFY(owner.isValid());
    control = std::make_unique<QDBusInterface>(
        owner.value(), "/org/ferricbrowser/PortalFixture",
        "org.ferricbrowser.PortalFixture", QDBusConnection::sessionBus());
    QVERIFY(control->isValid());
  }
  void init() { configure(); }
  void cleanup() {
    const auto reply = control->call("RestoreName");
    QVERIFY(reply.type() != QDBusMessage::ErrorMessage);
    for (auto request : QCoreApplication::instance()
                            ->findChildren<FerricPortalFileRequest *>())
      request->cancel();
    QTRY_VERIFY(QCoreApplication::instance()
                    ->findChildren<FerricPortalFileRequest *>()
                    .isEmpty());
  }
  void accepted_data() {
    QTest::addColumn<int>("mode");
    for (int mode = 0; mode < 4; ++mode)
      QTest::newRow(qPrintable(QString::number(mode))) << mode;
  }
  void accepted() {
    QFETCH(int, mode);
    const int index = requests().size();
    QPointer<FerricPortalFileRequest> request = new FerricPortalFileRequest(
        options(static_cast<FerricPortalFileRequest::Mode>(mode)));
    QSignalSpy done(request, &FerricPortalFileRequest::finished);
    request->start();
    QTRY_COMPARE(requests().size(), index + 1);
    QTRY_VERIFY(replyProcessed(request));
    const auto record = row(index);
    QCOMPARE(record["parent"].toString(), QString("wayland:owned-parent"));
    QCOMPARE(record["method"].toString(),
             mode == 2 ? QString("SaveFile") : QString("OpenFile"));
    const auto values = qdbus_cast<QVariantMap>(record["options"]);
    QCOMPARE(values["multiple"].toBool(), mode == 1);
    QCOMPARE(values["directory"].toBool(), mode == 3);
    QCOMPARE(values["current_folder"].toByteArray(),
             QByteArray("/tmp/owned folder\0", 18));
    const auto filters = values["filters"].value<QDBusArgument>();
    QStringList labels, patterns;
    filters.beginArray();
    while (!filters.atEnd()) {
      filters.beginStructure();
      QString label;
      filters >> label;
      labels.append(label);
      filters.beginArray();
      while (!filters.atEnd()) {
        filters.beginStructure();
        uint type;
        QString pattern;
        filters >> type >> pattern;
        QCOMPARE(type, uint(0));
        patterns.append(pattern);
        filters.endStructure();
      }
      filters.endArray();
      filters.endStructure();
    }
    filters.endArray();
    QCOMPARE(labels, (QStringList{"Text files", "All files"}));
    QCOMPARE(patterns, (QStringList{"*.txt", "*"}));
    if (mode == FerricPortalFileRequest::SaveFile)
      QCOMPARE(values["current_name"].toString(), QString("-owned result.txt"));
    QStringList uris{"file:///tmp/owned%20file.txt"};
    if (mode == 1)
      uris << "file:///tmp/another.txt";
    respond(record["path"].toString(), 0, uris);
    QTRY_COMPARE(done.count(), 1);
    QCOMPARE(done.at(0).at(0).toInt(), int(FerricPortalFileRequest::Accepted));
    QCOMPARE(done.at(0).at(1).toStringList(), uris);
    QTRY_VERIFY(request.isNull());
    QTRY_VERIFY(row(index)["caller_gone"].toBool());
  }
  void earlyResponse() {
    configure({{"early", true}});
    const int index = requests().size();
    QPointer<FerricPortalFileRequest> request =
        new FerricPortalFileRequest(options());
    QSignalSpy done(request, &FerricPortalFileRequest::finished);
    request->start();
    QTRY_COMPARE(done.count(), 1);
    QCOMPARE(done.at(0).at(0).toInt(), int(FerricPortalFileRequest::Accepted));
    QCOMPARE(done.at(0).at(1).toStringList(),
             QStringList{"file:///tmp/owned-early.txt"});
    QTRY_VERIFY(request.isNull());
    QTRY_VERIFY(row(index)["caller_gone"].toBool());
  }
  void alternateHandle() {
    configure({{"alternate", true}});
    const int index = requests().size();
    QPointer<FerricPortalFileRequest> request =
        new FerricPortalFileRequest(options());
    QSignalSpy done(request, &FerricPortalFileRequest::finished);
    request->start();
    QTRY_COMPARE(requests().size(), index + 1);
    QTRY_VERIFY(replyProcessed(request));
    respond(row(index)["path"].toString(), 0,
            {"file:///tmp/owned-alternate.txt"});
    QTRY_COMPARE(done.count(), 1);
    QCOMPARE(done.at(0).at(0).toInt(), int(FerricPortalFileRequest::Accepted));
    QTRY_VERIFY(request.isNull());
  }
  void cancel_data() {
    QTest::addColumn<bool>("hold");
    QTest::addColumn<bool>("error");
    QTest::newRow("after-reply") << false << false;
    QTest::newRow("before-reply") << true << false;
    QTest::newRow("late-error") << true << true;
  }
  void cancel() {
    QFETCH(bool, hold);
    QFETCH(bool, error);
    configure({{"hold", hold}});
    const int index = requests().size();
    QPointer<FerricPortalFileRequest> request =
        new FerricPortalFileRequest(options());
    QSignalSpy done(request, &FerricPortalFileRequest::finished);
    request->start();
    QTRY_COMPARE(requests().size(), index + 1);
    if (!hold)
      QTRY_VERIFY(replyProcessed(request));
    const auto path = row(index)["path"].toString();
    request->cancel();
    QTRY_COMPARE(done.count(), 1);
    QCOMPARE(done.at(0).at(0).toInt(), int(FerricPortalFileRequest::Cancelled));
    QTRY_VERIFY(row(index)["close_count"].toInt() >= 1);
    if (hold)
      release(path, error);
    QTRY_VERIFY(request.isNull());
    QTRY_VERIFY(row(index)["caller_gone"].toBool());
    QCOMPARE(done.count(), 1);
  }
  void distinctRequestsRemainIndependent() {
    configure({{"hold", true}});
    int first = requests().size();
    QPointer<FerricPortalFileRequest> old =
        new FerricPortalFileRequest(options());
    QSignalSpy oldDone(old, &FerricPortalFileRequest::finished);
    old->start();
    QTRY_COMPARE(requests().size(), first + 1);
    configure();
    int second = requests().size();
    QPointer<FerricPortalFileRequest> current =
        new FerricPortalFileRequest(options());
    QSignalSpy currentDone(current, &FerricPortalFileRequest::finished);
    current->start();
    QTRY_COMPARE(requests().size(), second + 1);
    QTRY_VERIFY(replyProcessed(current));
    QVERIFY(row(first)["sender"] != row(second)["sender"]);
    old->cancel();
    release(row(first)["path"].toString());
    QTRY_VERIFY(old.isNull());
    QCOMPARE(oldDone.count(), 1);
    QCOMPARE(currentDone.count(), 0);
    respond(row(second)["path"].toString(), 0, {"file:///tmp/owned-new.txt"});
    QTRY_COMPARE(currentDone.count(), 1);
    QCOMPARE(currentDone.at(0).at(0).toInt(),
             int(FerricPortalFileRequest::Accepted));
    QTRY_VERIFY(current.isNull());
  }
  void cancellingBeforeStartDoesNotOpenARequest() {
    const int count = requests().size();
    QPointer<FerricPortalFileRequest> request =
        new FerricPortalFileRequest(options());
    QSignalSpy done(request, &FerricPortalFileRequest::finished);
    request->cancel();
    request->start();
    QTRY_COMPARE(done.count(), 1);
    QCOMPARE(done.at(0).at(0).toInt(), int(FerricPortalFileRequest::Cancelled));
    QTRY_VERIFY(request.isNull());
    QCOMPARE(requests().size(), count);
  }
  void losingPortalServiceNameClosesTheRequest() {
    const int index = requests().size();
    QPointer<FerricPortalFileRequest> request =
        new FerricPortalFileRequest(options());
    QSignalSpy done(request, &FerricPortalFileRequest::finished);
    request->start();
    QTRY_COMPARE(requests().size(), index + 1);
    QTRY_VERIFY(replyProcessed(request));
    const auto reply = control->call("DropName");
    QVERIFY(reply.type() != QDBusMessage::ErrorMessage);
    QTRY_COMPARE_WITH_TIMEOUT(done.count(), 1, 500);
    QCOMPARE(done.at(0).at(0).toInt(), int(FerricPortalFileRequest::Failed));
    QTRY_VERIFY(row(index)["close_count"].toInt() >= 1);
    QTRY_VERIFY(request.isNull());
  }
  void unavailableCapabilityDoesNotOpenARequest() {
    configure({{"version", 1}});
    const int count = requests().size();
    QPointer<FerricPortalFileRequest> request =
        new FerricPortalFileRequest(options(FerricPortalFileRequest::Folder));
    QSignalSpy done(request, &FerricPortalFileRequest::finished);
    request->start();
    QTRY_COMPARE(done.count(), 1);
    QCOMPARE(done.at(0).at(0).toInt(),
             int(FerricPortalFileRequest::Unavailable));
    QCOMPARE(requests().size(), count);
    QTRY_VERIFY(request.isNull());
  }
};
QTEST_GUILESS_MAIN(Tests)
#include "tst_portal_request.moc"
