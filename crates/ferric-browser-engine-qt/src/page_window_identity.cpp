#include "page_window_identity.h"
#include "ferric-browser-engine-qt/src/bridge.cxxqt.h"
#include "ferric-browser-engine-qt/src/bridge.cxx.h"

#include "native_wayland.h"
#include <QGuiApplication>
#include <QThread>
#include <QJSValue>
#include <QQuickItem>
#include <QQuickWindow>
#include <QUuid>

FerricPageWindowIdentity::FerricPageWindowIdentity(QObject *parent)
    : QObject(parent), tag_(QStringLiteral("ferric-page-")
        + QUuid::createUuid().toString(QUuid::WithoutBraces)) {}

FerricPageWindowIdentity::~FerricPageWindowIdentity() {
    ferric::page_windows::native_window_retired(tag_);
}

QWindow *FerricPageWindowIdentity::window() const { return window_; }
QString FerricPageWindowIdentity::tag() const { return tag_; }
bool FerricPageWindowIdentity::prepared() const { return prepared_ && window_; }
bool FerricPageWindowIdentity::tagRequested() const { return prepared() && native_ && native_->tagSubmitted(); }

void FerricPageWindowIdentity::setWindow(QWindow *window)
{
    // A marker is never reassigned to a replacement window, and an already
    // mapped surface cannot acquire retrospective grouping authority.
    if (window_ || prepared_ || !window || window->isVisible()
        || window->thread() != QThread::currentThread()) return;
    window_ = window;
    native_ = FerricWaylandWindow::prepare(window, tag_);
    if (native_) {
        connect(native_, &FerricWaylandWindow::invalidated, this, [this](quint64 generation) {
            ferric::page_windows::native_window_invalidated(tag_, generation);
        }, Qt::DirectConnection);
        connect(native_, &FerricWaylandWindow::changed, this, [this] {
            ferric::page_windows::native_window_changed(tag_, native_->generation(), native_->tagSubmitted());
            emit tagRequestedChanged();
        }, Qt::DirectConnection);
        ferric::page_windows::native_window_changed(tag_, native_->generation(), native_->tagSubmitted());
    }
    prepared_ = true;
    connect(window, &QObject::destroyed, this, [this] {
        ferric::page_windows::native_window_retired(tag_);
        window_.clear();
        emit windowChanged();
        emit preparedChanged();
        emit tagRequestedChanged();
    });
    emit windowChanged();
    emit preparedChanged();
    emit tagRequestedChanged();
}

namespace {
struct Host {
    QPointer<QWindow> window;
    QPointer<FerricPageWindowIdentity> marker;
};
Host actualHost(const BrowserUi &ui) {
    if (!qGuiApp || QThread::currentThread() != qGuiApp->thread()
        || QThread::currentThread() != ui.thread()) return {};
    QPointer<BrowserUi> guardedUi(const_cast<BrowserUi *>(&ui));
    const QObject *parent = &ui;
    QPointer<QWindow> window;
    while (parent && !window) {
        window = qobject_cast<QWindow *>(const_cast<QObject *>(parent));
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
    if (!guardedUi || !window) return {};
    if (!owner) owner = object(window->property("browserUi"));
    if (!guardedUi || !window || owner != guardedUi) return {};
    QList<FerricPageWindowIdentity *> identities;
    for (auto *identity : window->findChildren<FerricPageWindowIdentity *>()) {
        if (identity->window() == window) identities.push_back(identity);
    }
    if (identities.size() != 1 || !identities[0]->prepared()) return {};
    return {window, identities[0]};
}
}

QString ferric_browser_page_window_marker(const BrowserUi &ui) {
    const auto host = actualHost(ui);
    return host.marker ? host.marker->tag() : QString();
}

QString FerricPageWindowIdentity::supersedePageFocus() {
    if (!qGuiApp || QThread::currentThread() != qGuiApp->thread()) return {};
    return ferric::page_windows::supersede_page_focus();
}

bool FerricPageWindowIdentity::focusIntentCurrent(const QString &intent) const {
    return qGuiApp && QThread::currentThread() == qGuiApp->thread()
        && ferric::page_windows::page_focus_intent_current(intent);
}
QString FerricPageWindowIdentity::currentPageFocus() const {
    if (!qGuiApp || QThread::currentThread() != qGuiApp->thread()) return {};
    return ferric::page_windows::current_page_focus();
}

QString FerricPageWindowIdentity::activatePage(QObject *issuerObject, QObject *targetObject, const QString &nonce) {
    QPointer<FerricPageWindowIdentity> self(this);
    QPointer<BrowserUi> issuer(qobject_cast<BrowserUi *>(issuerObject));
    QPointer<BrowserUi> target(qobject_cast<BrowserUi *>(targetObject));
    QPointer<QWindow> native(window_);
    auto current = [&](uint8_t phase) {
        if (!self || !issuer || !target || !native || !qGuiApp
            || QThread::currentThread() != qGuiApp->thread() || issuer->thread() != QThread::currentThread()) return false;
        const auto host = actualHost(*target);
        if (!self || !issuer || !target || !host.marker || !host.window
            || host.marker != self || host.window != native) return false;
        // This final comparison is pure Rust/Core. No QObject call follows it
        // before the corresponding real native window operation.
        return ferric::page_windows::validate_page_focus(*issuer, *target, nonce, self->tag(), phase);
    };
    if (!current(0) || !current(1)) return QStringLiteral("stale");
    native->show();
    if (!current(2)) return QStringLiteral("stale");
    native->raise();
    if (!current(2)) return QStringLiteral("stale");
    native->requestActivate();
    return self && issuer && target && native ? QStringLiteral("requested") : QStringLiteral("stale");
}

QString FerricPageWindowIdentity::pageFocusStatus(QObject *issuerObject, QObject *targetObject, const QString &nonce) const {
    QPointer<const FerricPageWindowIdentity> self(this);
    QPointer<BrowserUi> issuer(qobject_cast<BrowserUi *>(issuerObject));
    QPointer<BrowserUi> target(qobject_cast<BrowserUi *>(targetObject));
    if (!issuer || !target || !qGuiApp || QThread::currentThread() != qGuiApp->thread()
        || issuer->thread() != QThread::currentThread()) return QStringLiteral("stale");
    const auto host = actualHost(*target);
    if (!self || !issuer || !target || !host.marker || !host.window || host.marker.data() != self.data()
        || !ferric::page_windows::validate_page_focus(*issuer, *target, nonce, self->tag(), 2)) return QStringLiteral("stale");
    return host.window->isActive() && QGuiApplication::focusWindow() == host.window
        ? QStringLiteral("activated") : QStringLiteral("requested");
}

ferric::page_windows::NativeWindowSnapshot ferric_browser_native_page_window_identity(const BrowserUi &ui)
{
    if (QGuiApplication::platformName() != QStringLiteral("wayland")) return {};
    const auto host = actualHost(ui);
    if (!host.marker || !host.window) return {};
    const QPointer<FerricWaylandWindow> native = FerricWaylandWindow::prepare(host.window);
    if (!native || !native->generation()) return {};
    const auto generation = native->generation();
    const auto window = host.window;
    const auto current = [&] {
        return window && host.marker && native && native->generation() == generation;
    };
    auto appId = QGuiApplication::desktopFileName();
    if (appId.isEmpty()) appId = QCoreApplication::applicationName();
    if (appId.endsWith(QStringLiteral(".desktop"))) appId.chop(8);
    const bool activateRequested = window->property("windowPreparedPageActivateRequested").toBool();
    if (!current()) return {};
    const bool preparedMode = window->property("windowPreparedPageMode").toBool();
    if (!current()) return {};
    const bool initiallyVisible = window->property("windowInitialVisible").toBool();
    if (!current()) return {};
    const bool foreground = activateRequested || (!preparedMode && initiallyVisible);
    const bool active = window->isActive() && QGuiApplication::focusWindow() == window;
    return {true, host.marker->tag().toStdString(), appId.toStdString(), generation,
            foreground, active, native->tagSubmitted()};
}
