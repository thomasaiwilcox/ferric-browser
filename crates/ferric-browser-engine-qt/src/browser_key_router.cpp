#include "browser_key_router.h"

#include <QtCore/QCoreApplication>
#include <QtCore/QEvent>
#include <QtGui/QGuiApplication>
#include <QtGui/QKeyEvent>

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
    const bool escapeOverride = event->type() == QEvent::ShortcutOverride
        && static_cast<QKeyEvent *>(event)->key() == Qt::Key_Escape;
    if (!targetWindow_ || QGuiApplication::focusWindow() != targetWindow_
        || (!keyPress && !escapeOverride)) {
        return false;
    }

    if (!enabled_)
        return false;

    const auto *keyEvent = static_cast<QKeyEvent *>(event);
    accepted_ = false;
    dispatching_ = true;
    emit keyPressed(keyEvent->text(), keyEvent->key(), int(keyEvent->modifiers()));
    dispatching_ = false;

    const bool handled = accepted_;
    accepted_ = false;
    if (handled)
        event->accept();
    return handled;
}
