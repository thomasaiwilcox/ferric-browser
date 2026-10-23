#pragma once
#include <QApplication>
#include <QDBusInterface>
#include <QDBusReply>
#include <QGuiApplication>
#include <QKeyEvent>
#include <QQuickWindow>
#include <QtTest>

// Drive only owned surfaces and observe native delivery when a seat is present.
class OwnedBrowserKeyDelivery final : public QObject {
  public:
    explicit OwnedBrowserKeyDelivery(QQuickWindow *host, int key) : QObject(host), host(host), key(key) {
        qApp->installEventFilter(this);
    }
    int overrides = 0, presses = 0, releases = 0, repeats = 0;
    QStringList observations;
    quint32 scanCode = 0;
    quint32 releaseScanCode = 0;
    Qt::KeyboardModifiers modifiers = Qt::NoModifier;
  private:
    QQuickWindow *host;
    int key;
    bool eventFilter(QObject *watched, QEvent *event) override {
        if (watched == host && (event->type() == QEvent::ShortcutOverride
                || event->type() == QEvent::KeyPress || event->type() == QEvent::KeyRelease)) {
            const auto input = static_cast<QKeyEvent *>(event);
            observations.append(QString("type=%1 key=%2 mods=%3 scan=%4 text=%5").arg(event->type()).arg(input->key()).arg(int(input->modifiers())).arg(input->nativeScanCode()).arg(input->text()));
            if (input->key() != key) return false;
            // Search activation sends an internal Enter after the native find
            // handoff. It is not another physical press/release from the seat.
            if (input->nativeScanCode() == 0) return false;
            if (event->type() == QEvent::ShortcutOverride) {
                ++overrides; scanCode = input->nativeScanCode(); modifiers = input->modifiers();
            } else if (event->type() == QEvent::KeyPress) ++presses;
            else { ++releases; releaseScanCode = input->nativeScanCode(); }
            if (input->isAutoRepeat()) ++repeats;
        }
        return false;
    }
};

inline void sendOwnedBrowserKey(QQuickWindow *host, int value, Qt::KeyboardModifiers modifiers = Qt::NoModifier, const QString &physicalKey = {}) {
    if (!QGuiApplication::platformName().startsWith("wayland")
            || qEnvironmentVariable("FERRIC_PORTAL_TEST_BUS") != "isolated") {
        QTest::keyClick(host, Qt::Key(value), modifiers); QTest::qWait(30); return;
    }
    QDBusInterface compositor("org.ferricbrowser.ParentFixture", "/org/ferricbrowser/ParentFixture",
        "org.ferricbrowser.ParentFixture", QDBusConnection::sessionBus());
    QVERIFY(compositor.isValid());
    QString windowId;
    QTRY_VERIFY(([&] {
        QDBusReply<QVariantList> reply = compositor.call("Windows");
        if (!reply.isValid()) return false;
        QStringList candidates;
        for (const auto &item : reply.value()) {
            const auto candidate = qdbus_cast<QVariantMap>(item);
            if (candidate["title"].toString() == host->title()
                    && candidate["pid"].toLongLong() == QCoreApplication::applicationPid()
                    && candidate["mapped"].toBool()) {
                candidates.append(candidate["id"].toString());
            }
        }
        // Preserve the application's title binding, and reject ambiguous
        // ownership instead of renaming a production window for this fixture.
        if (candidates.size()!=1) return false;
        windowId=candidates.constFirst();
        return !windowId.isEmpty();
    })());
    OwnedBrowserKeyDelivery delivery(host, value);
    QDBusReply<bool> sent = physicalKey.isEmpty()
        ? compositor.call("Key", windowId, value, int(modifiers))
        : compositor.call("KeyPosition", windowId, physicalKey, int(modifiers));
    QVERIFY(sent.isValid() && sent.value());
    QTRY_VERIFY2(delivery.releases == 1, qPrintable(delivery.observations.join("; ")));
    // Qt's shortcut map consumes matching presses before window delivery.
    // ShortcutOverride still carries the seat's native press identity.
    QCOMPARE(delivery.overrides, 1);
    QVERIFY(delivery.presses <= 1);
    QCOMPARE(delivery.repeats, 0);
    QCOMPARE(delivery.modifiers & ~Qt::KeypadModifier, modifiers);
    QVERIFY(delivery.scanCode != 0);
    QCOMPARE(delivery.releaseScanCode, delivery.scanCode);
    QTRY_COMPARE(QGuiApplication::queryKeyboardModifiers(), Qt::NoModifier);
}

