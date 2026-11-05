#include "native_wayland.h"

// The only private Qt native-window dependency. Requalify this entire bridge
// (including signal ordering) whenever the qualified SDK changes.
#include <QtGui/qpa/qplatformwindow_p.h>
#include <QGuiApplication>
#include <QPlatformSurfaceEvent>
#include <QThread>
#include <QWaylandClientExtension>
#include <limits>
#include <utility>
#include <wayland-client-core.h>
#include "xdg-foreign-unstable-v2-client.h"
#include "xdg-toplevel-tag-v1-client.h"

static_assert(QT_VERSION == QT_VERSION_CHECK(6, 11, 2),
              "Requalify the private native-window interface for a new Qt SDK");

namespace {
using NativeWindow = QNativeInterface::Private::QWaylandWindow;

bool guiThread() {
    return qGuiApp && QThread::currentThread() == qGuiApp->thread();
}

class Exporter final : public QWaylandClientExtension {
public:
    Exporter() : QWaylandClientExtension(1) { initialize(); }
    ~Exporter() override { retire(); }
    const wl_interface *extensionInterface() const override { return &zxdg_exporter_v2_interface; }
    void bind(wl_registry *registry, int id, int version) override {
        setVersion(qMin(version, 1));
        object = static_cast<zxdg_exporter_v2 *>(wl_registry_bind(registry, id,
            extensionInterface(), this->version()));
    }
    void retire() { if (object) zxdg_exporter_v2_destroy(std::exchange(object, nullptr)); }
    zxdg_exporter_v2 *object = nullptr;
};
class Tags final : public QWaylandClientExtension {
public:
    Tags() : QWaylandClientExtension(1) { initialize(); }
    ~Tags() override { retire(); }
    const wl_interface *extensionInterface() const override { return &xdg_toplevel_tag_manager_v1_interface; }
    void bind(wl_registry *registry, int id, int version) override {
        setVersion(qMin(version, 1));
        object = static_cast<xdg_toplevel_tag_manager_v1 *>(wl_registry_bind(registry, id,
            extensionInterface(), this->version()));
    }
    void retire() { if (object) xdg_toplevel_tag_manager_v1_destroy(std::exchange(object, nullptr)); }
    xdg_toplevel_tag_manager_v1 *object = nullptr;
};
class Context final : public QObject {
public:
    explicit Context(QObject *parent) : QObject(parent) {}
    Exporter exporter;
    Tags tags;
    bool retired = false;
    QList<FerricWaylandWindow *> windows;
};
QPointer<Context> context;
quint64 nextGeneration() {
    static quint64 last = 0;
    if (last == std::numeric_limits<quint64>::max()) return 0;
    return ++last;
}
}

class FerricWaylandWindow::State {
public:
    State(FerricWaylandWindow *owner, QWindow *window, const QString &tag)
        : owner(owner), window(window), tag(tag) {}
    FerricWaylandWindow *owner;
    QPointer<QWindow> window;
    QString tag;
    QPointer<NativeWindow> native;
    wl_surface *surface = nullptr;
    xdg_toplevel *role = nullptr;
    quint64 generation = 0;
    bool submitted = false;
    zxdg_exported_v2 *exported = nullptr;
    QString handle;
    bool exportComplete = false;
    QList<QPointer<FerricWaylandExport>> consumers;

    void complete(FerricExportStatus status) {
        const FerricExportResult result{status, status == FerricExportStatus::Success ? handle : QString(), generation};
        const auto pending = std::exchange(consumers, {});
        for (const auto &consumer : pending)
            if (consumer) consumer->finish(result);
    }
    void invalidate() {
        Q_ASSERT(guiThread());
        const auto previous = generation;
        generation = 0;
        submitted = false;
        surface = nullptr;
        role = nullptr;
        handle.clear();
        exportComplete = false;
        if (exported) zxdg_exported_v2_destroy(std::exchange(exported, nullptr));
        const auto pending = std::exchange(consumers, {});
        QPointer<FerricWaylandWindow> guarded(owner);
        // Authority must disappear before any consumer callback can reenter.
        if (previous) emit owner->invalidated(previous);
        for (const auto &consumer : pending)
            if (consumer) consumer->finish({FerricExportStatus::Invalidated, {}, previous});
        if (previous && guarded) emit guarded->changed();
    }
    void refresh() {
        Q_ASSERT(guiThread());
        if (!native || !context || context->retired) return;
        auto currentSurface = native->surface();
        auto currentRole = native->surfaceRole<xdg_toplevel>();
        if (currentSurface == surface && currentRole == role && generation) return;
        QPointer<FerricWaylandWindow> guarded(owner);
        invalidate();
        if (!guarded || !native || !context || context->retired) return;
        currentSurface = native->surface();
        currentRole = native->surfaceRole<xdg_toplevel>();
        if (generation && currentSurface == surface && currentRole == role) return;
        if (!currentSurface || !currentRole) return;
        surface = currentSurface;
        role = currentRole;
        generation = nextGeneration();
        if (generation && !tag.isEmpty() && context->tags.object && context->tags.isActive()) {
            const auto bytes = tag.toUtf8();
            xdg_toplevel_tag_manager_v1_set_toplevel_tag(context->tags.object, role, bytes.constData());
            submitted = true;
        }
        emit owner->changed();
    }
    void attach() {
        Q_ASSERT(guiThread());
        if (!window || !context || context->retired) return;
        auto current = window->nativeInterface<NativeWindow>();
        if (native != current) {
            QPointer<FerricWaylandWindow> guarded(owner);
            invalidate();
            if (!guarded || !window) return;
            current = window->nativeInterface<NativeWindow>();
            if (native == current) { refresh(); return; }
            if (native) QObject::disconnect(native, nullptr, owner, nullptr);
            native = current;
            if (native) {
                QObject::connect(native, &NativeWindow::surfaceCreated, owner, [this] { refresh(); }, Qt::DirectConnection);
                QObject::connect(native, &NativeWindow::surfaceRoleCreated, owner, [this] { refresh(); }, Qt::DirectConnection);
                QObject::connect(native, &NativeWindow::surfaceDestroyed, owner, [this] { invalidate(); }, Qt::DirectConnection);
                QObject::connect(native, &NativeWindow::surfaceRoleDestroyed, owner, [this] { invalidate(); }, Qt::DirectConnection);
                QObject::connect(native, &QObject::destroyed, owner, [this] { native.clear(); invalidate(); }, Qt::DirectConnection);
            }
        }
        refresh();
    }
    static void received(void *data, zxdg_exported_v2 *object, const char *value) {
        Q_ASSERT(guiThread());
        auto self = static_cast<State *>(data);
        if (self->exported != object || !self->generation) return;
        self->handle = QString::fromUtf8(value);
        self->exportComplete = true;
        self->complete(self->handle.isEmpty() ? FerricExportStatus::Failed : FerricExportStatus::Success);
    }
};

void ferric_initialize_wayland() {
    Q_ASSERT(guiThread());
    if (!guiThread()) return;
    if (!context && QGuiApplication::platformName().startsWith(QStringLiteral("wayland"))) {
        context = new Context(qGuiApp);
        QObject::connect(qGuiApp, &QCoreApplication::aboutToQuit, context, ferric_shutdown_wayland, Qt::DirectConnection);
    }
}
void ferric_shutdown_wayland() {
    Q_ASSERT(guiThread());
    if (!guiThread()) return;
    if (!context || context->retired) return;
    context->retired = true;
    QList<QPointer<FerricWaylandWindow>> windows;
    for (auto window : context->windows) windows.append(window);
    for (auto window : windows) if (window) window->state_->invalidate();
    context->exporter.retire();
    context->tags.retire();
}

FerricWaylandWindow *FerricWaylandWindow::prepare(QWindow *window, const QString &tag) {
    Q_ASSERT(guiThread());
    if (!guiThread() || !window || window->thread() != QThread::currentThread()) return nullptr;
    ferric_initialize_wayland();
    if (auto existing = window->findChild<FerricWaylandWindow *>(QString(), Qt::FindDirectChildrenOnly)) {
        // A mapped role cannot gain retrospective tagging authority.
        if (existing->state_->tag.isEmpty() && !window->isVisible() && !existing->generation()) existing->state_->tag = tag;
        return existing;
    }
    return new FerricWaylandWindow(window, tag);
}
FerricWaylandWindow::FerricWaylandWindow(QWindow *window, const QString &tag)
    : QObject(window), state_(std::make_unique<State>(this, window, tag)) {
    if (context) context->windows.append(this);
    window->installEventFilter(this);
    state_->attach();
}
FerricWaylandWindow::~FerricWaylandWindow() {
    state_->invalidate();
    if (context) context->windows.removeAll(this);
}
quint64 FerricWaylandWindow::generation() const { return state_->generation; }
bool FerricWaylandWindow::tagSubmitted() const { return state_->generation && state_->submitted; }
bool FerricWaylandWindow::eventFilter(QObject *, QEvent *event) {
    if (event->type() == QEvent::PlatformSurface) {
        auto surfaceEvent = static_cast<QPlatformSurfaceEvent *>(event);
        if (surfaceEvent->surfaceEventType() == QPlatformSurfaceEvent::SurfaceCreated) state_->attach();
        else state_->invalidate();
    }
    return false;
}
FerricWaylandExport *FerricWaylandWindow::exportToplevel(QObject *consumer) {
    Q_ASSERT(guiThread());
    if (!guiThread() || !consumer || consumer->thread() != QThread::currentThread()) return nullptr;
    const auto requestedGeneration = generation();
    auto subscription = new FerricWaylandExport(requestedGeneration, consumer);
    state_->consumers.append(subscription);
    connect(subscription, &FerricWaylandExport::completed, this, [this, subscription] {
        state_->consumers.removeAll(subscription);
    });
    connect(subscription, &QObject::destroyed, this, [this] {
        state_->consumers.removeIf([](const auto &item) { return item.isNull(); });
    });
    const QPointer<FerricWaylandWindow> guarded(this);
    // Deferred delivery lets callers install their receiver before completion.
    QMetaObject::invokeMethod(subscription, [guarded, subscription, requestedGeneration] {
        if (subscription->finished()) return;
        if (!guarded || !requestedGeneration || guarded->generation() != requestedGeneration) {
            subscription->finish({FerricExportStatus::Invalidated, {}, requestedGeneration});
            return;
        }
        auto self = guarded.data();
        if (!context || context->retired || !context->exporter.isActive() || !context->exporter.object) {
            subscription->finish({FerricExportStatus::Unsupported, {}, requestedGeneration});
        } else if (self->state_->exportComplete) {
            const auto status = self->state_->handle.isEmpty() ? FerricExportStatus::Failed : FerricExportStatus::Success;
            subscription->finish({status, self->state_->handle, requestedGeneration});
        } else {
            if (!self->state_->exported) {
                self->state_->exported = zxdg_exporter_v2_export_toplevel(context->exporter.object, self->state_->surface);
                if (!self->state_->exported) {
                    self->state_->exportComplete = true;
                    self->state_->complete(FerricExportStatus::Failed);
                    return;
                }
                static const zxdg_exported_v2_listener listener{State::received};
                zxdg_exported_v2_add_listener(self->state_->exported, &listener, self->state_.get());
            }
        }
    }, Qt::QueuedConnection);
    return subscription;
}
