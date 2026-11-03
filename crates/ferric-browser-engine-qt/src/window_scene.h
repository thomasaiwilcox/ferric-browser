#pragma once

#include <QPointer>
#include <QQuickItem>
#include <QQuickWidget>

class QQmlEngine;
class QQuickItem;
class QQuickWindow;

// Renders the existing QML window's visual contents inside a Widgets host.
// QObject ownership and QML contexts stay with the logical window. No live
// Quick WebEngine view may be inserted into this scene by the native backend.
class FerricWindowScene final : public QQuickWidget {
    Q_OBJECT
    Q_PROPERTY(bool active READ isHostActive NOTIFY hostActiveChanged)
    Q_PROPERTY(QQuickItem *activeFocusItem READ activeFocusItem NOTIFY activeFocusItemChanged)
    Q_PROPERTY(QWindow *nativeWindow READ nativeWindow NOTIFY nativeWindowChanged)
public:
    explicit FerricWindowScene(QQuickWindow *logicalWindow, QQmlEngine *engine,
                               QWidget *host);
    ~FerricWindowScene() override;

    bool isReady() const;
    bool isHostActive() const;
    QQuickItem *activeFocusItem() const;
    QWindow *nativeWindow() const;
    Q_INVOKABLE void requestClose();
    // Returns every surviving visual child before the widget owns its scene
    // root's destruction. It is safe to destroy the first presentation while
    // retaining application/profile resources in the logical QML window.
    void release();

signals:
    void hostActiveChanged();
    void activeFocusItemChanged();
    void nativeWindowChanged();

protected:
    bool eventFilter(QObject *watched, QEvent *event) override;
    void resizeEvent(QResizeEvent *event) override;

private:
    void adoptChildren();
    QPointer<QQuickWindow> logicalWindow_;
    QPointer<QQuickItem> sourceRoot_;
    QPointer<QQuickItem> sceneRoot_;
    QPointer<QWidget> host_;
    QMetaObject::Connection childrenChanged_;
    QMetaObject::Connection titleChanged_;
    bool moving_ = false;
    bool closing_ = false;
};
