#ifndef RUSTBROWSER_BROWSER_KEY_ROUTER_H
#define RUSTBROWSER_BROWSER_KEY_ROUTER_H

#include <QtCore/QObject>
#include <QtCore/QPointer>
#include <QtGui/QWindow>
#include <QtQml/qqmlregistration.h>

class RustBrowserKeyRouter : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(BrowserKeyRouter)
    Q_PROPERTY(bool enabled READ enabled WRITE setEnabled NOTIFY enabledChanged)
    Q_PROPERTY(QWindow *targetWindow READ targetWindow WRITE setTargetWindow NOTIFY targetWindowChanged)

public:
    explicit RustBrowserKeyRouter(QObject *parent = nullptr);
    ~RustBrowserKeyRouter() override;

    bool enabled() const;
    void setEnabled(bool enabled);

    QWindow *targetWindow() const;
    void setTargetWindow(QWindow *window);

    Q_INVOKABLE void acceptCurrentEvent();

signals:
    void enabledChanged();
    void targetWindowChanged();
    void keyPressed(const QString &text, int key, int modifiers);

protected:
    bool eventFilter(QObject *watched, QEvent *event) override;

private:
    bool enabled_ = false;
    bool dispatching_ = false;
    bool accepted_ = false;
    QPointer<QWindow> targetWindow_;
};

#endif
