// Native input and observation for owned, offscreen process-restart tests.
// This plugin is never linked into or shipped with the browser.
#include <QDir>
#include <QFile>
#include <QGenericPlugin>
#include <QGuiApplication>
#include <QQmlContext>
#include <QQmlExpression>
#include <QQuickWindow>
#include <QTimer>
#include <QtTest>

class StartupRecoveryInput final : public QObject {
    QTimer timer;
    const QString root;
    bool sent = false, finished = false;

    void poll() {
        if (finished) return;
        QFile authorization(root + "/recover");
        if (!sent && (!authorization.open(QIODevice::ReadOnly)
                || authorization.readAll() != "Recover this owned browser.\n")) return;
        for (auto candidate : QGuiApplication::allWindows()) {
            auto window = qobject_cast<QQuickWindow *>(candidate);
            if (!window || !window->isVisible()
                    || !window->property("profileBootstrapComplete").toBool()) continue;
            auto ui = window->property("primaryBrowserUi").value<QObject *>();
            if (!ui) continue;
            const bool recovery = window->property("recoveryAvailable").toBool();
            if ((!sent && !recovery) || (sent && (recovery
                    || ui->property("load_state").toString() != "complete"))) continue;
            QQmlExpression expression(qmlContext(window), window, QStringLiteral(
                "(function(){if(tabsModel.count!==browserUi.tab_count)return '';"
                "var a=[];for(var i=0;i<browserUi.tab_count;i++){var v=tabViewAt(i);"
                "if(!v)return '';a.push({id:browserUi.tab_id_for_index(i),"
                "modelUrl:tabsModel.get(i).url,loaded:tabsModel.get(i).loaded,"
                "viewUrl:String(v.url)})}return JSON.stringify(a)})()"));
            const auto rows = expression.evaluate().toString();
            if (expression.hasError()) {
                QFile failure(root + "/error");
                if (failure.open(QIODevice::WriteOnly)) failure.write(expression.error().toString().toUtf8());
                finished = true;
                return;
            }
            if (rows.isEmpty()) continue;
            QFile out(root + (sent ? "/after.json" : "/before.json"));
            if (!out.open(QIODevice::WriteOnly | QIODevice::Truncate)) return;
            out.write(rows.toUtf8()); out.close();
            if (sent) { finished = true; return; }
            window->requestActivate();
            if (!window->isActive()) return;
            sent = true;
            QTest::keyClick(window, Qt::Key_Y);
            return;
        }
    }
public:
    explicit StartupRecoveryInput(QString path) : QObject(qApp), root(std::move(path)) {
        connect(&timer, &QTimer::timeout, this, [this] { poll(); });
        timer.start(50);
    }
};

class StartupRecoveryTestPlugin final : public QGenericPlugin {
    Q_OBJECT
    Q_PLUGIN_METADATA(IID "org.qt-project.Qt.QGenericPluginFactoryInterface" FILE "startup_recovery_test_plugin.json")
public:
    QObject *create(const QString &key, const QString &) override {
        const auto root = qEnvironmentVariable("FERRIC_OWNED_STARTUP_RECOVERY_ROOT");
        QFile marker(root + "/.owned-recovery-fixture");
        if (key != "ferricownedstartuprecovery" || QGuiApplication::platformName() != "offscreen"
                || !QDir(root).isAbsolute() || root.isEmpty() || !marker.open(QIODevice::ReadOnly)
                || marker.readAll() != "Owned startup recovery test only.\n") return nullptr;
        return new StartupRecoveryInput(root);
    }
};
#include "startup_recovery_test_plugin.moc"
