// Test-only UI confirmations, loaded explicitly for a disposable perf profile.
// Calls guarded dialog methods after rendering; this is not physical input qualification.
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
    const int expectedPages;
    QFileSystemWatcher watcher;
    QTimer timer;
    QPointer<QQuickWindow> presentedWindow;
    QPointer<QQuickItem> presentedDialog;
    QMetaObject::Connection frameConnection;
    bool presented = false;
    int attempts = 0;
    int expansionDecisions = 0;
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
    bool expansionAuthorized() const {
        QFile authorization(root + "/confirm-page-windows");
        return expectedPages > 12 && authorization.open(QIODevice::ReadOnly)
            && authorization.readAll() == QByteArray("Open up to ") + QByteArray::number(expectedPages)
                + " performance fixture page windows.\n";
    }
    static QQuickItem *findExpansion(QQuickItem *item, int maximum) {
        if (!item->isVisible()) return nullptr;
        const int count = item->property("pageCount").toInt();
        if (count > 0 && count < maximum
            && item->property("commandText").toString() == "Restore page windows"
            && item->property("title").toString() == QString("Open %1 page windows?").arg(count)) return item;
        for (auto child : item->childItems()) if (auto result = findExpansion(child, maximum)) return result;
        return nullptr;
    }
    bool awaitFrame(QQuickWindow *window, QQuickItem *dialog) {
        if (presentedWindow != window || presentedDialog != dialog) {
            QObject::disconnect(frameConnection);
            presentedWindow = window; presentedDialog = dialog; presented = false;
            frameConnection = connect(window, &QQuickWindow::frameSwapped, this, [this] { presented = true; });
            window->requestUpdate();
            return false;
        }
        if (!presented) window->requestUpdate();
        return presented;
    }
    void poll() {
        const bool expansion = expansionAuthorized();
        if (!authorized() && !expansion) { timer.stop(); return; }
        if (expansion) {
            for (auto candidate : QGuiApplication::allWindows()) {
                auto window = qobject_cast<QQuickWindow *>(candidate);
                if (!window || !window->isVisible()) continue;
                auto dialog = findExpansion(window->contentItem(), expectedPages);
                if (!dialog || !dialog->property("interactive").toBool()
                    || dialog->property("decisionTaken").toBool()) continue;
                auto action = find(dialog, "commandDialogAction-confirm");
                if (!action || !action->isEnabled()
                    || action->property("actionLabel").toString() != "Open page windows"
                    || !awaitFrame(window, dialog)) continue;
                const auto bounds = action->mapRectToScene(action->boundingRect());
                if (bounds.isEmpty() || !QRectF(0,0,window->width(),window->height()).contains(bounds)) continue;
                if (++expansionDecisions > expectedPages) qFatal("Too many fixture expansion prompts");
                QFile evidence(root + "/page-windows-confirmed");
                if (!evidence.open(QIODevice::WriteOnly | QIODevice::Append)) qFatal("Could not record expansion confirmation");
                evidence.write(QByteArray::number(dialog->property("pageCount").toInt()) + "\n");
                // Keep the bounded authorization through asynchronous moves;
                // each new live signature may require another confirmation.
                // The harness removes it once every requested page has settled.
                // The production dialog still validates its live signature,
                // enabled action, decision state and bounded expansion proof.
                if (!QMetaObject::invokeMethod(dialog, "requestAction", Qt::DirectConnection,
                        Q_ARG(QVariant, QVariant(QStringLiteral("confirm")))))
                    qFatal("Could not confirm the owned expansion dialog");
                return;
            }
            return;
        }
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
            if (!awaitFrame(window, dialog)) return;
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
                        << candidate->property("windowShutdownPagePromptVisible")
                        << candidate->property("applicationShutdownCommitStarted")
                        << candidate->property("applicationShutdownForcePromptVisible");
                    auto ui = candidate->property("primaryBrowserUi").value<QObject *>();
                    if (ui) qWarning() << "perf shutdown status" << ui->property("status_text");
                    auto work = candidate->property("applicationShutdownTimerObject").value<QObject *>();
                    if (work) qWarning() << "perf shutdown timer" << work->property("running");
                }
            });
            QFile evidence(root + "/quit-confirmed");
            if (!evidence.open(QIODevice::WriteOnly | QIODevice::NewOnly)) qFatal("Could not record performance quit");
            evidence.write("Qt button click on the rendered owned quit dialog.\n");
            return;
        }
    }
public:
    explicit OwnedPerfQuit(QString path) : root(std::move(path)),
        expectedPages(qEnvironmentVariableIntValue("FERRIC_PERF_EXPECTED_PAGES")) {
        if (!watcher.addPath(root)) qFatal("Could not watch the owned performance profile");
        timer.setInterval(25);
        connect(&watcher, &QFileSystemWatcher::directoryChanged, this, [this] {
            if ((authorized() || expansionAuthorized()) && !timer.isActive()) timer.start();
        });
        connect(&timer, &QTimer::timeout, this, [this] { poll(); });
        QTimer::singleShot(0, this, [this] { if (expansionAuthorized()) timer.start(); });
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
