// Qt-only control for separating SDK lifetime findings from the owned bridge.
#include <QApplication>
#include <QPainter>
#include <QRasterWindow>
#include <QtTest>

class ControlWindow final : public QRasterWindow {
    void paintEvent(QPaintEvent *) override {
        QPainter painter(this);
        painter.fillRect(QRect(QPoint(), size()), Qt::blue);
    }
};
class QtLifecycleControl final : public QObject {
    Q_OBJECT
private slots:
    void nativeRecreationWithoutFerric() {
        ControlWindow window;
        window.resize(320, 240);
        window.show();
        QVERIFY(QTest::qWaitForWindowExposed(&window));
        window.hide();
        window.show();
        QVERIFY(QTest::qWaitForWindowExposed(&window));
        window.destroy();
        window.show();
        QVERIFY(QTest::qWaitForWindowExposed(&window));
        for (int i = 0; i < 20; ++i) {
            window.hide(); window.show();
            QVERIFY(QTest::qWaitForWindowExposed(&window));
        }
    }
    void successiveWindowsWithoutFerric() {
        for (int index = 0; index < 3; ++index) {
            ControlWindow window;
            window.resize(320, 240); window.show();
            QVERIFY(QTest::qWaitForWindowExposed(&window));
            if (index == 1) {
                for (int cycle = 0; cycle < 20; ++cycle) {
                    window.hide();
                    if (cycle % 2) window.destroy();
                    window.show();
                    QVERIFY(QTest::qWaitForWindowExposed(&window));
                }
            }
            if (index == 2) { window.hide(); window.destroy(); }
        }
    }
};
QTEST_MAIN(QtLifecycleControl)
#include "tst_qt_lifecycle_control.moc"
