#include "browser_key_router.h"

#include <QtCore/QCoreApplication>
#include <QtCore/QEvent>
#include <QtGui/QGuiApplication>
#include <QtGui/QKeyEvent>

namespace {
quint64 keyIdentity(const QKeyEvent *event)
{
    const quint32 scanCode = event->nativeScanCode();
    if (scanCode != 0)
        return (quint64(1) << 63) | quint64(scanCode);
    return quint64(quint32(event->key()));
}
}

FerricBrowserKeyRouter::FerricBrowserKeyRouter(QObject *parent)
    : QObject(parent)
{
    if (auto *application = QCoreApplication::instance())
        application->installEventFilter(this);
}

FerricBrowserKeyRouter::~FerricBrowserKeyRouter()
{
    if (auto *application = QCoreApplication::instance())
        application->removeEventFilter(this);
}

bool FerricBrowserKeyRouter::enabled() const
{
    return enabled_;
}

void FerricBrowserKeyRouter::setEnabled(bool enabled)
{
    if (enabled_ == enabled)
        return;
    enabled_ = enabled;
    emit enabledChanged();
}

QWindow *FerricBrowserKeyRouter::targetWindow() const
{
    return targetWindow_.data();
}

void FerricBrowserKeyRouter::setTargetWindow(QWindow *window)
{
    if (targetWindow_ == window)
        return;
    targetWindow_ = window;
    emit targetWindowChanged();
}

void FerricBrowserKeyRouter::acceptCurrentEvent()
{
    if (dispatching_)
        accepted_ = true;
}

bool FerricBrowserKeyRouter::eventFilter(QObject *watched, QEvent *event)
{
    Q_UNUSED(watched);
    const bool keyPress = event->type() == QEvent::KeyPress;
    const bool keyRelease = event->type() == QEvent::KeyRelease;
    const bool escapeOverride = event->type() == QEvent::ShortcutOverride
        && static_cast<QKeyEvent *>(event)->key() == Qt::Key_Escape;
    if (keyRelease) {
        const auto *keyEvent = static_cast<QKeyEvent *>(event);
        const quint64 identity = keyIdentity(keyEvent);
        if (!acceptedKeys_.contains(identity))
            return false;
        acceptedKeys_.remove(identity);
        emit keyReleased(keyEvent->key(), int(keyEvent->modifiers()));
        event->accept();
        return true;
    }
    if (!targetWindow_ || QGuiApplication::focusWindow() != targetWindow_
        || (!keyPress && !escapeOverride)) {
        return false;
    }

    if (!enabled_)
        return false;

    const auto *keyEvent = static_cast<QKeyEvent *>(event);
    accepted_ = false;
    dispatching_ = true;
    emit keyPressed(
        keyEvent->text(), keyEvent->key(), int(keyEvent->modifiers()), keyEvent->isAutoRepeat());
    dispatching_ = false;

    const bool handled = accepted_;
    accepted_ = false;
    if (handled)
    {
        if (keyPress && !keyEvent->isAutoRepeat()) {
            acceptedKeys_.insert(keyIdentity(keyEvent));
        }
        event->accept();
    }
    return handled;
}
