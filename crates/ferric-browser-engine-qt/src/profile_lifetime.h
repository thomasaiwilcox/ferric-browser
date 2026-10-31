#pragma once
#include <QHash>
#include <QList>
#include <QObject>
#include <QPointer>
#include <QQuickItem>
#include <QString>
#include <QUrl>
#include <QtQml/qqmlregistration.h>
#include <QMetaObject>
#include <QQmlEngine>
#include <QSet>
#include <optional>
#include <memory>
class FerricProfileTls;

// Native ownership only. Profile policy and request presentation stay with
// the existing runtime and its application-lived QML context.
class FerricProfileLifetime : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(FerricProfileLifetime)
    Q_PROPERTY(QObject *policyUi READ policyUi NOTIFY policyUiChanged)
    Q_PROPERTY(int viewCount READ viewCount NOTIFY consumersChanged)
    Q_PROPERTY(int windowCount READ windowCount NOTIFY consumersChanged)
    struct ViewTransferState;
  public:
    using QObject::QObject;
    ~FerricProfileLifetime() override;
    Q_INVOKABLE bool ownResource(QObject *resource);
    Q_INVOKABLE bool ownProfile(QObject *factory, QObject *profile);
    Q_INVOKABLE bool retainWindow(QObject *host, QObject *ui);
    Q_INVOKABLE bool activateWindow(QObject *host);
    Q_INVOKABLE bool adoptView(QQuickItem *view, QObject *host, QObject *ui);
    Q_INVOKABLE QString acquirePopupOpenerLease(QQuickItem *source,
                                                 QQuickItem *target);
    Q_INVOKABLE bool beginPopupOpenerLease(const QString &leaseId);
    Q_INVOKABLE bool confirmPopupOpenerLease(const QString &leaseId);
    Q_INVOKABLE bool cancelPopupOpenerLease(const QString &leaseId);
    Q_INVOKABLE int popupOpenerLeaseCount() const { return popupLeases_.size(); }
    Q_INVOKABLE int popupOpenerLeaseDelegateCount(const QString &leaseId) const;
    Q_INVOKABLE QObject *ownerUi(QQuickItem *view) const;
    Q_INVOKABLE QObject *ownerHost(QQuickItem *view) const;
    Q_INVOKABLE void releaseWindow(QObject *host);
    Q_INVOKABLE void releaseView(QQuickItem *view);
    Q_INVOKABLE void shutdown();
    QObject *policyUi() const { return policyUi_; }
    int viewCount() const { return views_.size(); }
    int windowCount() const { return windows_.size(); }
    // Native-only, corroborated ownership for credential adapters. No claimed
    // Ui/profile/tab envelope is accepted through this API.
    struct CurrentView {
        QPointer<QObject> host, ui, profile;
        quint64 incarnation = 0;
        quint64 windowGeneration = 0;
    };
    std::optional<CurrentView> currentView(QQuickItem *view) const;
    bool matchesCurrentView(QQuickItem *view, const CurrentView &expected) const;
    QString credentialTlsIdentity(QQuickItem *view) const;
    bool credentialTlsEligible(QQuickItem *view, const QUrl &origin) const;
    // Native-only exclusive staging. A moving view is unavailable to popup,
    // credential and ordinary adoption consumers until the transaction ends.
    class ViewTransfer {
      public:
        ~ViewTransfer();
        ViewTransfer(const ViewTransfer &) = delete;
        ViewTransfer &operator=(const ViewTransfer &) = delete;
      private:
        friend class FerricProfileLifetime;
        ViewTransfer() = default;
        std::shared_ptr<ViewTransferState> state;
    };
    std::unique_ptr<ViewTransfer> beginViewTransfer(QQuickItem *view,
                                                  QObject *destinationHost,
                                                  QObject *destinationUi);
    // Last native proof and association change. Call private Rust Core commit
    // immediately afterwards in the SAME native entry point, with no QML hop.
    bool associateViewTransfer(ViewTransfer &transfer);
    bool completeViewTransfer(ViewTransfer &transfer);
    // Restoration is permitted only after the caller proves the original
    // Core Target, then restores its actual view routing/scene. It mints a new
    // incarnation; old credential/prompt leases are never resurrected.
    bool cancelViewTransfer(ViewTransfer &transfer, bool restoreSource);
  signals:
    void viewRetiring(QQuickItem *view);
    void policyUiChanged();
    void consumersChanged();
    void drained();
    void popupOpenerLeaseReleased(const QString &leaseId);
    void credentialTransportChanged();

  private:
    QPointer<FerricProfileTls> tls_;
    struct Window {
        QObject *key;
        QPointer<QObject> host;
        QPointer<QObject> ui;
        QMetaObject::Connection hostDestroyed;
        QMetaObject::Connection uiDestroyed;
        bool ready = false;
        quint64 generation = 0;
    };
    struct ViewTransferState {
        QQuickItem *key = nullptr; // equality/cleanup only, never dereferenced
        QPointer<FerricProfileLifetime> lifetime;
        QPointer<QQuickItem> view;
        CurrentView source;
        QPointer<QObject> destinationHost, destinationUi;
        quint64 destinationGeneration = 0;
        quint64 stagedIncarnation = 0;
        QList<QMetaObject::Connection> watches;
        bool invalid = false, retire = false, associated = false, ended = false;
    };
    struct View {
        QPointer<QQuickItem> item;
        QObject *hostKey;
        QPointer<QObject> host;
        QPointer<QObject> ui;
        quint64 incarnation = 0;
        quint64 windowGeneration = 0;
        QMetaObject::Connection lifecycleChanged;
        QMetaObject::Connection renderProcessPidChanged;
        bool retiring = false;
        bool requireDiscarded = false;
        bool deleteQueued = false;
    };
    struct ChildWatch {
        QPointer<QQuickItem> item;
        QMetaObject::Connection childrenChanged;
    };
    struct DelegateWatch {
        QPointer<QObject> item;
        QMetaObject::Connection destroyed;
    };
    struct PopupLease {
        QString id;
        quint64 generation = 0;
        QQuickItem *sourceKey = nullptr;
        QQuickItem *targetKey = nullptr;
        QPointer<QQuickItem> source;
        QPointer<QQuickItem> target;
        QPointer<QObject> profile;
        QPointer<QQmlEngine> engine;
        quint64 sourceIncarnation = 0;
        quint64 targetIncarnation = 0;
        bool targetRetiring = false;
        bool sourceDestroyed = false;
        bool targetDestroyed = false;
        bool sawDelegate = false;
        bool adoptionStarted = false;
        bool adoptionConfirmed = false;
        QHash<QQuickItem *, ChildWatch> childWatches;
        QHash<QObject *, DelegateWatch> delegates;
    };
  private slots:
    void onRetiredViewStateChanged();
    void invalidateViewTransfer();

  private:
    void refresh();
    void reconcilePopupLease(const QString &leaseId, quint64 generation);
    void onPopupDelegateDestroyed(const QString &leaseId, quint64 generation,
                                  quint64 targetIncarnation, QObject *delegate);
    void maybeReleasePopupLease(const QString &leaseId, quint64 generation);
    void maybeDeleteRetiredView(QQuickItem *view);
    void watchRetiredViewState(QQuickItem *view);
    bool isCurrentView(QQuickItem *view, QObject **profile,
                       QQmlEngine **engine) const;
    bool isDiscardedAndRendererFree(QQuickItem *view) const;
    bool isPopupDelegate(QObject *object) const;
    int outgoingPopupLeaseCount(QQuickItem *view) const;
    void removePopupLease(PopupLease &lease);
    bool popupLeaseIsCurrent(const PopupLease &lease) const;
    bool transferWindowsCurrent(const ViewTransferState &transfer) const;
    void removeViewTransfer(ViewTransferState &transfer);
    bool cancelViewTransfer(const std::shared_ptr<ViewTransferState> &transfer, bool restoreSource);
    QList<Window> windows_;
    QHash<QQuickItem *, View> views_;
    QHash<QString, PopupLease> popupLeases_;
    QHash<QQuickItem *, std::shared_ptr<ViewTransferState>> viewTransfers_;
    QPointer<QObject> policyUi_;
    QPointer<QObject> ownedProfile_;
    QObject *policyUiKey_ = nullptr;
    bool consumed_ = false;
    bool drainQueued_ = false;
    bool drained_ = false;
    bool shuttingDown_ = false;
    quint64 nextGeneration_ = 0;
    quint64 nextPopupLeaseId_ = 0;
};
