#include "page_window_identity.h"
#include "ferric-browser-engine-qt/src/bridge.cxxqt.h"

#include <KWaylandExtras>
#include <QGuiApplication>
#include <QThread>
#include <QJSValue>
#include <QQuickItem>
#include <QQuickWindow>
#include <QUuid>
#include <QWaylandClientExtension>
#include <wayland-client-core.h>

namespace {
// Observe the actual Qt connection's advertised globals. This observer does
// not bind a protocol object or perform a blocking Wayland round trip.
class ToplevelTagAvailability final : public QWaylandClientExtension {
public:
    ToplevelTagAvailability() : QWaylandClientExtension(1) {
        setParent(qGuiApp);
        initialize();
    }
    const wl_interface *extensionInterface() const override {
        static const wl_interface descriptor{"xdg_toplevel_tag_manager_v1", 1, 0, nullptr, 0, nullptr};
        return &descriptor;
    }
    void bind(wl_registry *, int, int version) override { setVersion(qMin(version, 1)); }
};

bool toplevelTagAvailable() {
    static QPointer<ToplevelTagAvailability> available;
    if (!available) available = new ToplevelTagAvailability;
    return available->isActive();
}
}

FerricPageWindowIdentity::FerricPageWindowIdentity(QObject *parent)
    : QObject(parent), tag_(QStringLiteral("ferric-page-")
        + QUuid::createUuid().toString(QUuid::WithoutBraces)) {}

QWindow *FerricPageWindowIdentity::window() const { return window_; }
QString FerricPageWindowIdentity::tag() const { return tag_; }
bool FerricPageWindowIdentity::prepared() const { return prepared_ && window_; }
bool FerricPageWindowIdentity::tagRequested() const { return prepared() && tagRequested_; }

void FerricPageWindowIdentity::setWindow(QWindow *window)
{
    // A marker is never reassigned to a replacement window, and an already
    // mapped surface cannot acquire retrospective grouping authority.
    if (window_ || prepared_ || !window || window->isVisible()
        || window->thread() != QThread::currentThread()) return;
    window_ = window;
    // KF 6.28 installs a surface-role callback that assumes the tag manager
    // exists. Unsupported compositors must keep usable untagged windows.
    if (QGuiApplication::platformName() == QStringLiteral("wayland") && toplevelTagAvailable()) {
        KWaylandExtras::setXdgToplevelTag(window, tag_);
        tagRequested_ = true;
    }
    prepared_ = true;
    connect(window, &QObject::destroyed, this, [this] {
        window_.clear();
        emit windowChanged();
        emit preparedChanged();
    });
    emit windowChanged();
    emit preparedChanged();
}

QStringList ferric_browser_native_page_window_identity(const BrowserUi &ui)
{
    if (!qGuiApp || QThread::currentThread() != qGuiApp->thread()
        || QThread::currentThread() != ui.thread()
        || QGuiApplication::platformName() != QStringLiteral("wayland")) return {};
    const QObject *parent = &ui;
    const QWindow *window = nullptr;
    while (parent && !window) {
        window = qobject_cast<const QWindow *>(parent);
        if (!window) {
            if (const auto *item = qobject_cast<const QQuickItem *>(parent)) window = item->window();
        }
        parent = parent->parent();
    }
    if (!window) return {};
    auto object = [](const QVariant &value) -> QObject * {
        if (auto *valueObject = value.value<QObject *>()) return valueObject;
        return value.value<QJSValue>().toQObject();
    };
    auto *owner = object(window->property("windowUi"));
    if (!owner) owner = object(window->property("browserUi"));
    if (owner != &ui) return {};
    QList<FerricPageWindowIdentity *> identities;
    for (auto *identity : window->findChildren<FerricPageWindowIdentity *>()) {
        if (identity->window() == window) identities.push_back(identity);
    }
    if (identities.size() != 1
        || !identities[0]->prepared() || !identities[0]->tagRequested()) return {};
    auto appId = QGuiApplication::desktopFileName();
    if (appId.isEmpty()) appId = QCoreApplication::applicationName();
    if (appId.endsWith(QStringLiteral(".desktop"))) appId.chop(8);
    const bool foreground = window->property("windowPreparedPageActivateRequested").toBool()
        || (!window->property("windowPreparedPageMode").toBool()
            && window->property("windowInitialVisible").toBool());
    const bool active = window->isActive() && QGuiApplication::focusWindow() == window;
    return {identities[0]->tag(), appId,
            foreground ? QStringLiteral("foreground") : QStringLiteral("background"),
            active ? QStringLiteral("active") : QStringLiteral("inactive")};
}
