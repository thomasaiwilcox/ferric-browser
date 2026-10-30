#pragma once

#include <QObject>
#include <QPointer>
#include <QString>
#include <memory>
#include <functional>

class QQuickItem;
class FerricCredentialBoundary;
class FerricQuickCredentialOwner;

// App-owned adapter. Not registered with QML or exposed on WebChannel. The
// actual view and application owner must be supplied by native integration.
class FerricQuickCredentialBinding final : public QObject {
    Q_OBJECT
public:
    static FerricQuickCredentialBinding *create(QQuickItem *view, QObject *ui,
                                                const QString &tabId,
                                                std::function<bool()> associationCurrent = {},
                                                std::function<QString()> profilePolicyIdentity = {});
    ~FerricQuickCredentialBinding() override;
    bool isReady() const;
    bool hasCurrentLease() const;
    FerricCredentialBoundary *boundary() const;
signals:
    void requestReady();
    void leaseInvalidated();
private slots:
    void applicationChanged();
private:
    friend class FerricQuickCredentialIntegration;
    explicit FerricQuickCredentialBinding(QQuickItem *view, QObject *ui,
                                          const QString &tabId,
                                          std::function<bool()> associationCurrent,
                                          std::function<QString()> profilePolicyIdentity);
    void capture();
    void clearLease();
    struct State;
    std::unique_ptr<State> state_;
    QPointer<QQuickItem> view_;
    QPointer<QObject> ui_;
    QString tabId_;
    std::function<bool()> associationCurrent_;
    std::function<QString()> profilePolicyIdentity_;
    QPointer<FerricQuickCredentialOwner> owner_;
    QPointer<FerricCredentialBoundary> boundary_;
};
