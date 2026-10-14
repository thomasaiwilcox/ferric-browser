#include "browser_page_pointer_adapter.h"

#include <QtCore/QCoreApplication>
#include <QtCore/QEvent>
#include <QtCore/QTimer>
#include <QtGui/QGuiApplication>
#include <QtGui/QMouseEvent>
#include <QtQuick/QQuickWindow>
#include <array>
#include <cmath>
#include <limits>

namespace {
constexpr int kMaxRequestCharacters = 128;
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
    }
    emit enabledChanged();
}

void FerricPagePointerAdapter::setTargetItem(QQuickItem *item)
{
    if (targetItem_ == item)
        return;
    if (targetItem_) {
        targetItem_->removeEventFilter(this);
        QObject::disconnect(targetItem_.data(), nullptr, this, nullptr);
    }
    targetItem_ = item;
    if (targetItem_) {
        targetItem_->installEventFilter(this);
        connect(targetItem_, &QObject::destroyed, this, [this] {
            targetItem_.clear();
            advanceSurface(true, QStringLiteral("target-changed"));
            emit targetItemChanged();
        });
        connect(targetItem_, &QQuickItem::widthChanged, this, [this] {
            advanceSurface(false, QStringLiteral("geometry-changed"));
        });
        connect(targetItem_, &QQuickItem::heightChanged, this, [this] {
            advanceSurface(false, QStringLiteral("geometry-changed"));
        });
        connect(targetItem_, &QQuickItem::visibleChanged, this, [this] {
            advanceSurface(false, QStringLiteral("geometry-changed"));
        });
    }
    advanceSurface(true, QStringLiteral("target-changed"));
    emit targetItemChanged();
}

void FerricPagePointerAdapter::setTargetWindow(QWindow *window)
{
    if (targetWindow_ == window)
        return;
    if (targetWindow_) {
        targetWindow_->removeEventFilter(this);
        QObject::disconnect(targetWindow_.data(), nullptr, this, nullptr);
    }
    targetWindow_ = window;
    if (targetWindow_) {
        targetWindow_->installEventFilter(this);
        connect(targetWindow_, &QObject::destroyed, this, [this] {
            targetWindow_.clear();
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

void FerricPagePointerAdapter::setDoubleClickEnabled(bool enabled)
{
    if (doubleClickEnabled_ == enabled)
        return;
    doubleClickEnabled_ = enabled;
    emit doubleClickEnabledChanged();
}

void FerricPagePointerAdapter::advanceSurface(bool targetChanged, const QString &reason)
{
    dispatchWatchdog_.stop();
    pendingRequestId_.clear();
    pendingSessionId_.clear();
    if (surfaceExhausted_ || surfaceRevision_ == std::numeric_limits<quint64>::max()
        || (targetChanged && surfaceSerial_ == std::numeric_limits<quint64>::max())) {
        surfaceExhausted_ = true;
        emit surfaceChanged();
        if (enabled_)
            emit invalidated(QStringLiteral("surface-unavailable"));
        return;
    }
    ++surfaceRevision_;
    if (targetChanged)
        ++surfaceSerial_;
    emit surfaceChanged();
    if (enabled_)
        emit invalidated(reason);
}

bool FerricPagePointerAdapter::validTarget() const
{
    return !surfaceExhausted_ && targetItem_ && targetWindow_
        && static_cast<QWindow *>(targetItem_->window()) == targetWindow_.data()
        && targetItem_->isVisible() && targetWindow_->isVisible()
        && targetWindow_->isActive() && std::isfinite(targetItem_->width())
        && std::isfinite(targetItem_->height()) && targetItem_->width() > 0.0
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
    serialOk = serialOk && serial != 0 && serialText == QString::number(serial);
    revisionOk = revisionOk && revision != 0 && revisionText == QString::number(revision);
    if (requestId.isEmpty() || requestId.size() > kMaxRequestCharacters || sessionId.isEmpty()
        || sessionId.size() > kMaxRequestCharacters
        || (!consumedSessionId_.isEmpty() && consumedSessionId_ != sessionId)
        || consumedRequestIds_.contains(requestId)) {
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
            && action != QStringLiteral("right") && action != QStringLiteral("middle")
            && action != QStringLiteral("double"))) {
        acknowledge(requestId, sessionId, serialText, revisionText, QStringLiteral("rejected"));
        return;
    }
    if (action == QStringLiteral("double") && !doubleClickEnabled_) {
        acknowledge(requestId, sessionId, serialText, revisionText,
                    QStringLiteral("unavailable"));
        return;
    }

    // In-scene Hint/Grid overlays can still own keyboard focus. A native
    // button gesture must focus the page just as a direct mouse press would;
    // trusted events alone do not satisfy focus-sensitive page API gates.
    if (action != QStringLiteral("hover")) {
        const QPointer<QQuickItem> focusTarget = targetItem_;
        const QPointer<QWindow> focusWindow = targetWindow_;
        focusTarget->forceActiveFocus(Qt::MouseFocusReason);
        if (!focusTarget || !focusWindow || targetItem_ != focusTarget
            || targetWindow_ != focusWindow || !validTarget()
            || serial != surfaceSerial_ || revision != surfaceRevision_) {
            acknowledge(requestId, sessionId, serialText, revisionText, QStringLiteral("rejected"));
            return;
        }
    }

    const QPointF targetLocal(x, y);
    const QPointF windowPosition = targetItem_->mapToScene(targetLocal);
    const std::array<QPointF, 4> localCorners = {
        QPointF(0, 0), QPointF(targetItem_->width(), 0),
        QPointF(0, targetItem_->height()),
        QPointF(targetItem_->width(), targetItem_->height())
    };
    std::array<QPointF, 4> sceneCorners;
    bool mappingFinite = std::isfinite(windowPosition.x()) && std::isfinite(windowPosition.y());
    for (std::size_t index = 0; index < localCorners.size(); ++index) {
        sceneCorners[index] = targetItem_->mapToScene(localCorners[index]);
        mappingFinite = mappingFinite && std::isfinite(sceneCorners[index].x())
            && std::isfinite(sceneCorners[index].y());
    }
    const QRect windowGeometry = targetWindow_->geometry();
    const qreal devicePixelRatio = targetWindow_->devicePixelRatio();
    if (!mappingFinite || !std::isfinite(devicePixelRatio) || devicePixelRatio <= 0.0
        || windowPosition.x() < 0.0 || windowPosition.y() < 0.0
        || windowPosition.x() >= windowGeometry.width()
        || windowPosition.y() >= windowGeometry.height()) {
        acknowledge(requestId, sessionId, serialText, revisionText, QStringLiteral("rejected"));
        return;
    }
    const QPoint global = targetWindow_->mapToGlobal(windowPosition.toPoint());
    const bool doubleClick = action == QStringLiteral("double");
    const Qt::MouseButton button = action == QStringLiteral("right")
        ? Qt::RightButton
        : action == QStringLiteral("middle") ? Qt::MiddleButton : Qt::LeftButton;
    const bool click = action != QStringLiteral("hover");
    const QPointer<QQuickItem> dispatchTarget = targetItem_;
    const QPointer<QWindow> dispatchWindow = targetWindow_;
    pendingRequestId_ = requestId;
    pendingSessionId_ = sessionId;
    dispatchWatchdog_.start();
    dispatching_ = true;
    // QQuickWindow owns pointer hit testing and delivery to the render item
    // beneath this view-local point. Sending directly to the outer
    // WebEngineView bypasses that delivery path because QQuickItem accepts no
    // mouse buttons by default.
    QMouseEvent move(QEvent::MouseMove, windowPosition, windowPosition, global,
                     Qt::NoButton, Qt::NoButton,
                     Qt::NoModifier, Qt::MouseEventSynthesizedByApplication);
    const bool moved = dispatchTarget && dispatchWindow
        && dispatchTarget->window() == dispatchWindow.data()
        && QCoreApplication::sendEvent(dispatchWindow.data(), &move);
    const auto sameTarget = [&] {
        return dispatchTarget && dispatchWindow && targetItem_ == dispatchTarget
            && targetWindow_ == dispatchWindow
            && dispatchTarget->window() == dispatchWindow.data();
    };
    const auto currentSurface = [&] {
        if (!sameTarget() || !enabled_ || inputBlocked_ || !validTarget()
            || surfaceSerial_ != serial || surfaceRevision_ != revision
            || pendingRequestId_ != requestId || pendingSessionId_ != sessionId
            || dispatchWindow->geometry() != windowGeometry
            || dispatchWindow->devicePixelRatio() != devicePixelRatio
            || dispatchWindow->mapToGlobal(windowPosition.toPoint()) != global)
            return false;
        // Compare the effective mapping, including ancestor and custom transforms.
        // The selected point alone can stay fixed while a view rotates or scales.
        for (std::size_t index = 0; index < localCorners.size(); ++index) {
            const QPointF current = dispatchTarget->mapToScene(localCorners[index]);
            if (current.x() != sceneCorners[index].x() || current.y() != sceneCorners[index].y())
                return false;
        }
        const QPointF current = dispatchTarget->mapToScene(targetLocal);
        return current.x() == windowPosition.x() && current.y() == windowPosition.y();
    };
    bool pressAttempted = false;
    bool delivered = moved && currentSurface();
    if (click && delivered) {
        QMouseEvent press(QEvent::MouseButtonPress, windowPosition, windowPosition, global,
                          button, button,
                          Qt::NoModifier, Qt::MouseEventSynthesizedByApplication);
        QMouseEvent release(QEvent::MouseButtonRelease, windowPosition, windowPosition,
                            global, button, Qt::NoButton, Qt::NoModifier,
                            Qt::MouseEventSynthesizedByApplication);
        // The move can synchronously change chrome, ownership or geometry.
        // Never begin a button gesture after an observed invalidation.
        pressAttempted = currentSurface();
        const bool pressed = pressAttempted
            && QCoreApplication::sendEvent(dispatchWindow.data(), &press);
        // Only clean up a gesture we attempted, on its unchanged receiver.
        const bool released = pressAttempted && sameTarget()
            && QCoreApplication::sendEvent(dispatchWindow.data(), &release);
        delivered = pressed && released && currentSurface();
        if (delivered && doubleClick) {
            if (!currentSurface()) {
                delivered = false;
            } else {
                QMouseEvent secondPress(QEvent::MouseButtonPress, windowPosition,
                                        windowPosition, global, button, button,
                                        Qt::NoModifier,
                                        Qt::MouseEventSynthesizedByApplication);
                const bool secondPressAttempted = currentSurface();
                const bool secondPressed = secondPressAttempted
                    && QCoreApplication::sendEvent(dispatchWindow.data(), &secondPress);
                QMouseEvent doubleClickEvent(QEvent::MouseButtonDblClick, windowPosition,
                                             windowPosition, global, button, button,
                                             Qt::NoModifier,
                                             Qt::MouseEventSynthesizedByApplication);
                const bool doubleClicked = secondPressed && currentSurface()
                    && QCoreApplication::sendEvent(dispatchWindow.data(), &doubleClickEvent);
                QMouseEvent secondRelease(QEvent::MouseButtonRelease, windowPosition,
                                          windowPosition, global, button, Qt::NoButton,
                                          Qt::NoModifier,
                                          Qt::MouseEventSynthesizedByApplication);
                const bool secondReleased = secondPressAttempted && sameTarget()
                    && QCoreApplication::sendEvent(dispatchWindow.data(), &secondRelease);
                delivered = secondPressed && doubleClicked && secondReleased
                    && currentSurface();
            }
        }
    }
    dispatching_ = false;
    acknowledge(requestId, sessionId, serialText, revisionText,
                delivered ? QStringLiteral("delivered")
                          : pressAttempted ? QStringLiteral("uncertain")
                                           : QStringLiteral("rejected"));
}

bool FerricPagePointerAdapter::eventFilter(QObject *watched, QEvent *event)
{
    if (dispatching_)
        return false;
    if (event->type() == QEvent::MouseButtonRelease) {
        const auto *mouse = static_cast<QMouseEvent *>(event);
        if (consumedButtons_.remove(int(mouse->button())) > 0) {
            event->accept();
            return true;
        }
    }
    if (consumedTabletSequence_
        && (event->type() == QEvent::TabletMove || event->type() == QEvent::TabletRelease)) {
        if (event->type() == QEvent::TabletRelease)
            consumedTabletSequence_ = false;
        event->accept();
        return true;
    }
    if (consumedTouchSequence_
        && (event->type() == QEvent::TouchUpdate || event->type() == QEvent::TouchEnd
            || event->type() == QEvent::TouchCancel)) {
        if (event->type() == QEvent::TouchEnd || event->type() == QEvent::TouchCancel)
            consumedTouchSequence_ = false;
        event->accept();
        return true;
    }
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
    case QEvent::MouseButtonRelease:
        return false;
    case QEvent::Wheel:
    case QEvent::TabletMove:
        dispatchWatchdog_.stop();
        pendingRequestId_.clear();
        pendingSessionId_.clear();
        emit physicalPointerDetected(QStringLiteral("physical-pointer"));
        return true;
    case QEvent::TabletPress:
        consumedTabletSequence_ = true;
        dispatchWatchdog_.stop();
        pendingRequestId_.clear();
        pendingSessionId_.clear();
        emit physicalPointerDetected(QStringLiteral("physical-pointer"));
        return true;
    case QEvent::TouchBegin:
        consumedTouchSequence_ = true;
        dispatchWatchdog_.stop();
        pendingRequestId_.clear();
        pendingSessionId_.clear();
        emit physicalPointerDetected(QStringLiteral("physical-pointer"));
        return true;
    default:
        return false;
    }
}
