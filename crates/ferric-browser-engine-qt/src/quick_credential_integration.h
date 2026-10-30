#pragma once
#include "profile_lifetime.h"
#include <QHash>
#include <QObject>
#include <QPointer>
#include <QtQml/qqmlregistration.h>

class FerricQuickCredentialBinding;

// App-owned profile adapter. attachView derives all owners from the registered
// native lifetime; neither QML arguments nor page text supply credential facts.
class FerricQuickCredentialIntegration : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(FerricCredentialIntegration)
    Q_PROPERTY(FerricProfileLifetime *lifetime READ lifetime WRITE setLifetime NOTIFY lifetimeChanged)
    Q_PROPERTY(QObject *requestInterceptor READ requestInterceptor WRITE setRequestInterceptor NOTIFY requestInterceptorChanged)
public:
    using QObject::QObject;
    ~FerricQuickCredentialIntegration() override;
    FerricProfileLifetime *lifetime() const { return lifetime_; }
    void setLifetime(FerricProfileLifetime *lifetime);
    QObject *requestInterceptor() const { return interceptor_; }
    void setRequestInterceptor(QObject *interceptor);
    Q_INVOKABLE bool attachView(QQuickItem *view);
signals:
    void lifetimeChanged();
    void requestInterceptorChanged();
private slots:
    void reconcile();
    void policyChanged();
private:
    struct Entry {
        QPointer<QQuickItem> view;
        QPointer<FerricQuickCredentialBinding> binding;
        FerricProfileLifetime::CurrentView owner;
        QString tab;
        QList<QMetaObject::Connection> watches;
        QList<QMetaObject::Connection> uiWatches;
        QPointer<QObject> watchedUi;
    };
    bool associationCurrent(QQuickItem *view, const FerricProfileLifetime::CurrentView &owner,
                             const QString &tab) const;
    QString profilePolicyIdentity(QObject *profile) const;
    void clear();
    QPointer<FerricProfileLifetime> lifetime_;
    QPointer<QObject> interceptor_;
    QList<QMetaObject::Connection> interceptorWatches_;
    quint64 interceptorIncarnation_ = 1;
    QList<QMetaObject::Connection> lifetimeWatches_;
    QHash<QQuickItem *, Entry> entries_;
    QString profileSession_;
    bool reconciling_ = false;
};
