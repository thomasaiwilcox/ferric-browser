#include "quick_credential_owner.h"
#include "credential_boundary.h"

#include <QQmlComponent>
#include <QQmlContext>
#include <QQmlEngine>
#include <QQmlProperty>
#include <QQuickWindow>
#include <QMetaMethod>
#include <QCoreApplication>
#include <QEvent>
#include <QThread>
#include <QWebChannel>
#include <QWebEngineLoadingInfo>
#include <QWebEngineScript>
#include <QWindow>
#include <QtWebEngineQuick/QQuickWebEngineProfile>
#include <limits>
#include <utility>

namespace {
quint64 nextIncarnation = 1;
quint64 incarnation()
{
    if (nextIncarnation == std::numeric_limits<quint64>::max()) return 0;
    return nextIncarnation++;
}
}

FerricQuickCredentialOwner *FerricQuickCredentialOwner::create(QQuickItem *view)
{
    if (!view || QThread::currentThread() != view->thread()) return nullptr;
    if (auto *existing = view->findChild<FerricQuickCredentialOwner *>(QString(), Qt::FindDirectChildrenOnly))
        return existing->isReady() ? existing : nullptr;
    auto *owner = new FerricQuickCredentialOwner(view);
    if (owner->isReady()) return owner;
    delete owner;
    return nullptr;
}

FerricQuickCredentialOwner::FerricQuickCredentialOwner(QQuickItem *view)
    : QObject(view), view_(view)
{
    if (!qmlEngine(view) || !qmlContext(view)) return;
    profile_ = view->property("profile").value<QObject *>();
    if (!qobject_cast<QQuickWebEngineProfile *>(profile_.data())) return;
    const auto frame = view->property("mainFrame");
    if (!frame.canConvert<QWebEngineFrame>()) return;
    operation_ = {incarnation(), incarnation(), incarnation(), 0, 1};
    if (!operation_.profileGeneration || !operation_.pageGeneration || !operation_.hostGeneration) return;
    if (!connect(view, SIGNAL(loadingChanged(QWebEngineLoadingInfo)),
                 this, SLOT(loadingChanged(QWebEngineLoadingInfo)))) return;
    // Its public QML signal carries a Quick enum that has no public C++
    // header. Connect the documented signal through Qt's public meta API;
    // ignoring the arguments fences every termination status conservatively.
    QMetaMethod terminated;
    for (int index = 0; index < view->metaObject()->methodCount(); ++index) {
        const auto method = view->metaObject()->method(index);
        if (method.methodType() == QMetaMethod::Signal && method.name() == "renderProcessTerminated"
            && method.parameterCount() == 2) terminated = method;
    }
    const auto terminationSlot = metaObject()->method(metaObject()->indexOfSlot("rendererTerminated()"));
    if (!terminated.isValid() || !QObject::connect(view, terminated, this, terminationSlot)) return;
    for (const auto &[name, slot] : {
        std::pair{"profile", SLOT(profileChanged())},
        std::pair{"lifecycleState", SLOT(lifecycleChanged())},
        std::pair{"renderProcessPid", SLOT(rendererChanged())},
        std::pair{"webChannel", SLOT(channelChanged())},
        std::pair{"webChannelWorld", SLOT(channelChanged())}}) {
        if (!QQmlProperty(view, QString::fromLatin1(name)).connectNotifySignal(this, slot)) return;
    }
    connect(profile_, &QObject::destroyed, this, &FerricQuickCredentialOwner::retire);
    connect(QCoreApplication::instance(), &QCoreApplication::aboutToQuit,
            this, &FerricQuickCredentialOwner::presentationChanged);
    connect(view, &QQuickItem::windowChanged, this, [this] { watchWindow(); presentationChanged(); });
    connect(view, &QQuickItem::parentChanged, this, &FerricQuickCredentialOwner::presentationChanged);
    connect(view, &QQuickItem::visibleChanged, this, &FerricQuickCredentialOwner::presentationChanged);
    renderer_ = view->property("renderProcessPid").toLongLong();
    watchWindow();
    ready_ = true;
}

FerricQuickCredentialOwner::~FerricQuickCredentialOwner()
{
    view_.clear();
    retire();
}

bool FerricQuickCredentialOwner::isReady() const
{
    return ready_ && !retired_ && view_ && profile_ && QThread::currentThread() == thread()
        && view_->property("profile").value<QObject *>() == profile_;
}

FerricQuickCredentialOwner::Operation FerricQuickCredentialOwner::captureOperation() const { return operation_; }

bool FerricQuickCredentialOwner::isCurrent(const Operation &value) const
{
    return isReady() && operation_.profileGeneration == value.profileGeneration
        && operation_.pageGeneration == value.pageGeneration && operation_.hostGeneration == value.hostGeneration
        && operation_.documentGeneration == value.documentGeneration
        && operation_.presentationGeneration == value.presentationGeneration;
}

QWebEngineFrame FerricQuickCredentialOwner::mainFrame() const
{
    if (!isReady()) return {};
    const auto frame = view_->property("mainFrame").value<QWebEngineFrame>();
    return frame.isValid() && frame.isMainFrame() ? frame : QWebEngineFrame();
}

QWindow *FerricQuickCredentialOwner::presentationWindow() const { return isReady() ? window_.data() : nullptr; }

void FerricQuickCredentialOwner::retire()
{
    if (retired_) return;
    retired_ = true;
    emit authorityInvalidated();
}

void FerricQuickCredentialOwner::invalidateDocument()
{
    if (operation_.documentGeneration == std::numeric_limits<quint64>::max()) { retire(); return; }
    ++operation_.documentGeneration;
    emit authorityInvalidated();
}

void FerricQuickCredentialOwner::loadingChanged(const QWebEngineLoadingInfo &info)
{
    if (info.status() == QWebEngineLoadingInfo::LoadStartedStatus) invalidateDocument();
    if (info.status() == QWebEngineLoadingInfo::LoadSucceededStatus) {
        if (auto *boundary = findChild<FerricCredentialBoundary *>(QString(), Qt::FindDirectChildrenOnly))
            boundary->ensureMediatorInCurrentDocument();
    }
}
void FerricQuickCredentialOwner::profileChanged() { retire(); }
void FerricQuickCredentialOwner::lifecycleChanged()
{
    if (view_ && view_->property("lifecycleState").toInt() == 2) invalidateDocument();
}
void FerricQuickCredentialOwner::rendererChanged()
{
    const auto pid = view_ ? view_->property("renderProcessPid").toLongLong() : 0;
    if (pid != renderer_) { renderer_ = pid; invalidateDocument(); }
}
void FerricQuickCredentialOwner::rendererTerminated() { invalidateDocument(); }
void FerricQuickCredentialOwner::channelChanged()
{
    if (!installing_ && channel_ && !channelMatches(channel_)) retire();
}
void FerricQuickCredentialOwner::presentationChanged()
{
    if (operation_.presentationGeneration == std::numeric_limits<quint64>::max()) { retire(); return; }
    ++operation_.presentationGeneration;
    emit authorityInvalidated();
}
void FerricQuickCredentialOwner::watchWindow()
{
    if (window_) window_->removeEventFilter(this);
    for (const auto &connection : std::as_const(windowConnections_)) disconnect(connection);
    windowConnections_.clear();
    window_ = view_ ? view_->window() : nullptr;
    if (window_) {
        window_->installEventFilter(this);
        windowConnections_.append(connect(window_, &QWindow::visibleChanged, this, &FerricQuickCredentialOwner::presentationChanged));
        windowConnections_.append(connect(window_, &QObject::destroyed, this, &FerricQuickCredentialOwner::presentationChanged));
    }
}

bool FerricQuickCredentialOwner::eventFilter(QObject *watched, QEvent *event)
{
    // Revoke before QML's unsaved-work/last-window guard can reject the close.
    // A cancelled close leaves the document usable for a fresh request.
    if (watched == window_ && event->type() == QEvent::Close) {
        const QPointer<FerricQuickCredentialOwner> self(this);
        const QPointer<QObject> host(watched);
        presentationChanged();
        // Provider cancellation may synchronously destroy this adapter or its
        // host. Qt must not deliver the Close event to a deleted receiver.
        if (!self || !host) return true;
        return false;
    }
    return QObject::eventFilter(watched, event);
}

QWebChannel *FerricQuickCredentialOwner::createChannel(QObject *parent)
{
    if (!isReady()) return nullptr;
    // Quick's documented getter lazily creates an empty, view-owned channel.
    // Reuse only that empty instance; never replace another feature's bridge.
    if (auto *existing = qobject_cast<QWebChannel *>(view_->property("webChannel").value<QObject *>())) {
        return existing->parent() == view_ && existing->registeredObjects().isEmpty()
            ? existing : nullptr;
    }
    QQmlComponent component(qmlEngine(view_));
    component.setData("import QtWebChannel; WebChannel {}", QUrl("qrc:/ferric-native-credential-channel.qml"));
    auto *object = component.create(qmlContext(view_));
    auto *channel = qobject_cast<QWebChannel *>(object);
    if (!channel) { delete object; return nullptr; }
    QQmlEngine::setObjectOwnership(channel, QQmlEngine::CppOwnership);
    channel->setParent(parent);
    return channel;
}

bool FerricQuickCredentialOwner::channelMatches(QWebChannel *channel) const
{
    return isReady() && view_->property("webChannelWorld").toUInt() == FerricCredentialBoundary::World
        && view_->property("webChannel").value<QObject *>() == channel;
}

bool FerricQuickCredentialOwner::installChannel(QWebChannel *channel, const QWebEngineScript &script)
{
    if (!isReady() || !channel || channel_) return false;
    const auto *existing = view_->property("webChannel").value<QObject *>();
    if (existing && existing != channel) return false;
    const auto objects = channel->registeredObjects();
    if (objects.size() != 1 || !qobject_cast<FerricCredentialNotification *>(objects.value("credentialNotification"))) return false;
    auto *scripts = view_->property("userScripts").value<QObject *>();
    if (!scripts) return false;
    installing_ = true;
    const bool installed = view_->setProperty("webChannelWorld", FerricCredentialBoundary::World)
        && QQmlProperty::write(view_, "webChannel", QVariant::fromValue(static_cast<QObject *>(channel)))
        && QMetaObject::invokeMethod(scripts, "insert", Qt::DirectConnection, Q_ARG(QWebEngineScript, script));
    if (installed) channel_ = channel;
    else QQmlProperty::write(view_, "webChannel", QVariant::fromValue(static_cast<QObject *>(nullptr)));
    installing_ = false;
    return installed && channelMatches(channel);
}

void FerricQuickCredentialOwner::removeChannel(QWebChannel *channel, const QWebEngineScript &script)
{
    if (!channelMatches(channel)) return;
    installing_ = true;
    if (auto *scripts = view_->property("userScripts").value<QObject *>()) {
        bool removed = false;
        QMetaObject::invokeMethod(scripts, "remove", Qt::DirectConnection,
                                  Q_RETURN_ARG(bool, removed), Q_ARG(QWebEngineScript, script));
    }
    QQmlProperty::write(view_, "webChannel", QVariant::fromValue(static_cast<QObject *>(nullptr)));
    channel_.clear();
    installing_ = false;
}

FerricCredentialBoundary *FerricQuickCredentialOwner::prepareBoundary(std::function<QString()> contextIdentity)
{
    if (!isReady() || !presentationWindow() || !contextIdentity || contextIdentity().isEmpty()) return nullptr;
    auto *boundary = findChild<FerricCredentialBoundary *>(QString(), Qt::FindDirectChildrenOnly);
    if (boundary) boundary->bindPresentation(presentationWindow(), std::move(contextIdentity));
    else boundary = new FerricCredentialBoundary(this, presentationWindow(), std::move(contextIdentity));
    if (boundary->isReady()) {
        boundary->ensureMediatorInCurrentDocument();
        return boundary;
    }
    delete boundary;
    return nullptr;
}
