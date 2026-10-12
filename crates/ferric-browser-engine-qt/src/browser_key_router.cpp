#include "browser_key_router.h"

#include <QtCore/QCoreApplication>
#include <QtCore/QEvent>
#include <QtGui/QGuiApplication>
#include <QtGui/QKeyEvent>
#include <QtGui/QInputMethodEvent>

namespace {
quint64 keyIdentity(const QKeyEvent *event)
{
    const quint32 scanCode = event->nativeScanCode();
    if (scanCode != 0)
        return (quint64(1) << 63) | quint64(scanCode);
    return quint64(quint32(event->key()));
}

bool modifierKey(int key)
{
    return key == Qt::Key_Shift || key == Qt::Key_Control
        || key == Qt::Key_Alt || key == Qt::Key_Meta || key == Qt::Key_AltGr;
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

void FerricBrowserKeyRouter::setTransientInsert(bool value)
{
    if (transientInsert_ == value)
        return;
    transientInsert_ = value;
    if (!value) {
        composing_ = false;
        compositionReceiver_.clear();
    }
    emit transientInsertChanged();
}

void FerricBrowserKeyRouter::setInputEpoch(int value)
{
    if (inputEpoch_ == value)
        return;
    inputEpoch_ = value;
    passGesture_.reset();
    // Entry projects the insert variant before its epoch. Keep live preedit
    // when entering transient editing in an already-focused IME receiver.
    if (!transientInsert_ || compositionReceiver_.isNull()) {
        composing_ = false;
        compositionReceiver_.clear();
    }
    // Keep the physical gesture until key-up, but invalidate its mode exit.
    emit inputEpochChanged();
}

void FerricBrowserKeyRouter::setPassNextKey(bool value)
{
    if (passNextKey_ == value)
        return;
    passNextKey_ = value;
    passGesture_.reset();
    if (value)
        entryKeys_.unite(acceptedKeys_);
    emit passNextKeyChanged();
}

void FerricBrowserKeyRouter::setPassThrough(bool value)
{
    if (passThrough_ == value)
        return;
    passThrough_ = value;
    if (value)
        entryKeys_.unite(acceptedKeys_);
    emit passThroughChanged();
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
    // Re-dispatch only the original release, synchronously through Qt. The
    // recursion guard also covers propagation through the Quick item tree.
    if (event == forwardingEvent_)
        return false;
    if (targetWindow_ && QGuiApplication::focusWindow() == targetWindow_
        && event->type() == QEvent::InputMethod) {
        const auto *input = static_cast<QInputMethodEvent *>(event);
        composing_ = !input->preeditString().isEmpty();
        compositionReceiver_ = composing_ ? watched : nullptr;
        return false;
    }
    if (event->type() == QEvent::FocusOut && watched == compositionReceiver_) {
        composing_ = false;
        compositionReceiver_.clear();
    }
    const bool keyPress = event->type() == QEvent::KeyPress;
    const bool keyRelease = event->type() == QEvent::KeyRelease;
    const bool escapeOverride = event->type() == QEvent::ShortcutOverride
        && static_cast<QKeyEvent *>(event)->key() == Qt::Key_Escape;
    if (keyRelease) {
        // Internal/renderer deliveries cannot retire ownership of a newer
        // physical gesture. Native releases can belong to another QWindow.
        if (!qobject_cast<QWindow *>(watched))
            return false;
        const auto *keyEvent = static_cast<QKeyEvent *>(event);
        const quint64 identity = keyIdentity(keyEvent);
        if (entryKeys_.contains(identity) && keyEvent->isAutoRepeat()) {
            event->accept();
            return true;
        }
        if (!keyEvent->isAutoRepeat())
            entryKeys_.remove(identity);
        if (forwardedKeys_.contains(identity)) {
            if (keyEvent->isAutoRepeat())
                return false;
            forwardedKeys_.remove(identity);
            if (!passGesture_ || passGesture_->identity != identity)
                return false;
            const int epoch = passGesture_->epoch;
            passGesture_.reset();
            QPointer<FerricBrowserKeyRouter> alive(this);
            QEvent *previous = forwardingEvent_;
            forwardingEvent_ = event;
            QCoreApplication::sendEvent(watched, event);
            if (!alive)
                return true;
            forwardingEvent_ = previous;
            if (passNextKey_ && inputEpoch_ == epoch && targetWindow_)
                emit passNextKeyDispatched(epoch);
            return true;
        }
        if (enterGesture_ && enterGesture_->identity == identity
            && !keyEvent->isAutoRepeat()) {
            const int epoch = enterGesture_->epoch;
            enterGesture_.reset();
            QPointer<FerricBrowserKeyRouter> alive(this);
            QEvent *previous = forwardingEvent_;
            forwardingEvent_ = event;
            QCoreApplication::sendEvent(watched, event);
            if (!alive)
                return true;
            forwardingEvent_ = previous;
            // Completion belongs to the session that accepted the press, even
            // if Enter moved keyboard ownership before its release arrived.
            if (transientInsert_ && inputEpoch_ == epoch && targetWindow_) {
                emit transientEnterDispatched(epoch);
            }
            return true;
        }
        // Auto-repeat releases are part of the held gesture. Keep ownership
        // until the real release so a consumer that accepted the press also
        // receives the matching release exactly once.
        if (keyEvent->isAutoRepeat()) {
            if (!acceptedKeys_.contains(identity))
                return false;
            event->accept();
            return true;
        }
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
    // Native presses enter through QWindow. Quick then delivers them to the
    // focus proxy, and WebEngine may asynchronously bubble an unhandled copy
    // to its parent. Never classify those deliveries as fresh browser input:
    // the owning gesture may already have returned to Normal.
    if (keyPress && watched != targetWindow_)
        return false;

    // A new physical press supersedes a lost release of the same key. Reset
    // before classifying modifiers/composition, so its release cannot inherit
    // an earlier ordinary Enter's exit policy.
    const auto *keyEvent = static_cast<QKeyEvent *>(event);
    const quint64 identity = keyIdentity(keyEvent);
    if (keyPress && entryKeys_.contains(identity)) {
        if (keyEvent->isAutoRepeat()) {
            event->accept();
            return true;
        }
        entryKeys_.remove(identity);
    }
    if (keyPress && forwardedKeys_.contains(identity)) {
        if (keyEvent->isAutoRepeat())
            return false;
        // A fresh press replaces a lost release without inheriting its exit.
        forwardedKeys_.remove(identity);
        if (passGesture_ && passGesture_->identity == identity)
            passGesture_.reset();
    }
    if (keyPress && !keyEvent->isAutoRepeat() && enterGesture_
        && enterGesture_->identity == keyIdentity(keyEvent)) {
        enterGesture_.reset();
    }
    if (!enabled_)
        return false;

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
            if (passNextKey_ || passThrough_)
                entryKeys_.insert(identity);
        }
        event->accept();
    }
    // A fresh native press owns its release, even if an earlier browser-command
    // release was lost while changing windows or editing sessions.
    if (keyPress && !handled && !keyEvent->isAutoRepeat())
        acceptedKeys_.remove(keyIdentity(keyEvent));
    if (keyPress && !handled && (passNextKey_ || passThrough_) && !keyEvent->isAutoRepeat()) {
        forwardedKeys_.insert(identity);
        if (passNextKey_ && !passGesture_ && !modifierKey(keyEvent->key()))
            passGesture_ = KeyGesture { identity, inputEpoch_ };
    }
    if (keyPress && !handled && transientInsert_
        && (!composing_ || compositionReceiver_.isNull())
        && !keyEvent->isAutoRepeat()
        && (!enterGesture_ || enterGesture_->epoch != inputEpoch_)
        && (keyEvent->key() == Qt::Key_Return || keyEvent->key() == Qt::Key_Enter)
        && (keyEvent->modifiers() & ~Qt::KeypadModifier) == Qt::NoModifier) {
        enterGesture_ = KeyGesture { keyIdentity(keyEvent), inputEpoch_ };
    }
    return handled;
}
