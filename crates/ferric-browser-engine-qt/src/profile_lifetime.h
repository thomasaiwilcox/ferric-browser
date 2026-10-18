#pragma once
#include <QHash>
#include <QList>
#include <QObject>
#include <QPointer>
#include <QQuickItem>
#include <QtQml/qqmlregistration.h>

// Native ownership only. Profile policy and request presentation stay with
// the existing runtime and its application-lived QML context.
class FerricProfileLifetime : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(FerricProfileLifetime)
    Q_PROPERTY(QObject *policyUi READ policyUi NOTIFY policyUiChanged)
    Q_PROPERTY(int viewCount READ viewCount NOTIFY consumersChanged)
    Q_PROPERTY(int windowCount READ windowCount NOTIFY consumersChanged)
  public:
    using QObject::QObject;
    ~FerricProfileLifetime() override;
    Q_INVOKABLE bool ownResource(QObject *resource);
    Q_INVOKABLE bool ownProfile(QObject *factory, QObject *profile);
    Q_INVOKABLE bool retainWindow(QObject *host, QObject *ui);
    Q_INVOKABLE bool activateWindow(QObject *host);
    Q_INVOKABLE bool adoptView(QQuickItem *view, QObject *host, QObject *ui);
    Q_INVOKABLE QObject *ownerUi(QQuickItem *view) const;
    Q_INVOKABLE QObject *ownerHost(QQuickItem *view) const;
    Q_INVOKABLE void releaseWindow(QObject *host);
    Q_INVOKABLE void releaseView(QQuickItem *view);
    Q_INVOKABLE void shutdown();
    QObject *policyUi() const { return policyUi_; }
    int viewCount() const { return views_.size(); }
    int windowCount() const { return windows_.size(); }
  signals:
    void viewRetiring(QQuickItem *view);
    void policyUiChanged();
    void consumersChanged();
    void drained();

  private:
    struct Window {
        QObject *key;
        QPointer<QObject> host;
        QPointer<QObject> ui;
        QMetaObject::Connection hostDestroyed;
        QMetaObject::Connection uiDestroyed;
        bool ready = false;
    };
    struct View {
        QPointer<QQuickItem> item;
        QObject *hostKey;
        QPointer<QObject> host;
        QPointer<QObject> ui;
        bool retiring = false;
    };
    void refresh();
    QList<Window> windows_;
    QHash<QQuickItem *, View> views_;
    QPointer<QObject> policyUi_;
    QObject *policyUiKey_ = nullptr;
    bool consumed_ = false;
    bool drainQueued_ = false;
    bool drained_ = false;
    bool shuttingDown_ = false;
};
