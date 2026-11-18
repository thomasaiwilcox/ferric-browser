#ifndef FERRIC_BROWSER_BROWSER_KEY_ROUTER_H
#define FERRIC_BROWSER_BROWSER_KEY_ROUTER_H

#include "hint_frame_broker.h"

#include <QtCore/QObject>
#include <QtCore/QPointer>
#include <QtGui/QWindow>
#include <QtQml/qqmlregistration.h>
#include <QtCore/QSet>
#include <QtCore/QHash>
#include <QtCore/QVariantMap>
#include <QtQuick/QQuickItem>
#include <optional>
#include <memory>
#include <vector>

class QKeyEvent;
class QQuickWindow;
class FerricNativeSourceCapture;

// A virtual destination owns its metadata; it never borrows source scan codes.
struct FerricPageKeyDestination {
    int key = Qt::Key_K;
    QString text = QStringLiteral("k");
    Qt::KeyboardModifiers modifiers = Qt::NoModifier;
    quint32 scan = 45;
    quint32 symbol = 0x6b;
    quint32 nativeModifiers = 0;
};

class FerricBrowserKeyRouter : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(BrowserKeyRouter)
    Q_PROPERTY(bool transientInsert READ transientInsert WRITE setTransientInsert NOTIFY transientInsertChanged)
    Q_PROPERTY(bool passNextKey READ passNextKey WRITE setPassNextKey NOTIFY passNextKeyChanged)
    Q_PROPERTY(bool passThrough READ passThrough WRITE setPassThrough NOTIFY passThroughChanged)
    Q_PROPERTY(int inputEpoch READ inputEpoch WRITE setInputEpoch NOTIFY inputEpochChanged)
    Q_PROPERTY(QString policyContext READ policyContext WRITE setPolicyContext NOTIFY policyContextChanged)
    Q_PROPERTY(bool enabled READ enabled WRITE setEnabled NOTIFY enabledChanged)
    Q_PROPERTY(bool captureOnly MEMBER captureOnly_)
    Q_PROPERTY(QWindow *targetWindow READ targetWindow WRITE setTargetWindow NOTIFY targetWindowChanged)

public:
    explicit FerricBrowserKeyRouter(QObject *parent = nullptr);
    ~FerricBrowserKeyRouter() override;

    bool transientInsert() const { return transientInsert_; }
    void setTransientInsert(bool value);
    bool passNextKey() const { return passNextKey_; }
    void setPassNextKey(bool value);
    bool passThrough() const { return passThrough_; }
    void setPassThrough(bool value);
    int inputEpoch() const { return inputEpoch_; }
    void setInputEpoch(int value);
    QString policyContext() const { return policyContext_; }
    void setPolicyContext(const QString &context);

    bool enabled() const;
    void setEnabled(bool enabled);

    QWindow *targetWindow() const;
    void setTargetWindow(QWindow *window);

    Q_INVOKABLE void acceptCurrentEvent();
    // Stamp this exact pending admission before resolver dispatch. A stale
    // browser-owned repeat remains consumed through its terminal release.
    Q_INVOKABLE bool captureBindingContext(const QString &context);
    Q_INVOKABLE void pageOwnsCurrentEvent();
    Q_INVOKABLE bool activateSearchResult(QQuickItem *view, bool newTab);
    Q_INVOKABLE QString submitCurrentPageCommand(QQuickItem *view,
        const QString &destination, const QVariantMap &descriptor,
        bool transformed, QObject *hintWorkspace,
        const QString &hintContext, const QString &receiverKey);
    Q_INVOKABLE void probePageReceiver(QQuickItem *view, const QString &script, int epoch);
    QString pageReceiverKey() const;
    Q_INVOKABLE bool observeCurrentPageKey(QQuickItem *view, const QString &source, bool raw = false);
    bool canEnterRawPassThrough() const;
    Q_INVOKABLE QString beginPageKeyRecovery();
    Q_INVOKABLE void consumeCurrentPageCommand();
    Q_INVOKABLE bool compositionActive() const { return composing_ && !compositionReceiver_.isNull(); }

    quint64 feasibilityReturnedEvents() const { return feasibilityReturnedEvents_; }
#if defined(FERRIC_NATIVE_PROBE)
    // Standalone historical probes only. These controls are absent from the
    // application build; production admission uses compiled policy exclusively.
    bool submitFeasibilityPageKey(QQuickItem *view, bool press, bool autoRepeat = false);
    bool configureFeasibilityMapping(QQuickItem *view, bool enabled);
    bool requestFeasibilityRaw();
    bool beginFeasibilityRecovery(quint64 physicalIdentity);
    void setFeasibilityInputBlocked(bool blocked);
    bool feasibilityRaw() const { return feasibilityRaw_; }
#endif
    qsizetype feasibilitySourceCount() const { return feasibilitySources_.size(); }
    qsizetype feasibilityOutputCount() const { return feasibilityOutputs_.size(); }

signals:
    void pageReceiverObserved(QQuickItem *view, int epoch, const QString &framePath, const QString &facts);
    void transientInsertChanged();
    void inputEpochChanged();
    void policyContextChanged();
    void transientEnterDispatched(int epoch);
    void passNextKeyChanged();
    void passNextKeyDispatched(int epoch);
    void passThroughChanged();
    void enabledChanged();
    void targetWindowChanged();
    void keyPressed(
        const QString &text,
        int key,
        int modifiers,
        bool isAutoRepeat,
        bool browserOwnedRepeat);
    void keyReleased(int key, int modifiers);
    void clipboardShortcutOverride(int key, int modifiers);

protected:
    bool eventFilter(QObject *watched, QEvent *event) override;

private slots:
    void invalidateFeasibilityDocument();

private:
    friend class FerricNativeSourceCapture;
    struct FeasibilitySource {
        quint64 identity = 0;
        quint64 serial = 0;
        quint64 document = 0;
        int epoch = 0;
        QString policyContext;
        int originalKey = 0;
        QString originalText;
        Qt::KeyboardModifiers modifiers = Qt::NoModifier;
        FerricPageKeyDestination destination;
        QString outputChannel;
        QString receiverKey;
        std::optional<FerricFrameBroker::DocumentReceipt> receiverDocument;
        QPointer<QQuickWindow> window;
        QPointer<QQuickItem> page;
        QPointer<QQuickItem> receiver;
        QPointer<QObject> hintWorkspace;
        QString hintContext;
        bool transformed = false;
        bool raw = false;
        bool suppressed = false;
        bool cancelled = false;
        bool committed = false;
        bool finishing = false;
        bool delivering = false;
        bool pendingRelease = false;
        bool cleanupPending = false;
        bool oneShotOwner = false;
        bool recovering = false;
        bool uncapturedFreshPress = false;
    };
    struct FeasibilityOutput {
        QSet<quint64> owners;
        bool transformed = false;
    };
    bool admitFeasibilitySource(const QKeyEvent *event, const QString &trackedChannel = {}, bool raw = false);
    bool bindPageReceiver(QQuickItem *view);
    bool pageReceiverKnown() const;
    void finishPageReceiver(QQuickItem *view, int epoch,
        std::optional<FerricFrameBroker::DocumentReceipt> document, const QVariantMap &state);
    FerricFrameBroker *focusBroker_ = nullptr;
    struct PageReceiver {
        QString key;
        FerricFrameBroker::DocumentReceipt document;
        QVariantMap state;
        int epoch;
    };
    std::optional<PageReceiver> pageReceiver_;
    quint64 focusRequest_ = 0;
    bool focusProbePending_ = false;
    struct FocusProbeRequest {
        QPointer<QQuickItem> view;
        QString script;
        int epoch;
    };
    std::optional<FocusProbeRequest> nextFocusProbe_;
    bool submitPageKey(QQuickItem *view, const FerricPageKeyDestination &destination,
        bool press, bool autoRepeat);
    bool continueFeasibilitySource(QObject *watched, const QKeyEvent *event);
    bool validFeasibilityReceiver(const FeasibilitySource &source) const;
    void retireFeasibilityOutput(const FeasibilitySource &source);
    void cancelFeasibilitySource(quint64 identity, bool cleanup);
    void cancelFeasibilitySources(bool cleanup);
    void deliverFeasibilityPress(quint64 identity, bool repeat);
    void settleFeasibilitySource(quint64 identity, quint64 serial);
    void finishFeasibilitySource(quint64 identity);
    void completePassGesture(int epoch);
    void completeHintPageCommand(const FeasibilitySource &source);
    QHash<quint64, FeasibilitySource> feasibilitySources_;
    QHash<QString, FeasibilityOutput> feasibilityOutputs_;
    QPointer<QQuickItem> feasibilityPage_;
    std::vector<QMetaObject::Connection> feasibilityConnections_;
    QMetaObject::Connection feasibilityFocusConnection_;
    QMetaObject::Connection feasibilityActiveConnection_;
    quint64 feasibilityDocument_ = 1;
    quint64 feasibilitySerial_ = 0;
    std::optional<quint64> feasibilityRecovery_;
    QPointer<QWindow> feasibilityRecoveryWindow_;
    bool feasibilityEnabled_ = false;
    bool feasibilityReady_ = false;
    bool feasibilityRaw_ = false;
    bool feasibilityInputBlocked_ = false;
    bool completingPassGesture_ = false;
    quint64 inputEpochRequest_ = 0;
    quint64 passNextRequest_ = 0;
    quint64 passThroughRequest_ = 0;
    struct KeyGesture {
        quint64 identity;
        int epoch;
    };
    std::optional<KeyGesture> enterGesture_;
    std::optional<KeyGesture> passGesture_;
    QSet<quint64> entryKeys_;
    // Keys already delivered to the page retain their repeats and release,
    // including overlapping keys after the selected gesture has completed.
    QSet<quint64> forwardedKeys_;
    QEvent *forwardingEvent_ = nullptr;
    QEvent *pageKeyDelivery_ = nullptr;
    quint64 feasibilityReturnedEvents_ = 0;
    QPointer<QObject> compositionReceiver_;
    bool composing_ = false;
    bool transientInsert_ = false;
    bool passNextKey_ = false;
    bool passThrough_ = false;
    int inputEpoch_ = 0;
    QString policyContext_;
    bool enabled_ = false;
    bool captureOnly_ = false;
    // Each synchronous callback gets its own decision. Nested event delivery
    // must not overwrite the outer source's admission or leave a live pointer
    // after that callback returns.
    struct KeyAdmission {
        ~KeyAdmission();
        struct DeferredRelease {
            QPointer<QObject> receiver;
            std::unique_ptr<QEvent> event;
        };
        bool accepted = false;
        bool pageOwned = false;
        bool isPress = false;
        bool isAutoRepeat = false;
        std::optional<QString> bindingContext;
        bool bindingContextCancelled = false;
        const QKeyEvent *event = nullptr;
        bool pageCommandOwned = false;
        QPointer<QWindow> sourceWindow;
        quint64 identity = 0;
        quint64 generation = 0;
        KeyAdmission *previous = nullptr;
        // Pending callbacks can overlap across router instances. Preserve
        // supersession even if the newer router has already retired its key.
        bool superseded = false;
        bool sharedRegistered = false;
        KeyAdmission *sharedPrevious = nullptr;
        std::vector<DeferredRelease> deferredReleases;
    };
    void captureAbandonedAdmission(const KeyAdmission &admission);
    static void captureAbandonedSource(const KeyAdmission &admission);
    void finishBrowserSource(const QKeyEvent *event);
    void cancelBrowserSources();
    KeyAdmission *currentAdmission_ = nullptr;
    quint64 nextGeneration_ = 0;
    QHash<quint64, quint64> keyGenerations_;
    QPointer<QWindow> targetWindow_;
    QSet<quint64> nativeClipboardOverrides_;
    QSet<quint64> acceptedKeys_;
    struct BrowserBindingContext {
        QString value;
        bool cancelled = false;
    };
    QHash<quint64, BrowserBindingContext> browserBindingContexts_;
    QSet<quint64> pageOwnedKeys_;
};

#endif
