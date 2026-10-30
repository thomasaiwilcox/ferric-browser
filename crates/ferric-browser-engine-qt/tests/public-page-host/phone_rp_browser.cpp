// Disposable production-Root harness. Never installed as the user's browser.
#include "credential_network_policy.h"
#include "gui_application.h"
#include "url_display.h"
#include <QApplication>
#include <QDialog>
#include <QElapsedTimer>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQmlExpression>
#include <QQuickItem>
#include <QQuickWindow>
#include <QPushButton>
#include <QTemporaryDir>
#include <QTimer>
#include <QWebEngineFrame>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <cstdio>
#include <memory>
#include <signal.h>
#include <sys/prctl.h>
#include <unistd.h>

extern "C" bool cxx_qt_init_crate_ferric_browser_engine_qt();

int main(int argc, char **argv)
{
    const auto parent = getppid();
    if (parent <= 1 || prctl(PR_SET_PDEATHSIG, SIGTERM) != 0 || getppid() != parent) return 2;
    if (argc != 5 || (QByteArray(argv[1]) != "--check" && QByteArray(argv[1]) != "--interactive")
        || QByteArray(argv[4]) != "--direct-network") return 2;
    const bool check = QByteArray(argv[1]) == "--check";
    const auto certificate = QFileInfo(QString::fromLocal8Bit(argv[2])).canonicalFilePath();
    QTemporaryDir storage(QString::fromLocal8Bit(argv[3]) + QStringLiteral("/qt-profile-XXXXXX"));
    if (certificate.isEmpty() || !storage.isValid()) return 2;
    ferric_browser_enable_software_rendering();
    ferric_browser_register_internal_scheme();
    QtWebEngineQuick::initialize();
    if (!cxx_qt_init_crate_ferric_browser_engine_qt()) return 2;
    QVector<QByteArray> arguments;
    for (int index = 0; index < argc; ++index) arguments.append(QByteArray(argv[index]));
    auto application = ferric_browser_gui_application(arguments);
    if (!application || !FerricCredentialNetworkPolicy::capture(true)) return 2;
    QGuiApplication::setDesktopFileName(QStringLiteral("ferric-phone-rp-fixture"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("Ferric disposable passkey test"));
    QQmlEngine engine;
    QQmlComponent component(&engine, QUrl("qrc:/qt/qml/io/github/ferricbrowser/qml/Main.qml"));
    std::unique_ptr<QObject> root(component.beginCreate(engine.rootContext()));
    if (!root) return 2;
    const QString config = QStringLiteral(R"JSON({"schema_version":3,"tabs":{"presentation":"windows"},"passkeys":{"phone":"on"},"blocking":{"enabled":false}})JSON");
    component.setInitialProperties(root.get(), {{"temporaryProfile", true}, {"ephemeralProfile", true},
        {"ephemeralInvocationToken", "phone-rp-fixture"}, {"storageBasePath", storage.path()},
        {"softwareRendering", true}, {"nativeWayland", true}, {"safeMode", true}, {"userscriptsOff", true},
        {"startupConfigJson", config}, {"startupConfigBaseJson", config},
        {"startupCliOverridesJson", "{\"schema_version\":3}"},
        {"startupProfileOverridesJson", "{\"schema_version\":3}"},
        {"startupUrl", "https://passkeys.ferric.test:8443/"}, {"width", 1000}, {"height", 760}});
    auto *prototype = root->property("browserProfilePrototypeObject").value<QObject *>();
    // This occurs before Component.onCompleted creates the actual profile.
    if (!prototype || !prototype->setProperty("additionalTrustedCertificateFiles",
            QVariant::fromValue(QList<QUrl>{QUrl::fromLocalFile(certificate)}))) return 2;
    component.completeCreate();
    if (component.isError()) return 2;
    auto *window = qobject_cast<QQuickWindow *>(root.get());
    if (!window) return 2;
    window->show();
    window->requestActivate();
    if (!check) return application->exec();

    QElapsedTimer elapsed;
    elapsed.start();
    QTimer monitor;
    monitor.setInterval(50);
    bool requested = false;
    int result = 1;
    QObject::connect(&monitor, &QTimer::timeout, application.get(), [&] {
        if (elapsed.elapsed() > 20000) { application->exit(1); return; }
        if (!requested) {
            QQmlExpression expression(engine.rootContext(), root.get(), QStringLiteral("activeWebView()"));
            auto *view = qobject_cast<QQuickItem *>(expression.evaluate().value<QObject *>());
            if (!view || view->property("loading").toBool() || !root->property("profileBootstrapComplete").toBool()
                || QGuiApplication::focusWindow() != window) return;
            auto frame = view->property("mainFrame").value<QWebEngineFrame>();
            if (!frame.isValid() || frame.url() != QUrl("https://passkeys.ferric.test:8443/")) return;
            requested = true;
            // Admission only: never select Phone or launch the real helper.
            frame.runJavaScript(QStringLiteral(
                "navigator.credentials.get({publicKey:{challenge:new Uint8Array([1]),rpId:'passkeys.ferric.test',timeout:15000}}).catch(()=>{})"), 0);
            return;
        }
        for (auto *widget : QApplication::topLevelWidgets()) {
            auto *dialog = qobject_cast<QDialog *>(widget);
            if (!dialog || !dialog->isVisible() || dialog->objectName() != QStringLiteral("ferricCredentialProviderChoice")) continue;
            auto *phone = dialog->findChild<QPushButton *>(QStringLiteral("credentialPhoneProvider"));
            result = phone && phone->isEnabled() ? 0 : 1;
            const auto receipt = QJsonDocument(QJsonObject{{"production_root", true},
                {"phone_available", result == 0}, {"helper_selected", false}, {"physical_qualified", false}})
                .toJson(QJsonDocument::Compact);
            fwrite(receipt.data(), 1, size_t(receipt.size()), stdout);
            fputc('\n', stdout);
            fflush(stdout);
            dialog->reject();
            application->exit(result);
            return;
        }
    });
    monitor.start();
    const int eventResult = application->exec();
    return eventResult ? eventResult : result;
}
