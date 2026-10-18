#include "../../src/portal_file_request.h"
#include <QCoreApplication>
#include <QDBusArgument>
#include <QDBusConnectionInterface>
#include <QDBusInterface>
#include <QDBusReply>
#include <QSignalSpy>
#include <QtTest>

class PortalActivationTests final : public QObject {
  Q_OBJECT
private slots:
  void activatesInstalledServiceBeforeSendingIntent() {
    QCOMPARE(qEnvironmentVariable("FERRIC_PORTAL_TEST_BUS"),
             QString("isolated"));
    const auto bus = QDBusConnection::sessionBus();
    QDBusReply<bool> running =
        bus.interface()->isServiceRegistered("org.freedesktop.portal.Desktop");
    QVERIFY(running.isValid());
    QVERIFY(!running.value());
    QPointer<FerricPortalFileRequest> actor = new FerricPortalFileRequest({});
    QSignalSpy opened(actor, &FerricPortalFileRequest::opened);
    QSignalSpy finished(actor, &FerricPortalFileRequest::finished);
    actor->start();
    QTRY_COMPARE(opened.size(), 1);
    QDBusInterface control("org.freedesktop.portal.Desktop",
                           "/org/ferricbrowser/PortalFixture",
                           "org.ferricbrowser.PortalFixture", bus);
    QVERIFY(control.isValid());
    QDBusReply<QVariantList> requests = control.call("Requests");
    QVERIFY(requests.isValid());
    QCOMPARE(requests.value().size(), 1);
    const auto request = qdbus_cast<QVariantMap>(requests.value().first());
    const auto reply = control.call(
        "Respond", request["path"].toString(), uint(0),
        QVariantMap{{"uris", QStringList{"file:///tmp/activated.txt"}}});
    QVERIFY(reply.type() != QDBusMessage::ErrorMessage);
    QTRY_COMPARE(finished.size(), 1);
    QCOMPARE(finished.first().first().toInt(),
             int(FerricPortalFileRequest::Accepted));
    QTRY_VERIFY(actor.isNull());
  }
};
QTEST_GUILESS_MAIN(PortalActivationTests)
#include "tst_portal_activation.moc"
