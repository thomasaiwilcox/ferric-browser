#include "browser_page_pointer_adapter.h"

#include <QtCore/QCoreApplication>
#include <QtCore/QEvent>
#include <QtCore/QTimer>
#include <QtGui/QCursor>
#include <QtGui/QGuiApplication>
#include <QtGui/QMouseEvent>
#include <QtGui/QWheelEvent>
#include <QtQuick/QQuickWindow>
#include <cmath>
#include <limits>

namespace {
constexpr int kMaxRequestBytes = 128;
constexpr qsizetype kMaxConsumedRequestIds = 64;
}

FerricPagePointerAdapter::FerricPagePointerAdapter(QObject *parent)
    : QObject(parent)
{
    dispatchWatchdog_.setSingleShot(true);
    dispatchWatchdog_.setInterval(2000);
    connect(&dispatchWatchdog_, &QTimer::timeout, this, [this] {
        if (pendingRequestId_.isEmpty())
            return;
        pendingRequestId_.clear();
        pendingSessionId_.clear();
        emit invalidated(QStringLiteral("dispatch-uncertain"));
    });
    if (auto *application = QCoreApplication::instance())
        application->installEventFilter(this);
}

FerricPagePointerAdapter::~FerricPagePointerAdapter()
{
    if (auto *application = QCoreApplication::instance())
        application->removeEventFilter(this);
}

void FerricPagePointerAdapter::setEnabled(bool enabled)
{
    if (enabled_ == enabled)
        return;
    enabled_ = enabled;
    if (!enabled_) {
        dispatchWatchdog_.stop();
        pendingRequestId_.clear();
        pendingSessionId_.clear();
        consumedRequestIds_.clear();
        consumedRequestOrder_.clear();
        consumedSessionId_.clear();
        consumedButtons_.clear();
    }
    emit enabledChanged();
}

void FerricPagePointerAdapter::setTargetItem(QQuickItem *item)
{
    if (targetItem_ == item)
        return;
    if (targetItem_)
        targetItem_->removeEventFilter(this);
    targetItem_ = item;
    if (targetItem_) {
        targetItem_->installEventFilter(this);
        connect(targetItem_, &QObject::destroyed, this, [this] {
            advanceSurface(true, QStringLiteral("target-changed"));
            emit targetItemChanged();
        });
    }
    advanceSurface(true, QStringLiteral("target-changed"));
    emit targetItemChanged();
}

void FerricPagePointerAdapter::setTargetWindow(QWindow *window)
{
    if (targetWindow_ == window)
        return;
    if (targetWindow_)
        targetWindow_->removeEventFilter(this);
    targetWindow_ = window;
    if (targetWindow_) {
        targetWindow_->installEventFilter(this);
        connect(targetWindow_, &QObject::destroyed, this, [this] {
            advanceSurface(true, QStringLiteral("window-changed"));
            emit targetWindowChanged();
        });
    }
    advanceSurface(true, QStringLiteral("window-changed"));
    emit targetWindowChanged();
}

void FerricPagePointerAdapter::setInputBlocked(bool blocked)
{
    if (inputBlocked_ == blocked)
        return;
    inputBlocked_ = blocked;
    emit inputBlockedChanged();
    if (blocked)
        advanceSurface(false, QStringLiteral("prompt-owned-input"));
}

void FerricPagePointerAdapter::advanceSurface(bool targetChanged, const QString &reason)
{
    dispatchWatchdog_.stop();
    pendingRequestId_.clear();
    pendingSessionId_.clear();
    surfaceRevision_ = surfaceRevision_ == std::numeric_limits<quint64>::max()
        ? surfaceRevision_
        : surfaceRevision_ + 1;
    if (targetChanged)
        surfaceSerial_ = surfaceSerial_ == std::numeric_limits<quint64>::max()
            ? surfaceSerial_
            : surfaceSerial_ + 1;
    emit surfaceChanged();
    if (enabled_)
        emit invalidated(reason);
}

bool FerricPagePointerAdapter::validTarget() const
{
    return targetItem_ && targetWindow_
        && static_cast<QWindow *>(targetItem_->window()) == targetWindow_.data()
        && targetItem_->isVisible() && targetWindow_->isVisible()
        && targetWindow_->isActive() && targetItem_->width() > 0.0
        && targetItem_->height() > 0.0 && !inputBlocked_;
}

void FerricPagePointerAdapter::acknowledge(const QString &requestId,
                                           const QString &sessionId,
                                           const QString &serial,
                                           const QString &revision,
                                           const QString &outcome)
{
    if (pendingRequestId_ == requestId && pendingSessionId_ == sessionId) {
        dispatchWatchdog_.stop();
        pendingRequestId_.clear();
        pendingSessionId_.clear();
    }
    emit dispatchAcknowledged(requestId, sessionId, serial, revision, outcome);
}

void FerricPagePointerAdapter::dispatch(const QString &requestId,
                                        const QString &sessionId,
                                        const QString &serialText,
                                        const QString &revisionText,
                                        double x,
                                        double y,
                                        const QString &action)
{
    bool serialOk = false;
    bool revisionOk = false;
    const quint64 serial = serialText.toULongLong(&serialOk);
    const quint64 revision = revisionText.toULongLong(&revisionOk);
    if (requestId.isEmpty() || requestId.size() > kMaxRequestBytes || sessionId.isEmpty()
        || sessionId.size() > kMaxRequestBytes || consumedRequestIds_.contains(requestId)) {
        acknowledge(requestId, sessionId, serialText, revisionText, QStringLiteral("rejected"));
        return;
    }
    while (consumedRequestOrder_.size() >= kMaxConsumedRequestIds) {
        const auto expired = consumedRequestOrder_.dequeue();
        consumedRequestIds_.remove(expired);
    }
    consumedRequestIds_.insert(requestId);
    consumedRequestOrder_.enqueue(requestId);
    consumedSessionId_ = sessionId;
    if (!serialOk || !revisionOk || !enabled_ || !validTarget() || serial != surfaceSerial_
        || revision != surfaceRevision_
        || !std::isfinite(x) || !std::isfinite(y) || x < 0.0 || y < 0.0
        || x >= targetItem_->width() || y >= targetItem_->height()
        || (action != QStringLiteral("hover") && action != QStringLiteral("left")
            && action != QStringLiteral("right") && action != QStringLiteral("middle"))) {
        acknowledge(requestId, sessionId, serialText, revisionText, QStringLiteral("rejected"));
        return;
    }

    const QPointF local(x, y);
    const QPointF scene = targetItem_->mapToScene(local);
    const QPoint global = targetWindow_->mapToGlobal(scene.toPoint());
    const Qt::MouseButton button = action == QStringLiteral("right")
        ? Qt::RightButton
        : action == QStringLiteral("middle") ? Qt::MiddleButton : Qt::LeftButton;
    const bool click = action != QStringLiteral("hover");
    pendingRequestId_ = requestId;
    pendingSessionId_ = sessionId;
    dispatchWatchdog_.start();
    dispatching_ = true;
    QMouseEvent move(QEvent::MouseMove, local, scene, global, Qt::NoButton, Qt::NoButton,
                     Qt::NoModifier, Qt::MouseEventNotSynthesized);
    const bool moved = QCoreApplication::sendEvent(targetItem_, &move);
    bool delivered = moved;
    if (click && moved) {
        QMouseEvent press(QEvent::MouseButtonPress, local, scene, global, button, button,
                          Qt::NoModifier, Qt::MouseEventNotSynthesized);
        QMouseEvent release(QEvent::MouseButtonRelease, local, scene, global, button,
                            Qt::NoButton, Qt::NoModifier, Qt::MouseEventNotSynthesized);
        const bool pressed = QCoreApplication::sendEvent(targetItem_, &press);
        const bool released = QCoreApplication::sendEvent(targetItem_, &release);
        delivered = pressed && released;
    }
    dispatching_ = false;
    acknowledge(requestId, sessionId, serialText, revisionText,
                delivered ? QStringLiteral("delivered") : QStringLiteral("rejected"));
}

bool FerricPagePointerAdapter::eventFilter(QObject *watched, QEvent *event)
{
    if (dispatching_)
        return false;
    if (watched == targetItem_) {
        switch (event->type()) {
        case QEvent::Resize:
        case QEvent::ParentChange:
        case QEvent::WindowChangeInternal:
            advanceSurface(false, QStringLiteral("geometry-changed"));
            break;
        default:
            break;
        }
    }
    if (watched == targetWindow_) {
        switch (event->type()) {
        case QEvent::WindowDeactivate:
        case QEvent::Hide:
            advanceSurface(false, QStringLiteral("window-inactive"));
            break;
        default:
            break;
        }
    }
    if (!enabled_ || !targetWindow_ || QGuiApplication::focusWindow() != targetWindow_)
        return false;
    switch (event->type()) {
    case QEvent::MouseMove:
        dispatchWatchdog_.stop();
        pendingRequestId_.clear();
        pendingSessionId_.clear();
        emit physicalPointerDetected(QStringLiteral("physical-pointer"));
        return false;
    case QEvent::MouseButtonPress: {
        const auto *mouse = static_cast<QMouseEvent *>(event);
        consumedButtons_.insert(int(mouse->button()));
        dispatchWatchdog_.stop();
        pendingRequestId_.clear();
        pendingSessionId_.clear();
        emit physicalPointerDetected(QStringLiteral("physical-pointer"));
        return true;
    }
    case QEvent::MouseButtonRelease: {
        const auto *mouse = static_cast<QMouseEvent *>(event);
        if (consumedButtons_.remove(int(mouse->button())) > 0)
            return true;
        return false;
    }
    case QEvent::Wheel:
    case QEvent::TabletMove:
    case QEvent::TabletPress:
    case QEvent::TouchBegin:
        dispatchWatchdog_.stop();
        pendingRequestId_.clear();
        pendingSessionId_.clear();
        emit physicalPointerDetected(QStringLiteral("physical-pointer"));
        return true;
    default:
        return false;
    }
}
