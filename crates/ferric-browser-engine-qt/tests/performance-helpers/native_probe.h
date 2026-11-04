// Test-only, event-driven measurements on actual Ferric QQuickWindows. Loaded
// only by the explicitly authorized disposable-profile performance plugin.
#ifndef FERRIC_PERF_NATIVE_PROBE_H
#define FERRIC_PERF_NATIVE_PROBE_H
#include <QAbstractEventDispatcher>
#include <QElapsedTimer>
#include <QEvent>
#include <QFileSystemWatcher>
#include <QJsonDocument>
#include <QJsonObject>
#include <QQuickWindow>
#include <QSaveFile>
#include <QSet>

class OwnedNativeProbe final : public QObject {
    const QString root;
    const int expected;
    QFileSystemWatcher watcher;
    QSet<QString> rendered;
    QSet<QString> original;
    QSet<QQuickWindow *> observed;
    QPointer<QQuickWindow> subject;
    QPointer<QObject> chooser;
    QMetaObject::Connection operationFrame;
    QElapsedTimer elapsed, idle;
    quint64 wakeups = 0;
    int sequence = 0;
    QString pending;
    bool startupRecorded = false;
    QList<QQuickWindow *> windows() const {
        QList<QQuickWindow *> result;
        for (auto candidate : QGuiApplication::allWindows()) {
            auto window = qobject_cast<QQuickWindow *>(candidate);
            if (window && window->isVisible()
                && window->property("pageWindowTag").toString().startsWith("ferric-page-"))
                result.append(window);
        }
        return result;
    }
    void write(const QString &name, const QJsonObject &object) {
        QSaveFile file(root + '/' + name);
        if (!file.open(QIODevice::WriteOnly)
            || file.write(QJsonDocument(object).toJson(QJsonDocument::Compact)) < 0 || !file.commit())
            qFatal("Could not write native performance evidence");
    }
    void reply(QJsonObject result) {
        QObject::disconnect(operationFrame);
        result.insert("sequence", sequence);
        result.insert("operation", pending);
        result.insert("windows", windows().size());
        pending.clear();
        write("native-response.json", result);
    }
    void frame(QQuickWindow *window, bool freshOperation = false) {
        if (!window->isVisible() || !window->isExposed()) return;
        const auto tag = window->property("pageWindowTag").toString();
        rendered.insert(tag);
        const auto live = windows();
        if (!startupRecorded && live.size() == expected
            && std::all_of(live.cbegin(), live.cend(), [this](auto item) {
                return rendered.contains(item->property("pageWindowTag").toString());
            })) {
            startupRecorded = true;
            write("native-startup.json", {{"windows", live.size()}, {"boundary", "all page windows exposed and frameSwapped delivered on GUI thread"}});
        }
        if ((pending == "create" && !original.contains(tag))
            || (freshOperation && (pending == "role" || pending == "surface") && subject == window)) {
            const double milliseconds = elapsed.nsecsElapsed() / 1000000.0;
            subject = window;
            reply({{"ok", true}, {"milliseconds", milliseconds}, {"tag", tag}});
        }
    }
    void command() {
        QFile file(root + "/native-command.json");
        if (!file.open(QIODevice::ReadOnly)) return;
        const auto request = QJsonDocument::fromJson(file.readAll()).object();
        const int next = request.value("sequence").toInt();
        if (next <= sequence) return;
        if (!pending.isEmpty()) qFatal("Overlapping native performance operations");
        sequence = next;
        pending = request.value("operation").toString();
        const auto live = windows();
        if (pending == "state") {
            reply({{"ok", true}, {"dialog_backend", chooser ? chooser->property("backend").toString() : QString()},
                {"dialog_visible", chooser && chooser->property("visible").toBool()}}); return;
        }
        if (pending == "dialog-open") {
            for (auto window : live) {
                chooser = window->findChild<QObject *>(QStringLiteral("diagnosticsExportChooser"));
                if (chooser) break;
            }
            if (!chooser || !chooser->setProperty("portalRequired", true)
                || !QMetaObject::invokeMethod(chooser, "open", Qt::DirectConnection)) {
                reply({{"ok", false}, {"error", "installed product diagnostics chooser unavailable"}});
            } else reply({{"ok", true}});
            return;
        }
        if (pending == "dialog-close") {
            const bool closed = chooser && QMetaObject::invokeMethod(chooser, "close", Qt::DirectConnection);
            reply({{"ok", closed}}); return;
        }
        if (pending == "idle-start") {
            wakeups = 0; idle.start(); reply({{"ok", true}}); return;
        }
        if (pending == "idle-stop") {
            const auto count = wakeups;
            const double seconds = idle.nsecsElapsed() / 1000000000.0;
            idle.invalidate();
            reply({{"ok", seconds > 0}, {"seconds", seconds}, {"dispatcher_awake", double(count)}});
            return;
        }
        if (pending == "cleanup") {
            if (subject && !original.contains(subject->property("pageWindowTag").toString())) {
                subject->setProperty("windowShutdownApproved", true);
                subject->close();
            }
            subject.clear(); reply({{"ok", true}}); return;
        }
        if (pending == "create") {
            original.clear();
            QQuickWindow *primary = nullptr;
            for (auto window : live) {
                original.insert(window->property("pageWindowTag").toString());
                if (window->objectName() == "ferric_browserWindow") primary = window;
            }
            auto ui = primary ? primary->property("primaryBrowserUi").value<QObject *>() : nullptr;
            bool accepted = false;
            elapsed.start();
            if (!ui || !QMetaObject::invokeMethod(ui, "execute_command", Qt::DirectConnection,
                    Q_RETURN_ARG(bool, accepted), Q_ARG(QString, QStringLiteral("open --target window about:blank"))) || !accepted)
                reply({{"ok", false}, {"error", "production window creation command rejected"}});
            return;
        }
        if ((pending == "role" || pending == "surface") && !live.isEmpty()) {
            subject = live.first();
            const auto tag = subject->property("pageWindowTag").toString();
            const auto group = subject->property("pageGroupId");
            rendered.remove(tag);
            elapsed.start();
            subject->hide();
            if (pending == "surface") subject->destroy();
            subject->show();
            if (subject->property("pageWindowTag").toString() != tag || subject->property("pageGroupId") != group) {
                reply({{"ok", false}, {"error", "logical identity changed during native recreation"}});
                return;
            }
            // A queued frame from before hide/destroy cannot complete a new
            // operation. Only emissions after this connection was installed
            // can witness the replacement, and old queued deliveries carry
            // their original command sequence.
            const QPointer<QQuickWindow> guarded(subject);
            const int admittedSequence = sequence;
            operationFrame = connect(subject, &QQuickWindow::frameSwapped, this,
                [this, guarded, admittedSequence] {
                    if (guarded && sequence == admittedSequence) frame(guarded, true);
                }, Qt::QueuedConnection);
            subject->requestUpdate();
            return;
        }
        reply({{"ok", false}, {"error", "unsuitable native performance request"}});
    }
protected:
    bool eventFilter(QObject *watched, QEvent *event) override {
        if (event->type() == QEvent::Show) {
            auto window = qobject_cast<QQuickWindow *>(watched);
            if (window && !observed.contains(window)) {
                observed.insert(window);
                const QPointer<QQuickWindow> guarded(window);
                connect(window, &QQuickWindow::frameSwapped, this, [this, guarded] {
                    if (guarded) frame(guarded);
                }, Qt::QueuedConnection);
                connect(window, &QObject::destroyed, this, [this, window] { observed.remove(window); });
            }
        }
        return false;
    }
public:
    OwnedNativeProbe(const QString &path, int pages, QObject *parent)
        : QObject(parent), root(path), expected(pages) {
        qGuiApp->installEventFilter(this);
        if (!watcher.addPath(root)) qFatal("Could not watch owned native probe commands");
        connect(&watcher, &QFileSystemWatcher::directoryChanged, this, [this] { command(); });
        auto dispatcher = QAbstractEventDispatcher::instance(qGuiApp->thread());
        if (!dispatcher) qFatal("Native performance probe has no GUI dispatcher");
        connect(dispatcher, &QAbstractEventDispatcher::awake, this, [this] {
            if (idle.isValid()) ++wakeups;
        });
    }
};
#endif
