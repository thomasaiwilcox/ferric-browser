#include <QtQuickTest/quicktest.h>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <QElapsedTimer>
#include <QApplication>
#include <QObject>
#include <QQmlEngine>
#include <QQmlContext>
#include <QJSEngine>
#include <QStringList>
#include <QtQml/qqml.h>
#include "../../src/script_runner.h"
#include "../../src/browser_key_router.h"
#include "../../src/settings_model.h"
#include "../../src/native_file_chooser.h"
#include "../../src/gui_application.h"
#include "../../src/download_source.h"

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
        engine->rootContext()->setContextProperty("testEmptySettingsOptions", QStringList{});
    }
};

int main(int argc, char **argv)
{
    // WebEngine must configure shared OpenGL contexts before Qt Quick Test
    // constructs its QGuiApplication, including when tests run offscreen.
    QtWebEngineQuick::initialize();
    QVector<QByteArray> arguments;
    for (int index = 0; index < argc; ++index) arguments.append(QByteArray(argv[index]));
    auto application = ferric_browser_gui_application(arguments);
    if (!application) return 1;
    qmlRegisterType<FerricBrowserKeyRouter>("io.github.ferricbrowser", 1, 0, "BrowserKeyRouter");
    qmlRegisterType<FerricScriptRunner>("Ferric.Testing", 1, 0, "ScriptRunner");
    qmlRegisterType<FerricScriptRunner>("io.github.ferricbrowser", 1, 0, "FerricScriptRunner");
    qmlRegisterType<FerricBrowserSettingsModel>("io.github.ferricbrowser", 1, 0, "FerricSettingsModel");
    qmlRegisterType<FerricNativeFileChooser>("io.github.ferricbrowser", 1, 0, "NativeFileChooser");
    qmlRegisterType<FerricDownloadSource>("io.github.ferricbrowser", 1, 0, "FerricDownloadSource");
    qmlRegisterSingletonType<PerformanceClock>("Ferric.Testing", 1, 0, "PerformanceClock",
                                               createPerformanceClock);
    TestSetup setup;
    return quick_test_main_with_setup(argc, argv, "ferric_qml_tests", nullptr, &setup);
}

#include "main.moc"
