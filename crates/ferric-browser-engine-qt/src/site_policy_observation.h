#pragma once
#include "profile_lifetime.h"
#include <QObject>
#include <functional>
#include <QPointer>
#include <QQuickItem>
#include <QtQml/qqmlregistration.h>
class QUrl;

// Association observations only. Source compilation and eligibility remain in
// the application; no policy or positive-eligibility envelope enters QML.
class FerricSitePolicyObserver : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(FerricSitePolicyObserver)
    Q_PROPERTY(FerricProfileLifetime *lifetime READ lifetime WRITE setLifetime NOTIFY lifetimeChanged)
public:
    using QObject::QObject;
    FerricProfileLifetime *lifetime() const;
    void setLifetime(FerricProfileLifetime *lifetime);
signals:
    void lifetimeChanged();
private:
    void schedule(QQuickItem *view);
    QPointer<FerricProfileLifetime> lifetime_;
    QMetaObject::Connection association_;
};

// Widget hosts use their concrete QWebEnginePage URL and native registry proof.
// The callback is rechecked after opaque Rust capture and before completion.
void ferric_observe_public_site_policy(QObject *ui, const QString &tab,
                                      const std::function<bool()> &current,
                                      const std::function<QUrl()> &actualUrl);
