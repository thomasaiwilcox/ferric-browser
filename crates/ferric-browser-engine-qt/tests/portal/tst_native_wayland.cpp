#include "../../src/native_wayland.h"
#include <QApplication>
#include <QDBusArgument>
#include <QDBusInterface>
#include <QDBusReply>
#include <QPainter>
#include <QRasterWindow>
#include <QSignalSpy>
#include <QtTest>

class Window final : public QRasterWindow {
    void paintEvent(QPaintEvent *) override {
        QPainter painter(this);
        painter.fillRect(QRect(QPoint(), size()), Qt::blue);
    }
};
class NativeTests final : public QObject {
    Q_OBJECT
    QDBusInterface control{"org.ferricbrowser.ParentFixture", "/org/ferricbrowser/ParentFixture",
                           "org.ferricbrowser.ParentFixture", QDBusConnection::sessionBus()};
    QVariantList records(const char *method) {
        QDBusReply<QVariantList> reply = control.call(method);
        if (!reply.isValid()) qFatal("Owned compositor control unavailable");
        return reply.value();
    }
    bool tags() const { return !qEnvironmentVariableIsSet("FERRIC_FIXTURE_NO_TAGS"); }
    bool exporter() const { return !qEnvironmentVariableIsSet("FERRIC_FIXTURE_NO_EXPORTER"); }
    QVariantMap resources() {
        QDBusReply<QVariantMap> result = control.call("Resources");
        if (!result.isValid()) qFatal("Owned compositor resources unavailable");
        return result.value();
    }
    void mapped(Window &window) {
        window.resize(320, 240);
        window.show();
        QVERIFY(QTest::qWaitForWindowExposed(&window));
    }
    void verifyOrdering(const QString &tag, int count) {
        if (!tags()) return;
        const auto rows = records("Protocol");
        int tagged = 0;
        for (int i = 0; i < rows.size(); ++i) {
            const auto row = qdbus_cast<QVariantMap>(rows[i]);
            if (row["request"] != "set_toplevel_tag" || row["tag"] != tag) continue;
            ++tagged;
            const auto role = row["role"].toUInt();
            uint surface = 0;
            int created = -1;
            for (int j = i - 1; j >= 0; --j) {
                const auto before = qdbus_cast<QVariantMap>(rows[j]);
                if (before["request"] == "get_toplevel" && before["new_id"].toUInt() == role
                    && before["pid"] == row["pid"]) {
                    surface = before["surface"].toUInt(); created = j; break;
                }
            }
            QVERIFY(surface && created >= 0);
            // Stronger than a mapping test: no wl_surface.commit at all can
            // precede the tag on this newly created role, even an empty one.
            for (int j = created + 1; j < i; ++j) {
                const auto before = qdbus_cast<QVariantMap>(rows[j]);
                QVERIFY(before["request"] != "commit" || before["object"].toUInt() != surface
                        || before["pid"] != row["pid"]);
            }
        }
        QCOMPARE(tagged, count);
    }
private slots:
    void firstWindowAndNativeRecreation() {
        Window window;
        const QString tag = "native-proof-first";
        auto native = FerricWaylandWindow::prepare(&window, tag);
        QVERIFY(native);
        QCOMPARE(native->generation(), quint64(0));
        mapped(window);
        const auto first = native->generation();
        QVERIFY(first);
        QCOMPARE(native->tagSubmitted(), tags());
        verifyOrdering(tag, 1);
        QSignalSpy invalidated(native, &FerricWaylandWindow::invalidated);
        window.hide();
        QCOMPARE(native->generation(), quint64(0)); // synchronous, no event-loop turn
        QCOMPARE(invalidated.size(), 1);
        mapped(window); // real role-only recreation on the surviving wl_surface
        QVERIFY(native->generation() > first);
        const auto second = native->generation();
        QCOMPARE(native->tagSubmitted(), tags());
        verifyOrdering(tag, 2);
        window.destroy(); // real Qt platform-window and wl_surface destruction
        QCOMPARE(native->generation(), quint64(0));
        QCOMPARE(invalidated.size(), 2);
        mapped(window);
        QVERIFY(native->generation() > second);
        verifyOrdering(tag, 3);
    }
    void sharedExportsAndCancellation() {
        Window window;
        auto native = FerricWaylandWindow::prepare(&window, "native-proof-shared");
        mapped(window);
        const auto before = records("Exports").size();
        control.call("HoldExports", true);
        auto cancelled = native->exportToplevel(this);
        QSignalSpy cancellation(cancelled, &FerricWaylandExport::completed);
        auto retained = native->exportToplevel(this);
        QSignalSpy completed(retained, &FerricWaylandExport::completed);
        if (exporter()) {
            QTRY_COMPARE(records("Exports").size(), before + 1);
            cancelled->cancel();
            QCOMPARE(cancellation.size(), 1);
            QCOMPARE(qvariant_cast<FerricExportResult>(cancellation[0][0]).status, FerricExportStatus::Cancelled);
            control.call("ReleaseExports");
        }
        QTRY_COMPARE(completed.size(), 1);
        const auto result = qvariant_cast<FerricExportResult>(completed[0][0]);
        QCOMPARE(result.status, exporter() ? FerricExportStatus::Success : FerricExportStatus::Unsupported);
        QCOMPARE(result.generation, native->generation());
        if (exporter()) {
            QVERIFY(!result.handle.isEmpty());
            auto cached = native->exportToplevel(this);
            QSignalSpy ready(cached, &FerricWaylandExport::completed);
            QTRY_COMPARE(ready.size(), 1);
            QCOMPARE(qvariant_cast<FerricExportResult>(ready[0][0]).handle, result.handle);
            QCOMPARE(records("Exports").size(), before + 1);
        }
        control.call("HoldExports", false);
        delete cancelled;
        delete retained;
    }
    void pendingExportInvalidationAndChurn() {
        Window window;
        auto native = FerricWaylandWindow::prepare(&window, "native-proof-churn");
        control.call("HoldExports", true);
        for (int i = 0; i < 20; ++i) {
            mapped(window);
            const auto before = records("Exports").size();
            const auto generation = native->generation();
            auto subscription = native->exportToplevel(this);
            QSignalSpy finished(subscription, &FerricWaylandExport::completed);
            if (exporter()) QTRY_COMPARE(records("Exports").size(), before + 1);
            else QTRY_COMPARE(finished.size(), 1);
            window.hide();
            if (i % 2) window.destroy();
            QCOMPARE(native->generation(), quint64(0));
            QCOMPARE(finished.size(), 1);
            const auto result = qvariant_cast<FerricExportResult>(finished[0][0]);
            QCOMPARE(result.status, exporter() ? FerricExportStatus::Invalidated : FerricExportStatus::Unsupported);
            QCOMPARE(result.generation, generation);
            // Destroyed exports are removed by the owned server before any
            // delayed delivery; replies already queued on the client are
            // discarded by libwayland after proxy destruction.
            control.call("ReleaseExports");
            QCoreApplication::processEvents();
            QTRY_COMPARE(resources()["exports"].toInt(), 0);
            QTRY_COMPARE(resources()["roles"].toInt(), 0);
            QTRY_COMPARE(resources()["views"].toInt(), 0);
            QCOMPARE(finished.size(), 1);
            delete subscription;
        }
        control.call("HoldExports", false);
        verifyOrdering("native-proof-churn", 20);
    }
    void zzShutdownWithLiveWindows() {
        Window window;
        auto native = FerricWaylandWindow::prepare(&window, "native-proof-shutdown");
        mapped(window);
        control.call("HoldExports", true);
        const auto before = records("Exports").size();
        auto subscription = native->exportToplevel(this);
        QSignalSpy finished(subscription, &FerricWaylandExport::completed);
        if (exporter()) QTRY_COMPARE(records("Exports").size(), before + 1);
        else QTRY_COMPARE(finished.size(), 1);
        ferric_shutdown_wayland();
        QCOMPARE(native->generation(), quint64(0));
        QCOMPARE(finished.size(), 1);
        QCoreApplication::processEvents();
        QCOMPARE(finished.size(), 1);
        window.hide();
        window.destroy();
    }
};
int main(int argc, char **argv) {
    QApplication app(argc, argv);
    ferric_initialize_wayland();
    NativeTests tests;
    const int result = QTest::qExec(&tests, argc, argv);
    ferric_shutdown_wayland();
    return result;
}
#include "tst_native_wayland.moc"
