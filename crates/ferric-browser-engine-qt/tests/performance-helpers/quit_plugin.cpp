// Test-only native confirmation, loaded explicitly for a disposable perf profile.
// Directory notifications keep the helper dormant throughout idle measurement.
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QFileSystemWatcher>
#include <QGenericPlugin>
#include <QGuiApplication>
#include <QPointer>
#include <QQuickItem>
#include <QQuickWindow>
#include <QTimer>

class OwnedPerfQuit final : public QObject {
    const QString root;
    QFileSystemWatcher watcher;
    QTimer timer;
    QPointer<QQuickWindow> presentedWindow;
    QPointer<QQuickItem> presentedDialog;
    QMetaObject::Connection frameConnection;
    bool presented = false;
    int attempts = 0;
    static QQuickItem *find(QQuickItem *item, const QString &name) {
        if (!item->isVisible()) return nullptr;
        if (item->objectName() == name) return item;
        for (auto child : item->childItems()) if (auto result = find(child, name)) return result;
        return nullptr;
    }
    bool authorized() const {
        QFile authorization(root + "/confirm-quit");
        return authorization.open(QIODevice::ReadOnly)
            && authorization.readAll() == "Confirm this performance fixture's quit.\n";
    }
    void poll() {
        if (!authorized()) { timer.stop(); return; }
        const bool diagnose = ++attempts % 200 == 0;
        for (auto candidate : QGuiApplication::allWindows()) {
            auto window = qobject_cast<QQuickWindow *>(candidate);
            if (!window || !window->isVisible()) continue;
            auto dialog = find(window->contentItem(), "shutdownDecisionDialog");
            if (diagnose) qWarning() << "perf quit window" << window->objectName() << window->isVisible()
                << "dialog" << bool(dialog) << "presented" << presented;
            if (!dialog || dialog->property("title").toString() != "Quit Ferric Browser?"
                || dialog->property("commandText").toString() != ":quit") continue;
            auto button = find(dialog, "shutdownProceedButton");
            if (!button || !button->isEnabled() || button->property("text").toString() != "Quit browser") continue;
            if (presentedWindow != window || presentedDialog != dialog) {
                QObject::disconnect(frameConnection);
                presentedWindow = window; presentedDialog = dialog; presented = false;
                // Weston may stop frame callbacks for a fully occluded root.
                // Remap only after measurement, like bringing Quit's window forward.
                window->hide();
                window->show();
                window->requestActivate();
                frameConnection = connect(window, &QQuickWindow::frameSwapped, this, [this] { presented = true; });
                window->requestUpdate();
                return;
            }
            if (!presented) { window->requestUpdate(); return; }
            const auto bounds = button->mapRectToScene(button->boundingRect());
            if (bounds.isEmpty() || !QRectF(0,0,window->width(),window->height()).contains(bounds)) return;
            if (!QFile::remove(root + "/confirm-quit")) qFatal("Could not consume owned performance authorization");
            timer.stop();
            if (!QMetaObject::invokeMethod(button, "click", Qt::DirectConnection))
                qFatal("Owned performance quit button has no click method");
            qWarning() << "perf quit confirmation delivered";
            QTimer::singleShot(5000, this, [] {
                for (auto candidate : QGuiApplication::allWindows()) {
                    qWarning() << "perf shutdown state" << candidate->objectName()
                        << candidate->property("shutdownConfirmationVisible")
                        << candidate->property("applicationShutdownInProgress")
                        << candidate->property("applicationShutdownStage")
                        << candidate->property("shutdownPagePromptVisible")
                        << candidate->property("windowApplicationQuitPrepared")
                        << candidate->property("windowShutdownPagePromptVisible");
                }
            });
            QFile evidence(root + "/quit-confirmed");
            if (!evidence.open(QIODevice::WriteOnly | QIODevice::NewOnly)) qFatal("Could not record performance quit");
            evidence.write("Qt button click on the rendered owned quit dialog.\n");
            return;
        }
    }
public:
    explicit OwnedPerfQuit(QString path) : root(std::move(path)) {
        watcher.addPath(root);
        timer.setInterval(25);
        connect(&watcher, &QFileSystemWatcher::directoryChanged, this, [this] {
            if (authorized() && !timer.isActive()) timer.start();
        });
        connect(&timer, &QTimer::timeout, this, [this] { poll(); });
    }
};
class PerfQuitPlugin final : public QGenericPlugin {
    Q_OBJECT
    Q_PLUGIN_METADATA(IID "org.qt-project.Qt.QGenericPluginFactoryInterface" FILE "quit_plugin.json")
public:
    QObject *create(const QString &name, const QString &) override {
        const auto root = qEnvironmentVariable("FERRIC_PERF_QUIT_ROOT");
        const auto args = QCoreApplication::arguments();
        const auto at = args.indexOf("--basedir");
        QFile marker(root + "/.ferric-performance-fixture");
        if (name != "ferricperfquitfixture" || !QDir(root).isAbsolute()
            || !QFileInfo(root).fileName().startsWith("ferric-browser-ui-perf-")
            || at < 0 || at + 1 >= args.size() || args[at + 1] != root
            || !marker.open(QIODevice::ReadOnly) || marker.readAll() != "Ferric performance fixture only.\n") return nullptr;
        return new OwnedPerfQuit(root);
    }
};
#include "quit_plugin.moc"
