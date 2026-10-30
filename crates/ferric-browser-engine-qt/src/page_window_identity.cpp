#include "page_window_identity.h"

#include <KWaylandExtras>
#include <QGuiApplication>
#include <QThread>
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
