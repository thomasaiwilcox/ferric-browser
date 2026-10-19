#ifndef FERRIC_BROWSER_BROWSER_KEY_ROUTER_H
#define FERRIC_BROWSER_BROWSER_KEY_ROUTER_H

#include <QtCore/QObject>
#include <QtCore/QPointer>
#include <QtGui/QWindow>
#include <QtQml/qqmlregistration.h>
#include <QtCore/QSet>
#include <QtQuick/QQuickItem>
#include <optional>

class FerricBrowserKeyRouter : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(BrowserKeyRouter)
    Q_PROPERTY(bool transientInsert READ transientInsert WRITE setTransientInsert NOTIFY transientInsertChanged)
    Q_PROPERTY(bool passNextKey READ passNextKey WRITE setPassNextKey NOTIFY passNextKeyChanged)
    Q_PROPERTY(bool passThrough READ passThrough WRITE setPassThrough NOTIFY passThroughChanged)
    Q_PROPERTY(int inputEpoch READ inputEpoch WRITE setInputEpoch NOTIFY inputEpochChanged)
    Q_PROPERTY(bool enabled READ enabled WRITE setEnabled NOTIFY enabledChanged)
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

    bool enabled() const;
    void setEnabled(bool enabled);

    QWindow *targetWindow() const;
    void setTargetWindow(QWindow *window);

    Q_INVOKABLE void acceptCurrentEvent();
    Q_INVOKABLE void pageOwnsCurrentEvent();
    Q_INVOKABLE bool activateSearchResult(QQuickItem *view, bool newTab);

signals:
    void transientInsertChanged();
    void inputEpochChanged();
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

private:
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
    QPointer<QObject> compositionReceiver_;
    bool composing_ = false;
    bool transientInsert_ = false;
    bool passNextKey_ = false;
    bool passThrough_ = false;
    int inputEpoch_ = 0;
    bool enabled_ = false;
    bool dispatching_ = false;
    bool accepted_ = false;
    bool pageOwnedCurrentEvent_ = false;
    QPointer<QWindow> targetWindow_;
    QSet<quint64> nativeClipboardOverrides_;
    QSet<quint64> acceptedKeys_;
    QSet<quint64> pageOwnedKeys_;
};

#endif
