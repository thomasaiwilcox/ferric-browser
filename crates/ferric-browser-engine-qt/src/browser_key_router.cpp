#include "browser_key_router.h"

#include <QtCore/QCoreApplication>
#include <QtCore/QEvent>
#include <QtCore/QVariant>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QMetaProperty>
#include <QtGui/QGuiApplication>
#include <QtGui/QKeyEvent>
#include <QtGui/QInputMethodEvent>
#include <QtQuick/QQuickItem>
#include <QtQuick/QQuickWindow>
#include <algorithm>
#include <limits>

namespace {
std::optional<FerricPageKeyDestination> pageKeyDestination(
    const QString &canonical, const QVariantMap &descriptor)
{
    if (descriptor.size() != 6) return std::nullopt;
    QString prefix;
    FerricPageKeyDestination result;
    result.modifiers = Qt::NoModifier;
    const struct { const char *field; const char *prefix; Qt::KeyboardModifier flag; quint32 native; }
        modifiers[] = {{"ctrl", "Ctrl+", Qt::ControlModifier, 4},
                       {"alt", "Alt+", Qt::AltModifier, 8},
                       {"meta", "Meta+", Qt::MetaModifier, 64},
                       {"shift", "Shift+", Qt::ShiftModifier, 1}};
    for (const auto &modifier : modifiers) {
        const auto value = descriptor.value(QLatin1String(modifier.field));
        if (value.metaType().id() != QMetaType::Bool) return std::nullopt;
        if (value.toBool()) {
            prefix += QLatin1String(modifier.prefix);
            result.modifiers |= modifier.flag;
            result.nativeModifiers |= modifier.native;
        }
    }
    const auto text = descriptor.value(QStringLiteral("text"));
    const auto named = descriptor.value(QStringLiteral("named"));
    if (text.metaType().id() != QMetaType::QString
        || named.metaType().id() != QMetaType::QString) return std::nullopt;
    result.text = text.toString();
    const auto name = named.toString();
    if (result.text.isEmpty() == name.isEmpty()) return std::nullopt;
    const QString token = name.isEmpty()
        ? (result.text == QStringLiteral("+") ? QStringLiteral("Plus") : result.text) : name;
    if (prefix + token != canonical) return std::nullopt;
    if (!name.isEmpty()) {
        const struct { const char *name; int key; quint32 scan; quint32 symbol; const char *text; }
            keys[] = {{"Space", Qt::Key_Space, 65, 0x20, " "},
                      {"Enter", Qt::Key_Return, 36, 0xff0d, "\r"},
                      {"Tab", Qt::Key_Tab, 23, 0xff09, "\t"},
                      {"Backtab", Qt::Key_Backtab, 23, 0xfe20, "\t"},
                      {"Backspace", Qt::Key_Backspace, 22, 0xff08, "\b"},
                      {"Delete", Qt::Key_Delete, 119, 0xffff, "\x7f"},
                      {"Insert", Qt::Key_Insert, 118, 0xff63, ""},
                      {"Escape", Qt::Key_Escape, 9, 0xff1b, "\x1b"},
                      {"Left", Qt::Key_Left, 113, 0xff51, ""},
                      {"Right", Qt::Key_Right, 114, 0xff53, ""},
                      {"Up", Qt::Key_Up, 111, 0xff52, ""},
                      {"Down", Qt::Key_Down, 116, 0xff54, ""},
                      {"Home", Qt::Key_Home, 110, 0xff50, ""},
                      {"End", Qt::Key_End, 115, 0xff57, ""},
                      {"PgUp", Qt::Key_PageUp, 112, 0xff55, ""},
                      {"PgDown", Qt::Key_PageDown, 117, 0xff56, ""},
                      {"Back", Qt::Key_Back, 166, 0x1008ff26, ""},
                      {"Forward", Qt::Key_Forward, 167, 0x1008ff27, ""}};
        bool found = false;
        for (const auto &key : keys) {
            if (name != QLatin1String(key.name)) continue;
            result.key = key.key; result.scan = key.scan; result.symbol = key.symbol;
            result.text = QString::fromLatin1(key.text); found = true; break;
        }
        if (!found) {
            bool valid = false;
            const int function = name.mid(1).toInt(&valid);
            if (!name.startsWith(QLatin1Char('F')) || !valid || function < 1 || function > 12)
                return std::nullopt;
            result.key = Qt::Key_F1 + function - 1;
            result.scan = function <= 10 ? 66 + function : 84 + function;
            result.symbol = 0xffbd + function;
        }
        if (result.key == Qt::Key_Backtab) {
            result.modifiers |= Qt::ShiftModifier;
            result.nativeModifiers |= 1;
        }
    } else {
        if (result.text.size() != 1) return std::nullopt;
        const auto character = result.text.front();
        const auto lower = character.toLower();
        if (lower >= QLatin1Char('a') && lower <= QLatin1Char('z')) {
            constexpr quint32 scans[] = {38,56,54,40,26,41,42,43,31,44,45,46,58,
                                        57,32,33,24,27,39,28,30,55,25,53,29,52};
            result.key = Qt::Key_A + lower.unicode() - 'a';
            result.scan = scans[lower.unicode() - 'a'];
            if (character != lower) result.modifiers |= Qt::ShiftModifier;
            result.text = QString(result.modifiers.testFlag(Qt::ShiftModifier)
                ? character.toUpper() : character);
            result.symbol = result.text.front().unicode();
        } else {
            const QString plain = QStringLiteral("1234567890`-=[]\\;' ,./");
            const QString shifted = QStringLiteral("!@#$%^&*()~_+{}|:\" <>?");
            constexpr quint32 scans[] = {10,11,12,13,14,15,16,17,18,19,49,20,21,
                                        34,35,51,47,48,65,59,60,61};
            int index = plain.indexOf(character);
            if (index < 0) {
                index = shifted.indexOf(character);
                if (index < 0) return std::nullopt;
                result.modifiers |= Qt::ShiftModifier;
            }
            result.key = character.unicode(); result.scan = scans[index];
            result.symbol = character.unicode();
        }
        if (result.modifiers.testFlag(Qt::ShiftModifier)) result.nativeModifiers |= 1;
    }
    return result;
}

QString outputChannel(quint64 document, const QString &canonical, const QString &receiver = {})
{
    return QString::number(document) + QLatin1Char(':') + receiver + QLatin1Char(':') + canonical;
}

// Qt 6.11.2's WebEngine NativeWebKeyboardEvent copies its QKeyEvent through
// virtual clone(), including the asynchronous unhandled-key return. Keep
// provenance in that copy rather than relying only on a synchronous flag.
class PageKeyEvent final : public QKeyEvent {
public:
    PageKeyEvent(FerricBrowserKeyRouter *router, QQuickItem *receiver,
                 const FerricPageKeyDestination &destination, bool press, bool autoRepeat)
        : QKeyEvent(press ? QEvent::KeyPress : QEvent::KeyRelease,
                    destination.key, destination.modifiers, destination.scan,
                    destination.symbol, destination.nativeModifiers, destination.text, autoRepeat)
        , owner(router), receiver(receiver)
    {
    }
    PageKeyEvent *clone() const override { return new PageKeyEvent(*this); }
    QPointer<FerricBrowserKeyRouter> owner;
    QPointer<QQuickItem> receiver;
};

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

bool protectedRecoveryKey(const QKeyEvent *event)
{
    return event->key() == Qt::Key_Escape && !event->isAutoRepeat()
        && (event->modifiers() == Qt::ControlModifier
            || event->modifiers() == (Qt::ControlModifier | Qt::ShiftModifier));
}
}

// Application-lifetime ownership of already claimed physical sources. This
// guard does no policy matching and creates no output. It only delegates to
// the existing owner, or captures a cancelled continuation after owner loss.
class FerricNativeSourceCapture final : public QObject {
    enum class Kind { Feasibility, Browser };
    struct Capture {
        Kind kind = Kind::Feasibility;
        QPointer<FerricBrowserKeyRouter> owner;
        QPointer<QWindow> browserWindow;
        quint64 serial = 0;
        quint64 generation = 0;
        QPointer<QWindow> recoveryWindow;
        bool recovering = false;
        bool uncapturedFreshPress = false;
        bool cancelled = false;
    };
    QHash<quint64, Capture> sources_;
    QHash<quint64, FerricBrowserKeyRouter::KeyAdmission *> pendingAdmissions_;
    quint64 generation_ = 0;
    QPointer<QWindow> recoveryCaptureWindow_;
    static bool recoverable(quint64 identity, const Capture &source) {
        if (source.kind != Kind::Feasibility) return false;
        if (!source.owner) return source.cancelled;
        const auto *owner = source.owner.data();
        const auto record = owner->feasibilitySources_.constFind(identity);
        return record != owner->feasibilitySources_.cend() && record->cancelled
            && !record->committed && !record->delivering && !record->finishing;
    }
public:
    explicit FerricNativeSourceCapture(QObject *parent) : QObject(parent) {
        QCoreApplication::instance()->installEventFilter(this);
    }
    ~FerricNativeSourceCapture() override {
        if (auto *app = QCoreApplication::instance()) app->removeEventFilter(this);
    }
    static FerricNativeSourceCapture *get(bool create = false) {
        static QPointer<FerricNativeSourceCapture> capture;
        if (!capture && create && QCoreApplication::instance() && !QCoreApplication::closingDown())
            capture = new FerricNativeSourceCapture(QCoreApplication::instance());
        return capture;
    }
    bool contains(quint64 identity) const { return sources_.contains(identity); }
    bool held() const {
        // Browser command entry keys do not own transformed native output and
        // must not make the experimental raw-output recovery refuse entry.
        return std::any_of(sources_.cbegin(), sources_.cend(),
            [](const Capture &source) { return source.kind != Kind::Browser; });
    }
    void beginPending(FerricBrowserKeyRouter::KeyAdmission &admission, bool fresh) {
        admission.sharedPrevious = pendingAdmissions_.value(admission.identity);
        if (fresh) {
            for (auto *pending = admission.sharedPrevious; pending; pending = pending->sharedPrevious)
                pending->superseded = true;
        }
        pendingAdmissions_.insert(admission.identity, &admission);
        admission.sharedRegistered = true;
    }
    void endPending(FerricBrowserKeyRouter::KeyAdmission &admission) {
        auto pending = pendingAdmissions_.find(admission.identity);
        if (pending == pendingAdmissions_.end() || pending.value() != &admission) return;
        if (admission.sharedPrevious) pending.value() = admission.sharedPrevious;
        else pendingAdmissions_.erase(pending);
    }
    void admissionLost(quint64 identity, bool browser = false) {
        if (sources_.contains(identity)) return;
        Capture source;
        source.kind = browser ? Kind::Browser : Kind::Feasibility;
        source.generation = ++generation_;
        sources_.insert(identity, source);
    }
    bool claim(FerricBrowserKeyRouter *owner, quint64 identity, quint64 serial) {
        if (sources_.contains(identity)) return false;
        Capture source;
        source.owner = owner; source.serial = serial; source.generation = ++generation_;
        sources_.insert(identity, source);
        return true;
    }
    bool claimBrowser(FerricBrowserKeyRouter *owner, quint64 identity, quint64 serial,
                      QWindow *window, bool cancelled) {
        if (!claim(owner, identity, serial)) return false;
        sources_[identity].kind = Kind::Browser;
        sources_[identity].browserWindow = window;
        sources_[identity].cancelled = cancelled || !window || owner->targetWindow_ != window;
        return true;
    }
    void release(FerricBrowserKeyRouter *owner, quint64 identity, quint64 serial) {
        auto source = sources_.find(identity);
        if (source != sources_.end() && source->owner == owner && source->serial == serial)
            sources_.erase(source);
    }
    void ownerLost(FerricBrowserKeyRouter *owner, quint64 identity, quint64 serial,
                   bool held, bool uncapturedFreshPress) {
        auto source = sources_.find(identity);
        if (source == sources_.end() || source->owner != owner || source->serial != serial) return;
        if (!held) { sources_.erase(source); return; }
        source->owner.clear();
        source->cancelled = true;
        source->recoveryWindow.clear();
        source->recovering = false;
        source->uncapturedFreshPress = uncapturedFreshPress;
    }
    bool armRecovery(quint64 identity, QWindow *window) {
        auto source = sources_.find(identity);
        if (source == sources_.end() || !window || QGuiApplication::focusWindow() != window)
            return false;
        if (!recoverable(identity, *source)) return false;
        if (source->owner) {
            auto *owner = source->owner.data();
            const auto record = owner->feasibilitySources_.constFind(identity);
            if (record == owner->feasibilitySources_.cend() || !record->cancelled
                || record->committed || record->delivering || record->finishing)
                return false;
            owner->feasibilityRecovery_ = identity;
            owner->feasibilityRecoveryWindow_ = window;
        } else {
            source->recoveryWindow = window;
        }
        return true;
    }
    bool armCancelledRecovery(QWindow *window) {
        if (!window || QGuiApplication::focusWindow() != window) return false;
        for (auto source = sources_.cbegin(); source != sources_.cend(); ++source) {
            if (!recoverable(source.key(), *source)) continue;
            recoveryCaptureWindow_ = window;
            return true;
        }
        return false;
    }
    void cancelPendingRecovery(QWindow *window) {
        if (recoveryCaptureWindow_ == window) recoveryCaptureWindow_.clear();
    }
    bool capture(QObject *watched, QEvent *event) {
        if (event->type() == QEvent::WindowDeactivate && watched == recoveryCaptureWindow_)
            recoveryCaptureWindow_.clear();
        if (const auto *output = dynamic_cast<const PageKeyEvent *>(event)) {
            if (output->owner) return false;
            event->accept();
            return true;
        }
        const bool press = event->type() == QEvent::KeyPress;
        const bool release = event->type() == QEvent::KeyRelease;
        const bool shortcut = event->type() == QEvent::ShortcutOverride;
        if ((!press && !release && !shortcut) || !qobject_cast<QWindow *>(watched))
            return false;
        const auto *key = static_cast<QKeyEvent *>(event);
        const auto identity = keyIdentity(key);
        auto source = sources_.find(identity);
        if (source == sources_.end()) return false;
        if (source->kind == Kind::Browser) {
            // Fresh downs replace a lost release, including transfer to a new
            // router. Merely observing another window cannot rematch a repeat.
            if (shortcut && !key->isAutoRepeat()) return false;
            const auto owner = source->owner;
            if (press && !key->isAutoRepeat()) {
                if (owner) {
                    owner->acceptedKeys_.remove(identity);
                    owner->browserBindingContexts_.remove(identity);
                    owner->entryKeys_.remove(identity);
                    owner->keyGenerations_.remove(identity);
                }
                sources_.erase(source);
                return false;
            }
            if (owner && watched == source->browserWindow
                && owner->targetWindow_ == source->browserWindow
                && (release || (!source->cancelled && owner->enabled_
                    && QGuiApplication::focusWindow() == owner->targetWindow_)))
                return false;
            source->cancelled = true;
            if (owner) {
                auto context = owner->browserBindingContexts_.find(identity);
                if (context != owner->browserBindingContexts_.end()) context->cancelled = true;
            }
            if (release && !key->isAutoRepeat()) {
                // Retire before notifying the original owner. A nested fresh
                // press from that notification must keep its newer record.
                sources_.erase(source);
                if (owner) owner->finishBrowserSource(key);
            }
            event->accept();
            return true;
        }
        // An unmodified Escape mapping can own this same physical key. Its
        // capture must still let a fresh protected recovery chord reach the
        // browser resolver. Keep the old source until the real release.
        if (protectedRecoveryKey(key) && (press || shortcut)) {
            const auto owner = source->owner;
            if (press && owner) owner->cancelFeasibilitySource(identity, true);
            return false;
        }
        if (shortcut) { event->accept(); return true; }
        if (press && !key->isAutoRepeat() && watched == recoveryCaptureWindow_
            && recoverable(identity, *source) && armRecovery(identity, recoveryCaptureWindow_))
            recoveryCaptureWindow_.clear();
        const auto owner = source->owner;
        if (owner) {
            const bool consumed = owner->continueFeasibilitySource(watched, key);
            if (owner && release && !key->isAutoRepeat()) owner->finishBrowserSource(key);
            if (consumed) event->accept();
            return consumed;
        }
        const auto generation = source->generation;
        if (press && !key->isAutoRepeat()) {
            source->recovering = source->recoveryWindow == watched
                && QGuiApplication::focusWindow() == source->recoveryWindow;
            source->uncapturedFreshPress = !source->recovering;
        }
        if (release && !key->isAutoRepeat()
            && (!source->uncapturedFreshPress || source->recovering)) {
            source = sources_.find(identity);
            if (source != sources_.end() && source->generation == generation)
                sources_.erase(source);
        }
        event->accept();
        return true;
    }
protected:
    bool eventFilter(QObject *watched, QEvent *event) override { return capture(watched, event); }
};

FerricBrowserKeyRouter::KeyAdmission::~KeyAdmission()
{
    // The marker lives on the synchronous dispatch stack, independently of
    // the router QObject. No physical-key history remains after unwinding.
    if (sharedRegistered) {
        if (auto *capture = FerricNativeSourceCapture::get()) capture->endPending(*this);
    }
}

FerricBrowserKeyRouter::FerricBrowserKeyRouter(QObject *parent)
    : QObject(parent)
{
    focusBroker_ = new FerricFrameBroker(this, QStringLiteral("__ferric_page_key_document"));
    if (auto *application = QCoreApplication::instance())
        application->installEventFilter(this);
}

FerricBrowserKeyRouter::~FerricBrowserKeyRouter()
{
    ++focusRequest_;
    focusBroker_->clear();
    for (auto *pending = currentAdmission_; pending; pending = pending->previous)
        captureAbandonedAdmission(*pending);
    if (auto *capture = FerricNativeSourceCapture::get()) {
        for (const auto &source : feasibilitySources_) {
            capture->ownerLost(this, source.identity, source.serial,
                               !source.pendingRelease && !source.finishing,
                               source.uncapturedFreshPress);
        }
    }
    if (auto *application = QCoreApplication::instance())
        application->removeEventFilter(this);
}

void FerricBrowserKeyRouter::captureAbandonedAdmission(const KeyAdmission &admission)
{
    if (admission.generation != keyGenerations_.value(admission.identity))
        return;
    captureAbandonedSource(admission);
}

void FerricBrowserKeyRouter::captureAbandonedSource(const KeyAdmission &admission)
{
    if (!admission.isPress || admission.superseded) return;
    for (const auto &release : admission.deferredReleases) {
        if (!static_cast<const QKeyEvent *>(release.event.get())->isAutoRepeat()) return;
    }
    if (auto *capture = FerricNativeSourceCapture::get(true))
        capture->admissionLost(admission.identity, admission.bindingContext.has_value());
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

void FerricBrowserKeyRouter::setPolicyContext(const QString &context)
{
    if (policyContext_ == context) return;
    policyContext_ = context;
    cancelBrowserSources();
    const QPointer<FerricBrowserKeyRouter> alive(this);
    cancelFeasibilitySources(true);
    if (alive) emit policyContextChanged();
}

void FerricBrowserKeyRouter::setInputEpoch(int value)
{
    const auto request = ++inputEpochRequest_;
    if (inputEpoch_ == value)
        return;
    if (auto *capture = FerricNativeSourceCapture::get()) capture->cancelPendingRecovery(targetWindow_);
    const QPointer<FerricBrowserKeyRouter> alive(this);
    if (!completingPassGesture_)
        cancelFeasibilitySources(true);
    if (!alive || inputEpochRequest_ != request)
        return;
    feasibilityRecovery_.reset();
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
    const auto request = ++passNextRequest_;
    if (passNextKey_ == value)
        return;
    const QPointer<FerricBrowserKeyRouter> alive(this);
    if (!value && !completingPassGesture_)
        cancelFeasibilitySources(true);
    if (!alive || passNextRequest_ != request)
        return;
    passNextKey_ = value;
    feasibilityRaw_ = false;
    passGesture_.reset();
    if (value)
        entryKeys_.unite(acceptedKeys_);
    emit passNextKeyChanged();
}

void FerricBrowserKeyRouter::setPassThrough(bool value)
{
    const auto request = ++passThroughRequest_;
    if (passThrough_ == value)
        return;
    const QPointer<FerricBrowserKeyRouter> alive(this);
    if (!value && !completingPassGesture_)
        cancelFeasibilitySources(true);
    if (!alive || passThroughRequest_ != request)
        return;
    passThrough_ = value;
    feasibilityRaw_ = false;
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
    if (!enabled) {
        if (auto *capture = FerricNativeSourceCapture::get()) capture->cancelPendingRecovery(targetWindow_);
        cancelBrowserSources();
        cancelFeasibilitySources(false);
    }
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
    if (auto *capture = FerricNativeSourceCapture::get()) capture->cancelPendingRecovery(targetWindow_);
    cancelBrowserSources();
    cancelFeasibilitySources(false);
    QObject::disconnect(feasibilityFocusConnection_);
    QObject::disconnect(feasibilityActiveConnection_);
    targetWindow_ = window;
    if (auto *quick = qobject_cast<QQuickWindow *>(window)) {
        feasibilityFocusConnection_ = connect(quick, &QQuickWindow::activeFocusItemChanged,
                                              this, [this] {
            cancelBrowserSources();
            cancelFeasibilitySources(false);
        });
        feasibilityActiveConnection_ = connect(quick, &QWindow::activeChanged, this, [this, quick] {
            if (!quick->isActive()) {
                cancelBrowserSources();
                cancelFeasibilitySources(false);
            }
        });
    }
    emit targetWindowChanged();
}

void FerricBrowserKeyRouter::acceptCurrentEvent()
{
    if (currentAdmission_)
        currentAdmission_->accepted = true;
}

void FerricBrowserKeyRouter::finishBrowserSource(const QKeyEvent *event)
{
    const auto identity = keyIdentity(event);
    nativeClipboardOverrides_.remove(identity);
    keyGenerations_.remove(identity);
    entryKeys_.remove(identity);
    browserBindingContexts_.remove(identity);
    if (!acceptedKeys_.remove(identity)) return;
    KeyAdmission releaseAdmission;
    releaseAdmission.previous = currentAdmission_;
    currentAdmission_ = &releaseAdmission;
    const QPointer<FerricBrowserKeyRouter> alive(this);
    emit keyReleased(event->key(), int(event->modifiers()));
    if (alive) currentAdmission_ = releaseAdmission.previous;
}

void FerricBrowserKeyRouter::cancelBrowserSources()
{
    for (auto &source : browserBindingContexts_) source.cancelled = true;
    for (auto *pending = currentAdmission_; pending; pending = pending->previous) {
        if (pending->bindingContext) pending->bindingContextCancelled = true;
    }
}

bool FerricBrowserKeyRouter::captureBindingContext(const QString &context)
{
    if (!currentAdmission_)
        return false;
    auto &admission = *currentAdmission_;
    // ShortcutOverride keeps its existing handling; only a physical press
    // may establish the context retained through repeats and real release.
    if (!admission.isPress)
        return true;
    if (!admission.bindingContext)
        admission.bindingContext = context;
    auto source = browserBindingContexts_.find(admission.identity);
    const auto cancel = [&] {
        admission.bindingContextCancelled = true;
        admission.accepted = true;
        if (source != browserBindingContexts_.end()) source->cancelled = true;
        return false;
    };
    if (context.isEmpty() || admission.bindingContextCancelled
        || *admission.bindingContext != context)
        return cancel();
    if (!admission.isAutoRepeat)
        return true;
    if (acceptedKeys_.contains(admission.identity)
        && (source == browserBindingContexts_.end() || source->cancelled
            || source->value != context))
        return cancel();
    // Publication can reenter input before the original press callback has
    // committed its ownership. Keep that exact pending context as well.
    for (auto *pending = admission.sharedPrevious; pending; pending = pending->sharedPrevious) {
        if (!pending->isPress || pending->superseded
            || pending->identity != admission.identity || !pending->bindingContext)
            continue;
        if (pending->bindingContextCancelled || *pending->bindingContext != context) {
            pending->bindingContextCancelled = true;
            return cancel();
        }
    }
    return true;
}

void FerricBrowserKeyRouter::pageOwnsCurrentEvent()
{
    if (currentAdmission_)
        currentAdmission_->pageOwned = true;
}

bool FerricBrowserKeyRouter::activateSearchResult(QQuickItem *view, bool newTab)
{
    const QPointer<FerricBrowserKeyRouter> alive(this);
    const QPointer<QQuickWindow> owner(qobject_cast<QQuickWindow *>(targetWindow_.data()));
    const QPointer<QQuickItem> viewOwner(view);
    if (!enabled_ || !owner || !viewOwner || viewOwner->window() != owner
        || QGuiApplication::focusWindow() != owner || !viewOwner->isVisible())
        return false;
    viewOwner->forceActiveFocus(Qt::OtherFocusReason);
    // Focus notifications and native delivery can synchronously retire any
    // participant. Never restore a member through an RAII reference after its
    // QObject has been destroyed.
    if (!alive || !owner || !viewOwner || !enabled_ || targetWindow_ != owner
        || viewOwner->window() != owner || !viewOwner->isVisible()
        || QGuiApplication::focusWindow() != owner)
        return false;
    const QPointer<QQuickItem> receiver(owner->activeFocusItem());
    if (!receiver || (receiver != viewOwner && !viewOwner->isAncestorOf(receiver)))
        return false;

    // Qt's empty findText hands the active native match to page selection and
    // focus. Activate that receiver as a browser key gesture, without guessing
    // a URL from DOM text, ancestry, coordinates, or match ordinals.
    const auto modifiers = newTab ? Qt::ControlModifier | Qt::ShiftModifier : Qt::NoModifier;
    QKeyEvent press(QEvent::KeyPress, Qt::Key_Return, modifiers, QStringLiteral("\r"));
    QEvent *previous = forwardingEvent_;
    forwardingEvent_ = &press;
    QCoreApplication::sendEvent(owner.data(), &press);
    if (!alive)
        return true;
    forwardingEvent_ = previous;
    if (owner && viewOwner && receiver && owner->activeFocusItem() == receiver
        && QGuiApplication::focusWindow() == owner) {
        QKeyEvent release(QEvent::KeyRelease, Qt::Key_Return, modifiers, QStringLiteral("\r"));
        forwardingEvent_ = &release;
        QCoreApplication::sendEvent(owner.data(), &release);
        if (!alive)
            return true;
        forwardingEvent_ = previous;
    }
    return true;
}

bool FerricBrowserKeyRouter::eventFilter(QObject *watched, QEvent *event)
{
    // Evidence capture keeps renderer focus for the checked frame chain, but
    // owns input. IME commits/queries and native drops have no KeyPress to route.
    if (captureOnly_ && targetWindow_ && QGuiApplication::focusWindow() == targetWindow_) {
        const auto *item = qobject_cast<QQuickItem *>(watched);
        if (watched == targetWindow_ || (item && item->window() == targetWindow_)) {
            switch (event->type()) {
            case QEvent::InputMethod:
            case QEvent::InputMethodQuery:
                event->accept();return true;
            case QEvent::DragEnter:
            case QEvent::DragMove:
            case QEvent::Drop:
            case QEvent::ContextMenu:
                event->ignore();return true;
            default:break;
            }
        }
    }
    if (const auto *pageKey = dynamic_cast<const PageKeyEvent *>(event)) {
        if (pageKey->owner && pageKey->owner != this)
            return false;
        if (pageKey->owner == this && event == pageKeyDelivery_
            && watched == pageKey->receiver)
            return false;
        // A returned clone is terminal output, never another source gesture.
        if (pageKey->owner == this)
            ++feasibilityReturnedEvents_;
        event->accept();
        return true;
    }
    // Re-dispatch only the original release, synchronously through Qt. The
    // recursion guard also covers propagation through the Quick item tree.
    if (event == forwardingEvent_)
        return false;
    if (auto *capture = FerricNativeSourceCapture::get(); capture && capture->capture(watched, event)) {
        event->accept();
        return true;
    }
    if (targetWindow_ && QGuiApplication::focusWindow() == targetWindow_
        && event->type() == QEvent::InputMethod) {
        const auto *input = static_cast<QInputMethodEvent *>(event);
        composing_ = !input->preeditString().isEmpty();
        compositionReceiver_ = composing_ ? watched : nullptr;
        if (composing_) {
            const QPointer<FerricBrowserKeyRouter> alive(this);
            const QPointer<QObject> receiver(watched);
            cancelFeasibilitySources(true);
            if (!alive || !receiver) { event->accept(); return true; }
        }
        return false;
    }
    if (event->type() == QEvent::FocusOut && watched == compositionReceiver_) {
        composing_ = false;
        compositionReceiver_.clear();
    }
    if (enabled_ && targetWindow_ && QGuiApplication::focusWindow() == targetWindow_
        && event->type() == QEvent::ShortcutOverride) {
        const auto *key = static_cast<QKeyEvent *>(event);
        const auto modifiers = key->modifiers() & ~Qt::KeypadModifier;
        const bool clipboard = (modifiers == Qt::ControlModifier
            && (key->key() == Qt::Key_C || key->key() == Qt::Key_X || key->key() == Qt::Key_V))
            || (modifiers == (Qt::ControlModifier | Qt::ShiftModifier) && key->key() == Qt::Key_V);
        if (clipboard) {
            const auto identity = keyIdentity(key);
            const bool hadNativeOverride = nativeClipboardOverrides_.contains(identity);
            KeyAdmission admission;
            KeyAdmission *previous = currentAdmission_;
            admission.previous = previous;
            currentAdmission_ = &admission;
            const QPointer<FerricBrowserKeyRouter> alive(this);
            const QPointer<QObject> receiver(watched);
            emit clipboardShortcutOverride(key->key(), int(key->modifiers()));
            if (!alive) {
                event->accept();
                return true;
            }
            currentAdmission_ = previous;
            if (!receiver) {
                event->accept();
                return true;
            }
            const bool nativeOwned = pageOwnedKeys_.contains(identity) || forwardedKeys_.contains(identity) || hadNativeOverride;
            const bool browserOwned = acceptedKeys_.contains(identity) || (!nativeOwned && admission.accepted);
            if (browserOwned) {
                nativeClipboardOverrides_.remove(keyIdentity(key));
                event->accept();
                return true;
            }
            // Native shortcut activation may change focus before KeyPress.
            // Retain the original owner instead of classifying that gesture twice.
            if (!key->isAutoRepeat()) nativeClipboardOverrides_.insert(keyIdentity(key));
        }
    }
    const bool keyPress = event->type() == QEvent::KeyPress;
    const bool keyRelease = event->type() == QEvent::KeyRelease;
    const bool escapeOverride = event->type() == QEvent::ShortcutOverride
        && static_cast<QKeyEvent *>(event)->key() == Qt::Key_Escape;
    if (keyPress && qobject_cast<QWindow *>(watched)
        && continueFeasibilitySource(watched, static_cast<QKeyEvent *>(event))) {
        event->accept();
        return true;
    }
    if (keyRelease) {
        // Internal/renderer deliveries cannot retire ownership of a newer
        // physical gesture. Native releases can belong to another QWindow.
        if (!qobject_cast<QWindow *>(watched))
            return false;
        const auto *keyEvent = static_cast<QKeyEvent *>(event);
        const quint64 identity = keyIdentity(keyEvent);
        const auto browserGeneration = keyGenerations_.value(identity);
        // A nested event loop can receive key-up before its press callback has
        // decided ownership. Keep it with that press until admission finishes.
        for (auto *pending = currentAdmission_; pending; pending = pending->previous) {
            if (pending->isPress && !pending->superseded && pending->identity == identity
                && pending->generation == keyGenerations_.value(identity)) {
                pending->deferredReleases.push_back({watched, std::unique_ptr<QEvent>(keyEvent->clone())});
                event->accept();
                return true;
            }
        }
        if (continueFeasibilitySource(watched, keyEvent)) {
            event->accept();
            return true;
        }
        if (!keyEvent->isAutoRepeat()) {
            nativeClipboardOverrides_.remove(identity);
            keyGenerations_.remove(identity);
            browserBindingContexts_.remove(identity);
        }
        if (pageOwnedKeys_.contains(identity)) {
            if (!keyEvent->isAutoRepeat())
                pageOwnedKeys_.remove(identity);
            return false;
        }
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
                completePassGesture(epoch);
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
        if (auto *capture = FerricNativeSourceCapture::get())
            capture->release(this, identity, browserGeneration);
        finishBrowserSource(keyEvent);
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
    if (keyPress && !keyEvent->isAutoRepeat()) {
        keyGenerations_.insert(identity, ++nextGeneration_);
        browserBindingContexts_.remove(identity);
    }
    if (keyPress && nativeClipboardOverrides_.remove(identity)) {
        acceptedKeys_.remove(identity);
        if (passNextKey_ || passThrough_) {
            forwardedKeys_.insert(identity);
            if (passNextKey_ && !passGesture_) passGesture_ = KeyGesture { identity, inputEpoch_ };
        } else {
            pageOwnedKeys_.insert(identity);
        }
        return false;
    }
    if (keyPress && entryKeys_.contains(identity)) {
        if (keyEvent->isAutoRepeat()) {
            event->accept();
            return true;
        }
        entryKeys_.remove(identity);
    }
    if (keyPress && pageOwnedKeys_.contains(identity)) {
        if (keyEvent->isAutoRepeat())
            return false;
        // A fresh press replaces a lost page-owned release and is classified
        // against the current policy as a new physical gesture.
        pageOwnedKeys_.remove(identity);
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
    if (keyPress && keyEvent->isAutoRepeat()) {
        const auto source = browserBindingContexts_.constFind(identity);
        if (source != browserBindingContexts_.cend() && source->cancelled) {
            event->accept();
            return true;
        }
    }
    KeyAdmission admission;
    KeyAdmission *previous = currentAdmission_;
    admission.previous = previous;
    admission.isPress = keyPress;
    admission.isAutoRepeat = keyEvent->isAutoRepeat();
    admission.event = keyEvent;
    admission.sourceWindow = qobject_cast<QWindow *>(watched);
    admission.identity = identity;
    admission.generation = keyGenerations_.value(identity);
    if (auto *capture = FerricNativeSourceCapture::get(true))
        capture->beginPending(admission, !keyEvent->isAutoRepeat());
    currentAdmission_ = &admission;
    const QPointer<FerricBrowserKeyRouter> alive(this);
    const QPointer<QObject> receiver(watched);
    QString decisionText = keyEvent->text();
    // Metadata-free Qt deliveries (including QTest) can report lowercase or
    // empty text for Shift+ASCII. Retain the existing browser-key fallback.
    // Native layout-produced text, including Shift with CapsLock, stays exact;
    // the original event and its metadata are never changed for page delivery.
    if (keyEvent->nativeScanCode() == 0
        && (keyEvent->modifiers() & (Qt::ControlModifier | Qt::AltModifier | Qt::MetaModifier)) == Qt::NoModifier
        && keyEvent->modifiers().testFlag(Qt::ShiftModifier)
        && keyEvent->key() >= Qt::Key_A && keyEvent->key() <= Qt::Key_Z
        && (decisionText.isEmpty() || (decisionText.size() == 1
            && decisionText.at(0).unicode() == keyEvent->key() + ('a' - 'A'))))
        decisionText = QChar(keyEvent->key());
    emit keyPressed(
        decisionText,
        keyEvent->key(),
        int(keyEvent->modifiers()),
        keyEvent->isAutoRepeat(),
        keyEvent->isAutoRepeat() && acceptedKeys_.contains(identity));
    if (!alive) {
        if (admission.bindingContext) captureAbandonedSource(admission);
        event->accept();
        return true;
    }
    currentAdmission_ = previous;
    if (!receiver) {
        captureAbandonedAdmission(admission);
        event->accept();
        return true;
    }

    // A newer physical press supersedes this still-pending press. Do not let
    // the old callback overwrite the newer gesture's ownership or deliver an
    // old press after it.
    if (admission.superseded || (keyPress && !keyEvent->isAutoRepeat()
        && admission.generation != keyGenerations_.value(identity))) {
        event->accept();
        return true;
    }

    if (admission.pageCommandOwned) {
        acceptedKeys_.remove(identity);
        entryKeys_.remove(identity);
        for (const auto &release : admission.deferredReleases) {
            if (release.receiver && feasibilitySources_.contains(identity)) {
                QCoreApplication::sendEvent(release.receiver.data(), release.event.get());
                if (!alive) break;
            }
        }
        event->accept();
        return true;
    }

    // Once a browser press owns the gesture, an unmapped repeat cannot
    // become a native edit after the count/keychain or configuration changes.
    const bool handled = admission.accepted || (keyPress && keyEvent->isAutoRepeat()
                                      && acceptedKeys_.contains(identity));
    if (keyPress && !handled) {
        if (const auto *capture = FerricNativeSourceCapture::get(); capture && capture->contains(identity)) {
            // A newer owner was admitted inside the synchronous callback.
            // This older admission must not cancel or release that generation.
            event->accept();
            return true;
        }
    }
    if (keyPress && !handled && !keyEvent->isAutoRepeat()) {
        const bool owned = admitFeasibilitySource(keyEvent);
        if (!alive) {
            event->accept();
            return true;
        }
        if (owned) {
            acceptedKeys_.remove(identity);
            entryKeys_.remove(identity);
            // Releases seen in a nested admission loop follow the submitted
            // destination down. They never become original page key-ups.
            for (const auto &release : admission.deferredReleases) {
                if (release.receiver && feasibilitySources_.contains(identity)) {
                    QCoreApplication::sendEvent(release.receiver.data(), release.event.get());
                    if (!alive) break;
                }
            }
            event->accept();
            return true;
        }
    }
    if (admission.pageOwned && keyPress && !keyEvent->isAutoRepeat())
        pageOwnedKeys_.insert(identity);
    if (handled)
    {
        if (keyPress && !keyEvent->isAutoRepeat()) {
            acceptedKeys_.insert(keyIdentity(keyEvent));
            if (admission.bindingContext) {
                browserBindingContexts_.insert(identity, BrowserBindingContext {
                    *admission.bindingContext, admission.bindingContextCancelled
                });
                if (auto *capture = FerricNativeSourceCapture::get(true))
                    capture->claimBrowser(this, identity, admission.generation,
                        admission.sourceWindow.data(), admission.bindingContextCancelled);
            }
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
    if (!admission.deferredReleases.empty()) {
        // The native event must reach the window before its release. Forward
        // it here under the recursion guard; posting key-up can overtake it in
        // a nested event loop entered by a later Qt event filter.
        QPointer<QQuickWindow> window(qobject_cast<QQuickWindow *>(receiver.data()));
        QPointer<QQuickItem> focused(window ? window->activeFocusItem() : nullptr);
        if (!handled && window && targetWindow_ == window
            && QGuiApplication::focusWindow() == window) {
            QEvent *forwardingPrevious = forwardingEvent_;
            forwardingEvent_ = event;
            currentAdmission_ = &admission;
            QCoreApplication::sendEvent(receiver.data(), event);
            if (!alive)
                return true;
            currentAdmission_ = previous;
            forwardingEvent_ = forwardingPrevious;
        }
        for (auto &release : admission.deferredReleases) {
            if (!release.receiver || admission.superseded
                || admission.generation != keyGenerations_.value(identity))
                break;
            if (!handled && (release.receiver != receiver || !window || targetWindow_ != window
                || QGuiApplication::focusWindow() != window
                || window->activeFocusItem() != focused)) {
                keyGenerations_.remove(identity);
                pageOwnedKeys_.remove(identity);
                forwardedKeys_.remove(identity);
                if (passGesture_ && passGesture_->identity == identity) passGesture_.reset();
                if (enterGesture_ && enterGesture_->identity == identity) enterGesture_.reset();
                break;
            }
            QCoreApplication::sendEvent(release.receiver.data(), release.event.get());
            if (!alive)
                return true;
        }
        event->accept();
        return true;
    }
    return handled;
}

#if defined(FERRIC_NATIVE_PROBE)
bool FerricBrowserKeyRouter::submitFeasibilityPageKey(QQuickItem *view,
                                                     bool press, bool autoRepeat)
{
    return submitPageKey(view, FerricPageKeyDestination{}, press, autoRepeat);
}
#endif

bool FerricBrowserKeyRouter::submitPageKey(QQuickItem *view,
    const FerricPageKeyDestination &key, bool press, bool autoRepeat)
{
#if !defined(Q_OS_LINUX)
    Q_UNUSED(view);
    Q_UNUSED(key);
    Q_UNUSED(press);
    Q_UNUSED(autoRepeat);
    return false;
#else
    const QString platform = QGuiApplication::platformName();
    if (platform != QStringLiteral("xcb") && platform != QStringLiteral("wayland")
        && platform != QStringLiteral("offscreen"))
        return false;
    const QPointer<FerricBrowserKeyRouter> alive(this);
    const QPointer<QQuickWindow> window(qobject_cast<QQuickWindow *>(targetWindow_.data()));
    const QPointer<QQuickItem> page(view);
    if (!enabled_ || !window || !page || page->window() != window
        || !window->isActive() || QGuiApplication::focusWindow() != window
        || !page->isVisible())
        return false;
    const QPointer<QQuickItem> receiver(window->activeFocusItem());
    if (!receiver || (receiver != page && !page->isAncestorOf(receiver)))
        return false;

    PageKeyEvent destination(this, receiver, key, press, autoRepeat);
    QEvent *previous = pageKeyDelivery_;
    pageKeyDelivery_ = &destination;
    QCoreApplication::sendEvent(receiver.data(), &destination);
    if (alive)
        pageKeyDelivery_ = previous;
    return true;
#endif
}

#if defined(FERRIC_NATIVE_PROBE)
bool FerricBrowserKeyRouter::configureFeasibilityMapping(QQuickItem *view, bool enabled)
{
    const QPointer<FerricBrowserKeyRouter> alive(this);
    if (!enabled) {
        cancelFeasibilitySources(true);
        if (!alive) return false;
        feasibilityEnabled_ = false;
        return true;
    }
    const bool ready = bindPageReceiver(view);
    if (alive) feasibilityEnabled_ = ready;
    return ready;
}
#endif

bool FerricBrowserKeyRouter::pageReceiverKnown() const
{
    if (!pageReceiver_ || !focusBroker_->documentAssociationIsCurrent(pageReceiver_->document)) return false;
    const auto &state = pageReceiver_->state;
    return state.value(QStringLiteral("focused")).metaType().id() == QMetaType::Bool
        && state.value(QStringLiteral("focused")).toBool()
        && state.value(QStringLiteral("editable")).metaType().id() == QMetaType::Bool
        && !state.value(QStringLiteral("editable")).toBool()
        && state.value(QStringLiteral("kind")).toString() == QStringLiteral("noneditable");
}

QString FerricBrowserKeyRouter::pageReceiverKey() const
{
    return pageReceiverKnown() ? pageReceiver_->key : QString();
}

void FerricBrowserKeyRouter::probePageReceiver(QQuickItem *view, const QString &script, int epoch)
{
    if (epoch != inputEpoch_ || !view || view->window() != targetWindow_) return;
    if (focusProbePending_) {
        // One walk owns the broker at a time, including across input epochs.
        // Retain only the latest notification while that walk completes.
        nextFocusProbe_ = FocusProbeRequest{view, script, epoch};
        return;
    }
    if (!bindPageReceiver(view)) { finishPageReceiver(view, epoch, std::nullopt, {}); return; }
    const auto property = view->metaObject()->property(view->metaObject()->indexOfProperty("mainFrame"));
    const auto root = property.isValid() && property.isFinal() ? property.read(view).value<QWebEngineFrame>() : QWebEngineFrame{};
    const auto request = ++focusRequest_;
    const auto document = feasibilityDocument_;
    focusProbePending_ = true;
    const QPointer<FerricBrowserKeyRouter> alive(this);
    const QPointer<QQuickItem> page(view);
    focusBroker_->capturePageFocus(root, script,
        [alive, page, epoch, request, document, root](std::optional<FerricFrameBroker::DocumentReceipt> receipt, const QVariantMap &state) {
            if (!alive || request != alive->focusRequest_) return;
            alive->focusProbePending_ = false;
            auto next = std::move(alive->nextFocusProbe_);
            alive->nextFocusProbe_.reset();
            if (page && page == alive->feasibilityPage_ && document == alive->feasibilityDocument_
                && epoch == alive->inputEpoch_) {
                if (receipt && receipt->documents().first().frame != root) receipt.reset();
                alive->finishPageReceiver(page, epoch, std::move(receipt), state);
            }
            if (alive && next && request == alive->focusRequest_)
                alive->probePageReceiver(next->view, next->script, next->epoch);
        });
}

void FerricBrowserKeyRouter::finishPageReceiver(QQuickItem *view, int epoch,
    std::optional<FerricFrameBroker::DocumentReceipt> document, const QVariantMap &state)
{
    QString path = QStringLiteral("0");
    if (document) {
        const auto &chain = document->documents();
        for (int index = 1; index < chain.size(); ++index) {
            const auto child = chain[index - 1].frame.children().indexOf(chain[index].frame);
            if (child < 0) { document.reset(); break; }
            path += QLatin1Char('.') + QString::number(child);
        }
    }
    QJsonObject facts{{QStringLiteral("state"), QJsonValue(QJsonValue::Null)}};
    if (document && focusBroker_->documentAssociationIsCurrent(*document)) {
        const auto &leaf = document->documents().last();
        const auto key = leaf.owner + QLatin1Char(':') + leaf.epoch;
        pageReceiver_ = PageReceiver{key, *document, state, epoch};
        facts.insert(QStringLiteral("state"), QJsonObject::fromVariantMap(state));
        facts.insert(QStringLiteral("receiver"), QJsonObject{{QStringLiteral("key"), key},
            {QStringLiteral("origin"), leaf.origin}, {QStringLiteral("top"), document->documents().size() == 1}});
    } else pageReceiver_.reset();
    const auto identities = feasibilitySources_.keys();
    for (const auto identity : identities) {
        const auto source = feasibilitySources_.constFind(identity);
        if (source != feasibilitySources_.cend() && source->receiverDocument && !validFeasibilityReceiver(*source))
            cancelFeasibilitySource(identity, false);
    }
    emit pageReceiverObserved(view, epoch, path, QString::fromUtf8(QJsonDocument(facts).toJson(QJsonDocument::Compact)));
}

bool FerricBrowserKeyRouter::bindPageReceiver(QQuickItem *view)
{
    if (!view || !view->property("loading").isValid()
        || view->property("loading").toBool())
        return false;
    const QPointer<FerricBrowserKeyRouter> alive(this);
    const QPointer<QQuickItem> page(view);
    if (feasibilityPage_ == view && feasibilityReady_) return true;
    // Cancelled captures retain their old generation through release. They
    // must not prevent observation of a replacement document or another view.
    if (std::any_of(feasibilitySources_.cbegin(), feasibilitySources_.cend(),
        [](const auto &source) { return source.delivering; })) return false;
    if (feasibilityPage_ == view && feasibilityConnections_.size() >= 2) {
        feasibilityReady_ = bool(feasibilityConnections_[0]) && bool(feasibilityConnections_[1])
            && feasibilityDocument_ != std::numeric_limits<quint64>::max();
        return feasibilityReady_;
    }
    cancelFeasibilitySources(false);
    if (!alive || !page) return false;
    for (const auto &connection : feasibilityConnections_)
        QObject::disconnect(connection);
    feasibilityConnections_.clear();
    feasibilityPage_ = view;
    const auto url = connect(view, SIGNAL(urlChanged()), this, SLOT(invalidateFeasibilityDocument()));
    const auto loading = connect(view, SIGNAL(loadingChanged(QWebEngineLoadingInfo)),
                                 this, SLOT(invalidateFeasibilityDocument()));
    feasibilityConnections_.push_back(url);
    feasibilityConnections_.push_back(loading);
    feasibilityConnections_.push_back(connect(view, &QObject::destroyed,
                                              this, &FerricBrowserKeyRouter::invalidateFeasibilityDocument));
    invalidateFeasibilityDocument();
    if (!alive) return false;
    feasibilityReady_ = bool(url) && bool(loading)
        && feasibilityDocument_ != std::numeric_limits<quint64>::max();
    return feasibilityReady_;
}

void FerricBrowserKeyRouter::invalidateFeasibilityDocument()
{
    ++focusRequest_;
    focusProbePending_ = false;
    nextFocusProbe_.reset();
    pageReceiver_.reset();
    focusBroker_->clear();
    feasibilityRecovery_.reset();
    feasibilityReady_ = false;
    if (feasibilityDocument_ != std::numeric_limits<quint64>::max())
        ++feasibilityDocument_;
    cancelFeasibilitySources(false);
}

#if defined(FERRIC_NATIVE_PROBE)
void FerricBrowserKeyRouter::setFeasibilityInputBlocked(bool blocked)
{
    if (feasibilityInputBlocked_ == blocked) return;
    feasibilityInputBlocked_ = blocked;
    if (blocked) cancelFeasibilitySources(false);
}
#endif

bool FerricBrowserKeyRouter::canEnterRawPassThrough() const
{
    if (!targetWindow_ || !targetWindow_->isActive()
        || QGuiApplication::focusWindow() != targetWindow_ || compositionActive()
        || feasibilityInputBlocked_) return false;
    if (const auto *capture = FerricNativeSourceCapture::get(); capture && capture->held())
        return false;
    for (const auto &source : feasibilitySources_) {
        if (source.transformed || source.suppressed || source.cancelled)
            return false;
    }
    return true;
}

#if defined(FERRIC_NATIVE_PROBE)
bool FerricBrowserKeyRouter::requestFeasibilityRaw()
{
    if ((!passNextKey_ && !passThrough_) || !canEnterRawPassThrough()) return false;
    feasibilityRaw_ = true;
    return true;
}
#endif

QString FerricBrowserKeyRouter::beginPageKeyRecovery()
{
    const auto *admission = currentAdmission_;
    if (!admission || !admission->isPress || admission->isAutoRepeat || !admission->event
        || admission->event->key() != Qt::Key_Escape
        || admission->event->modifiers() != (Qt::ControlModifier | Qt::ShiftModifier)) return {};
    auto *capture = FerricNativeSourceCapture::get();
    if (!capture || !capture->armCancelledRecovery(targetWindow_))
        return QStringLiteral("No cancelled page key is available for capture; release held keys first");
    return QStringLiteral("Press and release the cancelled source key, then explicitly retry raw passthrough");
}

#if defined(FERRIC_NATIVE_PROBE)
bool FerricBrowserKeyRouter::beginFeasibilityRecovery(quint64 physicalIdentity)
{
    if (!enabled_ || QGuiApplication::focusWindow() != targetWindow_)
        return false;
    auto *capture = FerricNativeSourceCapture::get();
    return capture && capture->armRecovery(physicalIdentity, targetWindow_);
}
#endif

bool FerricBrowserKeyRouter::validFeasibilityReceiver(const FeasibilitySource &source) const
{
    return enabled_ && !feasibilityInputBlocked_ && feasibilityReady_
        && (source.policyContext == policyContext_ || (source.cancelled && source.cleanupPending))
        && (!source.receiverDocument || (pageReceiverKnown() && pageReceiver_->key == source.receiverKey
            && focusBroker_->documentAssociationIsCurrent(*source.receiverDocument)))
        && source.document == feasibilityDocument_ && source.page && source.receiver
        && source.window && source.page == feasibilityPage_ && source.window == targetWindow_
        && source.page->window() == source.window && source.page->isVisible()
        && source.window->isActive() && QGuiApplication::focusWindow() == source.window
        && source.window->activeFocusItem() == source.receiver
        && (source.page == source.receiver || source.page->isAncestorOf(source.receiver));
}

void FerricBrowserKeyRouter::retireFeasibilityOutput(const FeasibilitySource &source)
{
    auto output = feasibilityOutputs_.find(source.outputChannel);
    if (output == feasibilityOutputs_.end()) return;
    output->owners.remove(source.identity);
    if (output->owners.isEmpty()) feasibilityOutputs_.erase(output);
}

void FerricBrowserKeyRouter::cancelFeasibilitySource(quint64 identity, bool cleanup)
{
    auto source = feasibilitySources_.find(identity);
    if (source == feasibilitySources_.end()) return;
    if (!source->transformed && !source->suppressed && !source->receiverDocument) {
        if (!cleanup && !source->delivering) retireFeasibilityOutput(*source);
        return;
    }
    if (!source->transformed && source->receiverDocument && !source->suppressed) {
        source->suppressed = true;
        if (auto *capture = FerricNativeSourceCapture::get(true))
            capture->claim(this, identity, source->serial);
        source->committed = false; // unchanged input cannot be reconstructed for cleanup
    }
    source->cancelled = true;
    source->cleanupPending |= cleanup;
    if (!source->delivering)
        settleFeasibilitySource(identity, source->serial);
}

void FerricBrowserKeyRouter::cancelFeasibilitySources(bool cleanup)
{
    const QPointer<FerricBrowserKeyRouter> alive(this);
    const auto identities = feasibilitySources_.keys();
    for (const auto identity : identities) {
        cancelFeasibilitySource(identity, cleanup);
        if (!alive) return;
    }
}

void FerricBrowserKeyRouter::settleFeasibilitySource(quint64 identity, quint64 serial)
{
    auto source = feasibilitySources_.find(identity);
    if (source == feasibilitySources_.end() || source->serial != serial || source->delivering)
        return;
    if (source->pendingRelease) {
        finishFeasibilitySource(identity);
        return;
    }
    if (!source->cancelled) return;
    const auto snapshot = *source;
    const bool cleanup = snapshot.cleanupPending && snapshot.committed
        && validFeasibilityReceiver(snapshot);
    source->committed = false;
    source->cleanupPending = false;
    if (cleanup) {
        source->delivering = true;
        const QPointer<FerricBrowserKeyRouter> alive(this);
        submitPageKey(snapshot.page, snapshot.destination, false, false);
        if (!alive) return;
        source = feasibilitySources_.find(identity);
        if (source == feasibilitySources_.end() || source->serial != serial) return;
        source->delivering = false;
    }
    retireFeasibilityOutput(snapshot);
    if (source->pendingRelease) finishFeasibilitySource(identity);
}

void FerricBrowserKeyRouter::deliverFeasibilityPress(quint64 identity, bool repeat)
{
    auto source = feasibilitySources_.find(identity);
    if (source == feasibilitySources_.end()) return;
    if (!validFeasibilityReceiver(*source)) {
        cancelFeasibilitySource(identity, false);
        return;
    }
    const auto snapshot = *source;
    source->delivering = true;
    source->committed = true;
    const QPointer<FerricBrowserKeyRouter> alive(this);
    const bool submitted = submitPageKey(snapshot.page, snapshot.destination, true, repeat);
    if (!alive) return;
    source = feasibilitySources_.find(identity);
    if (source == feasibilitySources_.end() || source->serial != snapshot.serial) return;
    source->delivering = false;
    if (!submitted) {
        source->committed = false;
        source->cancelled = true;
    }
    settleFeasibilitySource(identity, snapshot.serial);
}

void FerricBrowserKeyRouter::finishFeasibilitySource(quint64 identity)
{
    auto source = feasibilitySources_.find(identity);
    if (source == feasibilitySources_.end() || source->delivering || source->finishing) return;
    const auto snapshot = *source;
    const bool sendRelease = snapshot.transformed && snapshot.committed
        && validFeasibilityReceiver(snapshot)
        && (!snapshot.cancelled || snapshot.cleanupPending);
    source->committed = false;
    source->delivering = true;
    source->finishing = true;
    const QPointer<FerricBrowserKeyRouter> alive(this);
    const bool released = sendRelease
        && submitPageKey(snapshot.page, snapshot.destination, false, false);
    if (!alive) return;
    source = feasibilitySources_.find(identity);
    if (source == feasibilitySources_.end() || source->serial != snapshot.serial) return;
    const bool completed = released && !source->cancelled;
    retireFeasibilityOutput(snapshot);
    if (auto *capture = FerricNativeSourceCapture::get())
        capture->release(this, identity, snapshot.serial);
    feasibilitySources_.erase(source);
    if (feasibilityRecovery_ == identity) feasibilityRecovery_.reset();
    keyGenerations_.remove(identity);
    nativeClipboardOverrides_.remove(identity);
    if (passGesture_ && passGesture_->identity == identity) passGesture_.reset();
    if (completed && snapshot.hintWorkspace)
        completeHintPageCommand(snapshot);
    else if (snapshot.oneShotOwner && passNextKey_ && inputEpoch_ == snapshot.epoch)
        completePassGesture(snapshot.epoch);
}

void FerricBrowserKeyRouter::completeHintPageCommand(const FeasibilitySource &source)
{
    if (!source.hintWorkspace || source.epoch != inputEpoch_ || source.policyContext != policyContext_)
        return;
    // Preserve other already-owned gestures across this intentional mode exit,
    // using the same completion guard as explicit one-key passthrough.
    const bool previous = completingPassGesture_;
    completingPassGesture_ = true;
    const QPointer<FerricBrowserKeyRouter> alive(this);
    QMetaObject::invokeMethod(source.hintWorkspace, "finishHintPageCommand", Qt::DirectConnection,
        Q_ARG(QVariant, QVariant(source.epoch)), Q_ARG(QVariant, QVariant(source.hintContext)));
    if (alive) completingPassGesture_ = previous;
}

void FerricBrowserKeyRouter::completePassGesture(int epoch)
{
    const bool previous = completingPassGesture_;
    completingPassGesture_ = true;
    const QPointer<FerricBrowserKeyRouter> alive(this);
    emit passNextKeyDispatched(epoch);
    if (alive) completingPassGesture_ = previous;
}

bool FerricBrowserKeyRouter::continueFeasibilitySource(QObject *watched, const QKeyEvent *event)
{
    if (event->type() == QEvent::KeyPress && protectedRecoveryKey(event)) return false;
    const auto identity = keyIdentity(event);
    auto source = feasibilitySources_.find(identity);
    if (source == feasibilitySources_.end()) return false;
    if (source->receiverDocument && !validFeasibilityReceiver(*source)) {
        cancelFeasibilitySource(identity, false);
        source = feasibilitySources_.find(identity);
        if (source == feasibilitySources_.end()) return true;
    }
    const bool release = event->type() == QEvent::KeyRelease;
    if (!source->transformed && !source->suppressed) {
        if (source->finishing) return true;
        if (release && !event->isAutoRepeat()) {
            const auto snapshot = *source;
            source->finishing = true;
            source->delivering = true;
            const bool selected = passGesture_ && passGesture_->identity == identity;
            const int epoch = selected ? passGesture_->epoch : 0;
            const QPointer<FerricBrowserKeyRouter> alive(this);
            QEvent *previous = forwardingEvent_;
            forwardingEvent_ = const_cast<QKeyEvent *>(event);
            QCoreApplication::sendEvent(watched, forwardingEvent_);
            if (!alive) return true;
            forwardingEvent_ = previous;
            source = feasibilitySources_.find(identity);
            bool completed = false;
            if (source != feasibilitySources_.end() && source->serial == snapshot.serial) {
                completed = source->committed && !source->cancelled;
                retireFeasibilityOutput(snapshot);
                feasibilitySources_.erase(source);
            }
            forwardedKeys_.remove(identity);
            pageOwnedKeys_.remove(identity);
            keyGenerations_.remove(identity);
            nativeClipboardOverrides_.remove(identity);
            if (selected && passGesture_ && passGesture_->identity == identity)
                passGesture_.reset();
            if (completed && snapshot.hintWorkspace)
                completeHintPageCommand(snapshot);
            else if (selected && passNextKey_ && inputEpoch_ == epoch)
                completePassGesture(epoch);
            return true;
        }
        if (!event->isAutoRepeat()) {
            retireFeasibilityOutput(*source);
            feasibilitySources_.erase(source);
        }
        return false;
    }
    if (release) {
        if (event->isAutoRepeat()) return true;
        if (source->cancelled && source->uncapturedFreshPress && !source->recovering)
            return true;
        if (source->recovering) {
            retireFeasibilityOutput(*source);
            if (auto *capture = FerricNativeSourceCapture::get())
                capture->release(this, identity, source->serial);
            feasibilitySources_.erase(source);
            if (feasibilityRecovery_ == identity) feasibilityRecovery_.reset();
            keyGenerations_.remove(identity);
            return true;
        }
        source->pendingRelease = true;
        if (!source->delivering) finishFeasibilitySource(identity);
        return true;
    }
    if (!event->isAutoRepeat() && source->cancelled) {
        source->recovering = feasibilityRecovery_ == identity && watched == feasibilityRecoveryWindow_
            && QGuiApplication::focusWindow() == feasibilityRecoveryWindow_;
        source->uncapturedFreshPress = !source->recovering;
        if (source->recovering) feasibilityRecovery_.reset();
        return true;
    }
    if (source->cancelled || source->suppressed || source->finishing) return true;
    if (!event->isAutoRepeat() || source->delivering
        || event->key() != source->originalKey || event->text() != source->originalText
        || event->modifiers() != source->modifiers) {
        if (!event->isAutoRepeat()) source->uncapturedFreshPress = true;
        cancelFeasibilitySource(identity, true);
        return true;
    }
    deliverFeasibilityPress(identity, true);
    return true;
}

bool FerricBrowserKeyRouter::observeCurrentPageKey(QQuickItem *view, const QString &source, bool raw)
{
    auto *admission = currentAdmission_;
    if (!admission || !admission->isPress || admission->isAutoRepeat
        || !admission->event || source.isEmpty() || !bindPageReceiver(view)) return false;
    const bool owned = admitFeasibilitySource(admission->event, source, raw);
    if (owned) admission->pageCommandOwned = true;
    return owned;
}

void FerricBrowserKeyRouter::consumeCurrentPageCommand()
{
    auto *admission = currentAdmission_;
    if (!admission || !admission->isPress || admission->isAutoRepeat
        || !admission->event || admission->pageCommandOwned) return;
    FeasibilitySource source;
    source.identity = admission->identity;
    source.serial = feasibilitySerial_ == std::numeric_limits<quint64>::max()
        ? feasibilitySerial_ : ++feasibilitySerial_;
    source.document = feasibilityDocument_;
    source.epoch = inputEpoch_;
    source.policyContext = policyContext_;
    source.originalKey = admission->event->key();
    source.originalText = admission->event->text();
    source.modifiers = admission->event->modifiers();
    source.cancelled = true;
    source.suppressed = true;
    if (auto *capture = FerricNativeSourceCapture::get(true)) {
        if (!capture->claim(this, source.identity, source.serial)) return;
    } else return;
    if (passNextKey_ && !passGesture_) {
        passGesture_ = KeyGesture{source.identity, inputEpoch_};
        source.oneShotOwner = true;
    }
    feasibilitySources_.insert(source.identity, source);
    admission->pageCommandOwned = true;
}

QString FerricBrowserKeyRouter::submitCurrentPageCommand(QQuickItem *view,
    const QString &destination, const QVariantMap &descriptor, bool transformed, QObject *hintWorkspace,
    const QString &hintContext, const QString &receiverKey)
{
    auto *admission = currentAdmission_;
    if (!admission || !admission->isPress || admission->isAutoRepeat
        || !admission->event || admission->accepted || admission->superseded
        || composing_ || feasibilityInputBlocked_)
        return QStringLiteral("Page command input is unavailable");
    const auto key = pageKeyDestination(destination, descriptor);
    if (transformed && !key)
        return QStringLiteral("Page command destination is not supported");
#if defined(Q_OS_LINUX)
    const auto backend = QGuiApplication::platformName();
    if (transformed && backend != QStringLiteral("xcb")
        && backend != QStringLiteral("wayland") && backend != QStringLiteral("offscreen"))
#else
    if (transformed)
#endif
        return QStringLiteral("Page command backend is not qualified");
    const QPointer<FerricBrowserKeyRouter> alive(this);
    if (!bindPageReceiver(view) || !alive)
        return QStringLiteral("Page command receiver is unavailable");
    if (!pageReceiverKnown() || receiverKey.isEmpty() || pageReceiver_->key != receiverKey
        || pageReceiver_->epoch != inputEpoch_)
        return QStringLiteral("Page command receiver observation is unavailable");
    FeasibilitySource source;
    source.identity = admission->identity;
    source.serial = feasibilitySerial_ == std::numeric_limits<quint64>::max()
        ? feasibilitySerial_ : ++feasibilitySerial_;
    source.document = feasibilityDocument_;
    source.epoch = inputEpoch_;
    source.policyContext = policyContext_;
    source.originalKey = admission->event->key();
    source.originalText = admission->event->text();
    source.modifiers = admission->event->modifiers();
    source.window = qobject_cast<QQuickWindow *>(targetWindow_.data());
    source.page = view;
    source.receiver = source.window ? source.window->activeFocusItem() : nullptr;
    source.transformed = transformed;
    source.receiverKey = receiverKey;
    source.receiverDocument = pageReceiver_->document;
    source.hintWorkspace = hintWorkspace;
    source.hintContext = hintContext;
    if (key) source.destination = *key;
    source.outputChannel = outputChannel(source.document, destination, receiverKey);
    if (!validFeasibilityReceiver(source)
        || feasibilitySerial_ == std::numeric_limits<quint64>::max())
        return QStringLiteral("Page command receiver changed");
    const auto output = feasibilityOutputs_.constFind(source.outputChannel);
    const auto unassociated = feasibilityOutputs_.constFind(outputChannel(source.document, destination));
    const bool collision = (output != feasibilityOutputs_.cend()
        && !output->owners.isEmpty() && (transformed || output->transformed))
        || (unassociated != feasibilityOutputs_.cend() && !unassociated->owners.isEmpty()
            && (transformed || unassociated->transformed));
    source.suppressed = collision || !transformed; // reserve unchanged input through commit
    source.delivering = !collision;
    if (transformed || collision) {
        auto *capture = FerricNativeSourceCapture::get(true);
        if (!capture || !capture->claim(this, source.identity, source.serial))
            return QStringLiteral("Page command source changed");
        admission->pageCommandOwned = true;
    }
    if (passNextKey_ && !passGesture_) {
        passGesture_ = KeyGesture{source.identity, inputEpoch_};
        source.oneShotOwner = true;
    }
    feasibilitySources_.insert(source.identity, source);
    if (collision) return QStringLiteral("Page command output is already held");
    {
        auto &channel = feasibilityOutputs_[source.outputChannel];
        channel.owners.insert(source.identity);
        channel.transformed |= transformed;
    }
    QVariant committed(true);
    const QPointer<QObject> hints(hintWorkspace);
    if (hints) {
        committed = false;
        if (!QMetaObject::invokeMethod(hints, "commitHintPageCommand", Qt::DirectConnection,
            Q_RETURN_ARG(QVariant, committed), Q_ARG(QVariant, QVariant(hintContext))))
            committed = false;
    }
    if (!alive) return QStringLiteral("owned");
    auto retained = feasibilitySources_.find(source.identity);
    if (retained == feasibilitySources_.end() || retained->serial != source.serial)
        return QStringLiteral("Page command source changed");
    retained->delivering = false;
    if (!committed.toBool() || (hintWorkspace && !hints) || admission->superseded
        || admission->generation != keyGenerations_.value(source.identity)
        || !validFeasibilityReceiver(*retained) || retained->cancelled) {
        retained->suppressed = true;
        retained->cancelled = true;
        retireFeasibilityOutput(*retained);
        if (!admission->pageCommandOwned) {
            auto *capture = FerricNativeSourceCapture::get(true);
            if (capture) capture->claim(this, source.identity, source.serial);
            admission->pageCommandOwned = true;
        }
        settleFeasibilitySource(source.identity, source.serial);
        return QStringLiteral("Page command was cancelled before delivery");
    }
    retained->suppressed = false;
    if (transformed) {
        deliverFeasibilityPress(source.identity, false);
        return QStringLiteral("owned");
    }
    retained->committed = true;
    admission->pageOwned = true;
    return QStringLiteral("forward");
}

bool FerricBrowserKeyRouter::admitFeasibilitySource(const QKeyEvent *event, const QString &trackedChannel, bool raw)
{
    const bool tracking = !trackedChannel.isEmpty();
#if defined(FERRIC_NATIVE_PROBE)
    if ((!feasibilityEnabled_ && !tracking) || feasibilityInputBlocked_ || composing_
        || (!passNextKey_ && !passThrough_ && !tracking) || event->isAutoRepeat()
        || (!tracking && event->modifiers() != Qt::NoModifier))
        return false;
    const bool transformed = !tracking && event->key() == Qt::Key_P
        && event->text() == QStringLiteral("p") && !feasibilityRaw_;
    if (!tracking && !transformed && event->key() != Qt::Key_K) return false;
    const auto channel = tracking ? trackedChannel : QStringLiteral("k");
#else
    if (!tracking || feasibilityInputBlocked_ || composing_ || event->isAutoRepeat()) return false;
    constexpr bool transformed = false;
    const auto &channel = trackedChannel;
#endif
    FeasibilitySource source;
    source.identity = keyIdentity(event);
    source.serial = feasibilitySerial_ == std::numeric_limits<quint64>::max()
        ? feasibilitySerial_ : ++feasibilitySerial_;
    source.document = feasibilityDocument_;
    source.epoch = inputEpoch_;
    source.policyContext = policyContext_;
    source.originalKey = event->key();
    source.originalText = event->text();
    source.modifiers = event->modifiers();
    source.window = qobject_cast<QQuickWindow *>(targetWindow_.data());
    source.page = feasibilityPage_;
    source.receiver = source.window ? source.window->activeFocusItem() : nullptr;
    source.transformed = transformed;
    source.outputChannel = outputChannel(source.document, channel,
        tracking && pageReceiverKnown() ? pageReceiver_->key : QString());
    source.raw = raw;
#if defined(FERRIC_NATIVE_PROBE)
    source.raw |= feasibilityRaw_;
#endif
    const bool valid = validFeasibilityReceiver(source)
        && feasibilitySerial_ != std::numeric_limits<quint64>::max();
    if (!valid && !transformed) return false;
    source.cancelled = !valid;
    if (valid) {
        const auto output = feasibilityOutputs_.constFind(source.outputChannel);
        source.suppressed = output != feasibilityOutputs_.cend() && !output->owners.isEmpty()
            && (transformed || output->transformed);
    }
    const bool owned = source.transformed || source.suppressed;
    if (owned) {
        auto *capture = FerricNativeSourceCapture::get(true);
        if (!capture || !capture->claim(this, source.identity, source.serial)) return true;
    }
    if (valid && !source.suppressed) {
        auto &channel = feasibilityOutputs_[source.outputChannel];
        channel.owners.insert(source.identity);
        channel.transformed |= transformed;
        source.committed = !transformed;
    }
    if (owned && passNextKey_ && !passGesture_) {
        passGesture_ = KeyGesture{source.identity, inputEpoch_};
        source.oneShotOwner = true;
    }
    feasibilitySources_.insert(source.identity, source);
    if (source.transformed && !source.cancelled && !source.suppressed)
        deliverFeasibilityPress(source.identity, false);
    return owned;
}
