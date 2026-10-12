#include <QtQuickTest/quicktest.h>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <QElapsedTimer>
#include <QObject>
#include <QQmlEngine>
#include <QJSEngine>
#include <QtQml/qqml.h>

class PerformanceClock final : public QObject
{
    Q_OBJECT

public:
    PerformanceClock() { timer_.start(); }

    Q_INVOKABLE qint64 nowNsecs() const { return timer_.nsecsElapsed(); }

private:
    QElapsedTimer timer_;
};

static QObject *createPerformanceClock(QQmlEngine *, QJSEngine *)
{
    return new PerformanceClock;
}

int main(int argc, char **argv)
{
    // WebEngine must configure shared OpenGL contexts before Qt Quick Test
    // constructs its QGuiApplication, including when tests run offscreen.
    QtWebEngineQuick::initialize();
    qmlRegisterSingletonType<PerformanceClock>("Ferric.Testing", 1, 0, "PerformanceClock",
                                               createPerformanceClock);
    return quick_test_main(argc, argv, "ferric_qml_tests", nullptr);
}

#include "main.moc"
