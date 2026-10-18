#include "../../src/gui_application.h"
#include "../../src/native_file_chooser.h"
#include <QApplication>
#include <QDBusArgument>
#include <QDBusConnectionInterface>
#include <QDBusInterface>
#include <QDBusReply>
#include <QFile>
#include <QSignalSpy>
#include <QtTest>

class PortalFileChooserTests final : public QObject {
  Q_OBJECT
  std::unique_ptr<QDBusInterface> control;
  QWindow owner;

  QVariantList requests() {
    QDBusReply<QVariantList> reply = control->call("Requests");
    if (!reply.isValid())
      qFatal("Owned portal fixture is unavailable");
    return reply.value();
  }
  QVariantMap row(int index) {
    return qdbus_cast<QVariantMap>(requests().at(index));
  }
  void configure(const QVariantMap &settings = {}) {
    const auto reply = control->call("Configure", settings);
    QVERIFY(reply.type() != QDBusMessage::ErrorMessage);
  }
  void respond(int index, uint code, const QStringList &uris = {}) {
    const auto reply = control->call("Respond", row(index)["path"].toString(),
                                     code, QVariantMap{{"uris", uris}});
    QVERIFY(reply.type() != QDBusMessage::ErrorMessage);
  }
  void open(FerricNativeFileChooser &chooser, bool required = true) {
    chooser.setParentWindow(&owner);
    chooser.setTitle("Choose a fixture file");
    chooser.setPortalRequired(required);
    chooser.open();
    QVERIFY(chooser.visible());
  }
  bool hasWidget() {
    for (auto widget : QApplication::topLevelWidgets())
      if (qobject_cast<QFileDialog *>(widget))
        return true;
    return false;
  }
private slots:
  void initTestCase() {
    QCOMPARE(qEnvironmentVariable("FERRIC_PORTAL_TEST_BUS"),
             QString("isolated"));
    QCOMPARE(QGuiApplication::platformName(), QString("wayland"));
    QDBusReply<QString> service =
        QDBusConnection::sessionBus().interface()->serviceOwner(
            "org.freedesktop.portal.Desktop");
    QVERIFY(service.isValid());
    control = std::make_unique<QDBusInterface>(
        service.value(), "/org/ferricbrowser/PortalFixture",
        "org.ferricbrowser.PortalFixture", QDBusConnection::sessionBus());
    QVERIFY(control->isValid());
    owner.setTitle("Actual native chooser owner");
    owner.resize(640, 480);
    owner.show();
    QVERIFY(QTest::qWaitForWindowExposed(&owner));
  }
  void init() { configure(); }
  void cleanup() {
    const auto reply = control->call("RestoreName");
    QVERIFY(reply.type() != QDBusMessage::ErrorMessage);
    for (auto actor : QCoreApplication::instance()
                          ->findChildren<FerricPortalFileRequest *>())
      actor->cancel();
    QTRY_VERIFY(QCoreApplication::instance()
                    ->findChildren<FerricPortalFileRequest *>()
                    .isEmpty());
    QTRY_VERIFY(!hasWidget());
  }
  void accepted_data() {
    QTest::addColumn<int>("mode");
    QTest::newRow("single") << int(FerricNativeFileChooser::OpenFile);
    QTest::newRow("multiple") << int(FerricNativeFileChooser::OpenFiles);
    QTest::newRow("save") << int(FerricNativeFileChooser::SaveFile);
    QTest::newRow("folder") << int(FerricNativeFileChooser::Folder);
  }
  void accepted() {
    QFETCH(int, mode);
    const int index = requests().size();
    FerricNativeFileChooser chooser;
    chooser.setFileMode(mode);
    chooser.setNameFilters({"Text files (*.txt *.md)", "All files (*)"});
    QSignalSpy accepted(&chooser, &FerricNativeFileChooser::accepted);
    QSignalSpy rejected(&chooser, &FerricNativeFileChooser::rejected);
    QSignalSpy failed(&chooser, &FerricNativeFileChooser::failed);
    open(chooser);
    QTRY_COMPARE(chooser.backend(), QString("system"));
    QCOMPARE(requests().size(), index + 1);
    const auto record = row(index);
    QCOMPARE(record["method"].toString(),
             mode == FerricNativeFileChooser::SaveFile ? QString("SaveFile")
                                                       : QString("OpenFile"));
    const auto options = qdbus_cast<QVariantMap>(record["options"]);
    QCOMPARE(options["multiple"].toBool(),
             mode == FerricNativeFileChooser::OpenFiles);
    QCOMPARE(options["directory"].toBool(),
             mode == FerricNativeFileChooser::Folder);
    QDBusInterface compositor(
        "org.ferricbrowser.ParentFixture", "/org/ferricbrowser/ParentFixture",
        "org.ferricbrowser.ParentFixture", QDBusConnection::sessionBus());
    QDBusReply<QVariantList> exported = compositor.call("Exports");
    QVERIFY(exported.isValid());
    bool matched = false;
    for (const auto &item : exported.value()) {
      const auto parent = qdbus_cast<QVariantMap>(item);
      if (record["parent"].toString() !=
          "wayland:" + parent["handle"].toString())
        continue;
      QCOMPARE(parent["title"].toString(), owner.title());
      QCOMPARE(parent["pid"].toLongLong(), QCoreApplication::applicationPid());
      matched = true;
    }
    QVERIFY(matched);
    const auto prefix =
        qEnvironmentVariable("FERRIC_TEST_WINDOW_SYSTEM_PREFIX");
    if (!prefix.isEmpty()) {
      QFile maps(QStringLiteral("/proc/self/maps"));
      QVERIFY(maps.open(QIODevice::ReadOnly));
      const auto loaded = maps.readAll();
      QVERIFY(loaded.contains(
          (prefix + "/lib/libKF6WindowSystem.so.6.28.0").toUtf8()));
      QVERIFY(loaded.contains((prefix + "/lib/qt6/plugins/kf6/kwindowsystem/"
                                        "KF6WindowSystemKWaylandPlugin.so")
                                  .toUtf8()));
    }
    const auto literal =
        QUrl::fromLocalFile(QString::fromUtf8("/tmp/résumé + %2F#?.txt"));
    QStringList uris{literal.toString(QUrl::FullyEncoded)};
    QList<QUrl> expected{literal};
    if (mode == FerricNativeFileChooser::OpenFiles) {
      uris << "file:///tmp/another.txt";
      expected << QUrl(uris.last());
    }
    respond(index, 0, uris);
    QTRY_COMPARE(accepted.size(), 1);
    QCOMPARE(chooser.selectedFiles(), expected);
    QCOMPARE(rejected.size(), 0);
    QCOMPARE(failed.size(), 0);
    QVERIFY(!chooser.visible());
    QVERIFY(!hasWidget());
  }
  void invalidSelection_data() {
    QTest::addColumn<QStringList>("uris");
    QTest::newRow("remote") << QStringList{"smb://server/file.txt"};
    QTest::newRow("mixed") << QStringList{"file:///tmp/good.txt",
                                          "https://example.test/file"};
    QTest::newRow("empty") << QStringList{};
    QTest::newRow("invalid-encoding") << QStringList{"file:///tmp/bad%00file"};
    QTest::newRow("query") << QStringList{"file:///tmp/file?secret"};
    QTest::newRow("authority") << QStringList{"file://remote/tmp/file"};
  }
  void invalidSelection() {
    QFETCH(QStringList, uris);
    const int index = requests().size();
    FerricNativeFileChooser chooser;
    chooser.setFileMode(FerricNativeFileChooser::OpenFiles);
    QSignalSpy accepted(&chooser, &FerricNativeFileChooser::accepted);
    QSignalSpy rejected(&chooser, &FerricNativeFileChooser::rejected);
    QSignalSpy failed(&chooser, &FerricNativeFileChooser::failed);
    open(chooser);
    QTRY_COMPARE(chooser.backend(), QString("system"));
    respond(index, 0, uris);
    QTRY_COMPARE(failed.size(), 1);
    QVERIFY(!failed.first().first().toString().isEmpty());
    QVERIFY(!failed.first().first().toString().contains("org.freedesktop"));
    QVERIFY(chooser.selectedFiles().isEmpty());
    QCOMPARE(accepted.size(), 0);
    QCOMPARE(rejected.size(), 0);
    QVERIFY(!hasWidget());
  }
  void userCancellationIsNotFailure() {
    const int index = requests().size();
    FerricNativeFileChooser chooser;
    QSignalSpy rejected(&chooser, &FerricNativeFileChooser::rejected);
    QSignalSpy failed(&chooser, &FerricNativeFileChooser::failed);
    open(chooser);
    QTRY_COMPARE(chooser.backend(), QString("system"));
    respond(index, 1);
    QTRY_COMPARE(rejected.size(), 1);
    QCOMPARE(failed.size(), 0);
  }
  void lateReplyCannotAffectReplacement() {
    configure({{"hold", true}});
    const int oldIndex = requests().size();
    FerricNativeFileChooser old;
    QSignalSpy rejected(&old, &FerricNativeFileChooser::rejected);
    QSignalSpy failed(&old, &FerricNativeFileChooser::failed);
    open(old);
    QTRY_COMPARE(requests().size(), oldIndex + 1);
    old.close();
    QTRY_COMPARE(rejected.size(), 1);
    QTRY_VERIFY(row(oldIndex)["close_count"].toInt() >= 1);
    configure();
    const int newIndex = requests().size();
    FerricNativeFileChooser current;
    QSignalSpy accepted(&current, &FerricNativeFileChooser::accepted);
    open(current);
    QTRY_COMPARE(current.backend(), QString("system"));
    QVERIFY(row(oldIndex)["sender"] != row(newIndex)["sender"]);
    const auto released =
        control->call("Release", row(oldIndex)["path"].toString(), false);
    QVERIFY(released.type() != QDBusMessage::ErrorMessage);
    respond(oldIndex, 0, {"file:///tmp/stale.txt"});
    respond(newIndex, 0, {"file:///tmp/current.txt"});
    QTRY_COMPARE(accepted.size(), 1);
    QCOMPARE(current.selectedFile(), QUrl("file:///tmp/current.txt"));
    QCOMPARE(rejected.size(), 1);
    QCOMPARE(failed.size(), 0);
  }
  void ownerAndControllerDestruction_data() {
    QTest::addColumn<bool>("destroyOwner");
    QTest::newRow("owner") << true;
    QTest::newRow("controller") << false;
  }
  void ownerAndControllerDestruction() {
    QFETCH(bool, destroyOwner);
    configure({{"hold", true}});
    const int index = requests().size();
    auto host = std::make_unique<QWindow>();
    host->resize(320, 240);
    host->show();
    QVERIFY(QTest::qWaitForWindowExposed(host.get()));
    auto chooser = std::make_unique<FerricNativeFileChooser>();
    chooser->setParentWindow(host.get());
    chooser->setPortalRequired(true);
    chooser->open();
    QTRY_COMPARE(requests().size(), index + 1);
    if (destroyOwner)
      host.reset();
    else
      chooser.reset();
    QTRY_VERIFY(row(index)["close_count"].toInt() >= 1);
    const auto released =
        control->call("Release", row(index)["path"].toString(), false);
    QVERIFY(released.type() != QDBusMessage::ErrorMessage);
    QTRY_VERIFY(row(index)["caller_gone"].toBool());
    if (chooser)
      QVERIFY(!chooser->visible());
  }
  void closeDuringParentExportDoesNotOpenPicker_data() {
    QTest::addColumn<bool>("explicitClose");
    QTest::newRow("caller-close") << true;
    QTest::newRow("owner-close") << false;
  }
  void closeDuringParentExportDoesNotOpenPicker() {
    QFETCH(bool, explicitClose);
    const int count = requests().size();
    auto host = std::make_unique<QWindow>();
    host->resize(320, 240);
    host->show();
    QVERIFY(QTest::qWaitForWindowExposed(host.get()));
    FerricNativeFileChooser chooser;
    chooser.setParentWindow(host.get());
    chooser.setPortalRequired(true);
    QSignalSpy rejected(&chooser, &FerricNativeFileChooser::rejected);
    chooser.open();
    if (explicitClose)
      chooser.close();
    host.reset();
    QTRY_COMPARE(rejected.size(), 1);
    QTest::qWait(100);
    QCOMPARE(requests().size(), count);
    QVERIFY(!hasWidget());
  }
  void changedConfigurationDoesNotRetargetOutstandingRequest() {
    const int index = requests().size();
    FerricNativeFileChooser chooser;
    chooser.setFileMode(FerricNativeFileChooser::OpenFiles);
    QSignalSpy accepted(&chooser, &FerricNativeFileChooser::accepted);
    QSignalSpy failed(&chooser, &FerricNativeFileChooser::failed);
    open(chooser);
    chooser.setFileMode(FerricNativeFileChooser::SaveFile);
    chooser.setTitle("Title for the next request");
    QTRY_COMPARE(chooser.backend(), QString("system"));
    QCOMPARE(row(index)["method"].toString(), QString("OpenFile"));
    QCOMPARE(row(index)["title"].toString(), QString("Choose a fixture file"));
    respond(index, 0, {"file:///tmp/first.txt", "file:///tmp/second.txt"});
    QTRY_COMPARE(accepted.size(), 1);
    QCOMPARE(chooser.selectedFiles().size(), 2);
    QCOMPARE(failed.size(), 0);
  }
  void reopeningDuringCompletionCannotReceiveOldResult() {
    const int index = requests().size();
    FerricNativeFileChooser chooser;
    QSignalSpy accepted(&chooser, &FerricNativeFileChooser::accepted);
    bool reopened = false;
    connect(&chooser, &FerricNativeFileChooser::currentFolderChanged, &chooser,
            [&] {
              if (chooser.visible() || reopened)
                return;
              reopened = true;
              chooser.open();
            });
    open(chooser);
    QTRY_COMPARE(chooser.backend(), QString("system"));
    respond(index, 0, {"file:///tmp/previous.txt"});
    QTRY_COMPARE(requests().size(), index + 2);
    QTRY_COMPARE(chooser.backend(), QString("system"));
    QVERIFY(reopened);
    QCOMPARE(accepted.size(), 0);
    respond(index + 1, 0, {"file:///tmp/current.txt"});
    QTRY_COMPARE(accepted.size(), 1);
    QCOMPARE(chooser.selectedFile(), QUrl("file:///tmp/current.txt"));
  }
  void unavailableCapability_data() {
    QTest::addColumn<bool>("required");
    QTest::newRow("required") << true;
    QTest::newRow("auto") << false;
  }
  void unavailableCapability() {
    QFETCH(bool, required);
    configure({{"version", 1}});
    const int count = requests().size();
    FerricNativeFileChooser chooser;
    chooser.setFileMode(FerricNativeFileChooser::Folder);
    QSignalSpy failed(&chooser, &FerricNativeFileChooser::failed);
    QSignalSpy rejected(&chooser, &FerricNativeFileChooser::rejected);
    open(chooser, required);
    if (required) {
      QTRY_COMPARE(failed.size(), 1);
      QCOMPARE(rejected.size(), 0);
      QVERIFY(!hasWidget());
    } else {
      QTRY_COMPARE(chooser.backend(), QString("browser"));
      QVERIFY(hasWidget());
      for (auto widget : QApplication::topLevelWidgets())
        if (auto dialog = qobject_cast<QFileDialog *>(widget))
          QVERIFY(dialog->testOption(QFileDialog::DontUseNativeDialog));
      chooser.close();
      QTRY_COMPARE(rejected.size(), 1);
      QCOMPARE(failed.size(), 0);
    }
    QCOMPARE(requests().size(), count);
  }
  void lostServiceDoesNotOpenFallback() {
    const int index = requests().size();
    FerricNativeFileChooser chooser;
    QSignalSpy failed(&chooser, &FerricNativeFileChooser::failed);
    QSignalSpy rejected(&chooser, &FerricNativeFileChooser::rejected);
    open(chooser, false);
    QTRY_COMPARE(chooser.backend(), QString("system"));
    const auto reply = control->call("DropName");
    QVERIFY(reply.type() != QDBusMessage::ErrorMessage);
    QTRY_COMPARE(failed.size(), 1);
    QTRY_VERIFY(row(index)["close_count"].toInt() >= 1);
    QVERIFY(!hasWidget());
    QCOMPARE(rejected.size(), 0);
    QCOMPARE(requests().size(), index + 1);
  }
};

int main(int argc, char **argv) {
  QVector<QByteArray> arguments;
  for (int index = 0; index < argc; ++index)
    arguments.append(QByteArray(argv[index]));
  auto application = ferric_browser_gui_application(arguments);
  PortalFileChooserTests tests;
  return QTest::qExec(&tests, argc, argv);
}
#include "tst_portal_file_chooser.moc"
