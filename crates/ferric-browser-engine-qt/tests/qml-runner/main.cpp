#include <QtQuickTest/quicktest.h>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <QElapsedTimer>
#include <QObject>
#include <QQmlEngine>
#include <QQmlContext>
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

class TestSetup final : public QObject
{
    Q_OBJECT
public slots:
    void qmlEngineAvailable(QQmlEngine *engine)
    {
        engine->rootContext()->setContextProperty(
            "testArtifactDirectory", qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR"));
    }
};

int main(int argc, char **argv)
{
    // WebEngine must configure shared OpenGL contexts before Qt Quick Test
    // constructs its QGuiApplication, including when tests run offscreen.
    QtWebEngineQuick::initialize();
    qmlRegisterSingletonType<PerformanceClock>("Ferric.Testing", 1, 0, "PerformanceClock",
                                               createPerformanceClock);
    TestSetup setup;
    return quick_test_main_with_setup(argc, argv, "ferric_qml_tests", nullptr, &setup);
}

#include "main.moc"
