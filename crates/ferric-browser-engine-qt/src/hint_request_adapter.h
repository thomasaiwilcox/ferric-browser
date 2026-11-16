#pragma once
#include "profile_lifetime.h"
#include "hint_frame_broker.h"
#include <QJSValue>
#include <QVariant>

// One opaque original Rust capture + native ownership/document receipts.
// QML may carry the value, but a map/JSON or another view cannot construct it.
// Create this as a QML object so callbacks use its owning script engine.
class FerricHintRequestAdapter : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(FerricHintRequestAdapter)
    Q_PROPERTY(FerricProfileLifetime *lifetime READ lifetime WRITE setLifetime NOTIFY lifetimeChanged)
public:
    using QObject::QObject;
    FerricProfileLifetime *lifetime() const { return lifetime_; }
    quint64 configurationGeneration() const { return configurationGeneration_; }
    void setLifetime(FerricProfileLifetime *value);
    // Begin synchronously before even the provider's asynchronous attachment.
    // Bind only its native catalogue later; never recapture policy at bind time.
    Q_INVOKABLE QVariant begin(QQuickItem *view, FerricHintFrameBroker *broker);
    // Classification and token come from the same synchronous capture. An
    // empty/failed capture is never interpreted as ordinary discovery.
    Q_INVOKABLE QVariantMap beginCollection(QQuickItem *view, FerricHintFrameBroker *broker,
                                            const QString &scopedOwner = QString());
    Q_INVOKABLE QVariantMap beginPreview(QQuickItem *view, FerricHintFrameBroker *broker,
                                         const QString &preview, const QString &family);
    Q_INVOKABLE QString publishPreview(const QVariant &handle);
    Q_INVOKABLE QVariantMap beginCapture(QQuickItem *view, FerricHintFrameBroker *broker,
                                         const QString &capture, const QString &family);
    Q_INVOKABLE QString builderCandidate(const QVariant &handle, const QString &label) const;
    Q_INVOKABLE QString collectionState(const QVariant &handle) const;
    // Strict leaf admission for a non-Top owner. Combined collection must keep
    // independently valid Top work when an optional receiver is unsupported;
    // this API's failed selected binding is not a Top-proof rejection.
    Q_INVOKABLE void bindDocuments(const QVariant &handle, const QString &owner,
                                   const QJSValue &callback);
    // Independent Top draft, then optional focused admission and, on fallback,
    // a seal of the original Top receipt: at most three <=1000ms stages. The
    // frozen entered-frame owner is mandatory and can never fall back to Top.
    Q_INVOKABLE void bindCollection(const QVariant &handle, const QString &topOwner,
                                    const QString &selectedOwner, const QJSValue &callback);
    Q_INVOKABLE void capture(QQuickItem *view, FerricHintFrameBroker *broker,
                             const QString &owner, const QJSValue &callback);
    // One explicit document revalidation per original capture. Refresh needs
    // a new capture; this is not a live action permit or a renewable allowance.
    Q_INVOKABLE void validate(const QVariant &handle, const QJSValue &callback);
    Q_INVOKABLE QString collectorPlan(const QVariant &handle) const;
    // Only canonical Top or the admitted selected foreign collector. Native
    // ancestors remain structural and cannot obtain a collector plan.
    Q_INVOKABLE QString collectorPlanForOwner(const QVariant &handle, const QString &owner) const;
    Q_INVOKABLE QString collectorSelectors(const QVariant &handle) const;
    // Request-owned ordinary inventories and remaining grants only. No raw
    // graph has been sealed or classified, and no Hint session is published.
    Q_INVOKABLE bool beginCollectionWork(const QVariant &handle);
    Q_INVOKABLE QString collectionWorkPlan(const QVariant &handle) const;
    Q_INVOKABLE QString collectionGeometry(const QVariant &handle, const QString &owner) const;
    // Uses retained native projections and Rust inventories only. QML may
    // exclude Top for the existing foreign modal/entered-frame presentation.
    Q_INVOKABLE QString finishCollection(const QVariant &handle, bool includeTop = true);
    Q_INVOKABLE QString publishCollection(const QVariant &handle);
    Q_INVOKABLE QVariantMap beginSelection(QQuickItem *view, FerricHintFrameBroker *broker,
        const QString &label, const QString &scopedOwner = QString());
    Q_INVOKABLE void confirmSelection(const QVariant &handle, const QJSValue &callback);
    Q_INVOKABLE QString cancelSelection(const QVariant &handle);
    Q_INVOKABLE bool acceptCollectionOrdinary(const QVariant &handle, const QString &stage,
        const QString &owner, const QString &candidates, const QString &metadata);
    // Seal a raw LOCAL graph against its internally selected original receipt.
    // This retains data only: no projection, normalization or checked status.
    Q_INVOKABLE void verifyCollectionPolicy(const QVariant &handle, const QString &stage,
        const QString &owner, const QString &annotation, const QString &metadata, const QJSValue &callback);
    Q_INVOKABLE bool rejectCollectionWork(const QVariant &handle, const QString &stage, const QString &owner);
    // Stages facts using the original Rust request, then associates the RAW
    // document table with its captured native world. This does not authorize
    // roots, focused receiver roles, labels or actions.
    Q_INVOKABLE void verifyObservationDocuments(const QVariant &handle, const QString &candidates,
                                               const QString &annotation, const QString &batch,
                                               const QString &revision, const QJSValue &callback);
    Q_INVOKABLE bool observationDocumentsChecked(const QVariant &handle) const;
    Q_INVOKABLE void verifyObservationObjects(const QVariant &handle, const QString &candidates,
                                               const QString &annotation, const QString &batch,
                                               const QString &revision, const QJSValue &callback);
    Q_INVOKABLE bool observationObjectsChecked(const QVariant &handle) const;
signals:
    void lifetimeChanged();
private:
    QVariant beginImpl(QQuickItem *view, FerricHintFrameBroker *broker,
                       const QString &scopedOwner, bool collection, QString &state,
                       const QString &preview = QString(), const QString &family = QString());
    void verifyObservation(const QVariant &handle, const QString &candidates,
                           const QString &annotation, const QString &batch,
                           const QString &revision, const QJSValue &callback, bool objects);
    void deliver(const QJSValue &callback, const QVariant &value);
    QPointer<FerricProfileLifetime> lifetime_;
    quint64 configurationGeneration_ = 0;
};
