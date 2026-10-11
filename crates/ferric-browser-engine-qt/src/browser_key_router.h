#ifndef FERRIC_BROWSER_BROWSER_KEY_ROUTER_H
#define FERRIC_BROWSER_BROWSER_KEY_ROUTER_H

#include <QtCore/QObject>
#include <QtCore/QPointer>
#include <QtGui/QWindow>
#include <QtQml/qqmlregistration.h>
#include <QtCore/QSet>
#include <optional>

class FerricBrowserKeyRouter : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(BrowserKeyRouter)
    Q_PROPERTY(bool transientInsert READ transientInsert WRITE setTransientInsert NOTIFY transientInsertChanged)
    Q_PROPERTY(int inputEpoch READ inputEpoch WRITE setInputEpoch NOTIFY inputEpochChanged)
    Q_PROPERTY(bool enabled READ enabled WRITE setEnabled NOTIFY enabledChanged)
    Q_PROPERTY(QWindow *targetWindow READ targetWindow WRITE setTargetWindow NOTIFY targetWindowChanged)

public:
    explicit FerricBrowserKeyRouter(QObject *parent = nullptr);
    ~FerricBrowserKeyRouter() override;

    bool transientInsert() const { return transientInsert_; }
    void setTransientInsert(bool value);
    int inputEpoch() const { return inputEpoch_; }
    void setInputEpoch(int value);

    bool enabled() const;
    void setEnabled(bool enabled);

    QWindow *targetWindow() const;
    void setTargetWindow(QWindow *window);

    Q_INVOKABLE void acceptCurrentEvent();

signals:
    void transientInsertChanged();
    void inputEpochChanged();
    void transientEnterDispatched(int epoch);
    void enabledChanged();
    void targetWindowChanged();
    void keyPressed(const QString &text, int key, int modifiers, bool isAutoRepeat);
    void keyReleased(int key, int modifiers);

protected:
    bool eventFilter(QObject *watched, QEvent *event) override;

private:
    struct EnterGesture {
        quint64 identity;
        int epoch;
    };
    std::optional<EnterGesture> enterGesture_;
    QEvent *forwardingEvent_ = nullptr;
    QPointer<QObject> compositionReceiver_;
    bool composing_ = false;
    bool transientInsert_ = false;
    int inputEpoch_ = 0;
    bool enabled_ = false;
    bool dispatching_ = false;
    bool accepted_ = false;
    QPointer<QWindow> targetWindow_;
    QSet<quint64> acceptedKeys_;
};

#endif
