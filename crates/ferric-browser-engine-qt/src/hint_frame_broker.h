#ifndef FERRIC_HINT_FRAME_BROKER_H
#define FERRIC_HINT_FRAME_BROKER_H

#include "hint_graph_work.h"

#include <QtCore/QHash>
#include <QtCore/QDeadlineTimer>
#include <QtCore/QJsonArray>
#include <QtCore/QJsonObject>
#include <QtCore/QJsonValue>
#include <QtCore/QObject>
#include <QtCore/QPointer>
#include <QtCore/QSizeF>
#include <QtCore/QUrl>
#include <QtQml/QJSValue>
#include <QtQml/qqmlregistration.h>
#include <QtWebEngineCore/QWebEngineFrame>
#include <functional>
#include <memory>
#include <optional>
#include <utility>

// Handles and document leases stay in the browser process. No page payload can
// select a native frame by its URL, name, position or enumeration index.
class FerricFrameDispatcher;
class FerricFrameBroker : public QObject {
    Q_OBJECT
    QML_ANONYMOUS
    struct FragmentCatalogueData;
    struct AncestryLease;
public:
    // Native-only captured facts. Receiver admission, selected-view ownership
    // and the retained Rust request must still be checked by the caller.
    class DocumentReceipt {
    public:
        struct Document {
            QString owner, parent, epoch, origin;
            QWebEngineFrame frame;
            QUrl url;
            bool focused = false;
        };
        DocumentReceipt(const DocumentReceipt &) = default;
        DocumentReceipt(DocumentReceipt &&) = default;
        DocumentReceipt &operator=(const DocumentReceipt &) = default;
        const QList<Document> &documents() const { return documents_; }
        // Native frame association stays documents().size()-1. This separate
        // value exists only after exact focused-parent ancestry capture.
        std::optional<quint8> composedDepth() const;
    private:
        friend class FerricFrameBroker;
        DocumentReceipt() = default;
        QPointer<FerricFrameBroker> broker_;
        quint64 generation_ = 0;
        QList<Document> documents_;
        std::shared_ptr<AncestryLease> ancestry_;
    };
    using ReceiptCompletion = std::function<void(std::optional<DocumentReceipt>)>;
    using FocusCompletion = std::function<void(std::optional<DocumentReceipt>, const QVariantMap &)>;
    // One bounded asynchronous observation of the actual focused native chain.
    // Input delivery consumes the cached receipt, never a per-key DOM query.
    void capturePageFocus(const QWebEngineFrame &mainFrame, const QString &script, FocusCompletion completion);
    struct FocusedDocuments {
        DocumentReceipt selected;
        QList<quint32> requiredDocuments;
    };
    // Exact explicit scope, independent of focused-root obligations. The
    // caller must retain and validate the original browser collection issuer.
    struct EnteredDocument {
        DocumentReceipt selected;
    };
    // Native expectation from the original pending policy slot. Verification
    // of this debit alone is not a fragment catalogue/projection receipt.
    struct AnnotationDebit {
        quint32 reported;
        quint32 granted;
    };
    struct DocumentWork {
        FerricHintDocumentAdmission admission;
        QString checkpoint;
    };
    struct FragmentRequest {
        DocumentReceipt world;
        std::optional<FocusedDocuments> focused;
        QString batch, stamp;
        quint64 revision;
        QString annotation, scope, plan, ordinaryGeometry;
        AnnotationDebit debit;
        QSizeF viewport;
        QString ordinaryInput = {};
        // Original-request callbacks, never renderer/QML input. The retained
        // request must hold its owner weakly to avoid a catalogue/handle cycle.
        FerricHintGraphAdmission graphAdmission = {};
        FerricHintDocumentAdmission documentAdmission = {};
        std::optional<EnteredDocument> entered = std::nullopt;
    };
    // Immutable captured-original facts, not fresh geometry, parent exposure
    // or an action permit. Only this broker can create an associated snapshot.
    class FragmentCatalogue {
    public:
        FragmentCatalogue(const FragmentCatalogue &) = default;
        FragmentCatalogue(FragmentCatalogue &&) = default;
        FragmentCatalogue &operator=(const FragmentCatalogue &) = default;
        const QString &json() const;
        const QJsonArray &targets() const;
    private:
        friend class FerricFrameBroker;
        explicit FragmentCatalogue(std::shared_ptr<const FragmentCatalogueData> data) : data_(std::move(data)) {}
        std::shared_ptr<const FragmentCatalogueData> data_;
    };
    using FragmentCompletion = std::function<void(std::optional<FragmentCatalogue>)>;
    explicit FerricFrameBroker(QObject *parent, const QString &documentProperty);
    ~FerricFrameBroker() override;
    Q_INVOKABLE void attach(const QWebEngineFrame &frame, const QJSValue &callback);
    Q_INVOKABLE void focusedChild(const QString &parent, const QJSValue &callback);
    // Proves terminal focused ownership without attaching a deeper receiver.
    Q_INVOKABLE void verifyFocusedLeaf(const QString &owner, const QJSValue &callback);
    Q_INVOKABLE void run(const QString &owner, const QString &script, const QJSValue &callback);
    Q_INVOKABLE void dispatch(const QString &owner, const QString &script, const QJSValue &callback);
    // Wait only for scripts already outstanding; expiry does not cancel WebEngine.
    Q_INVOKABLE void waitForPendingScripts(const QJSValue &callback);
    Q_INVOKABLE void clear();
    // Deliberately not Qt invokables: JSON/QML values cannot mint receipts.
    void captureDocument(const QString &owner, ReceiptCompletion completion, int timeoutMs = 1000,
                         std::optional<DocumentWork> work = std::nullopt);
    void captureComposedAncestry(const DocumentReceipt &receipt, ReceiptCompletion completion, int timeoutMs = 1000,
                                FerricHintDocumentAdmission admission = {});
    void revalidateDocument(const DocumentReceipt &receipt, std::function<void(bool)> completion,
                            int timeoutMs = 1000, std::optional<DocumentWork> work = std::nullopt);
    // Selected collector admission, not merely an ancestor with hasFocus().
    void revalidateFocusedDocument(const DocumentReceipt &receipt, std::function<void(bool)> completion,
                                   int timeoutMs = 1000, FerricHintDocumentAdmission admission = {});
    // Preserve the captured Top fact while keeping selected-leaf facts separate.
    // This creates no new native frame or epoch and still requires revalidation.
    std::optional<DocumentReceipt> topDocumentReceipt(const DocumentReceipt &receipt) const;
    // Native continuation only. These bounded local IDs are checked against
    // exact weak DOM objects under this receipt's world, not treated as leases.
    // The caller must separately validate the original request and all roots,
    // witnesses, scope and receiver roles. Success is not an action permit.
    void validateWorldDocuments(const DocumentReceipt &receipt, const QString &batch,
                                const QString &stamp, quint64 revision,
                                const QString &documentsJson, std::function<void(bool)> completion,
                                int timeoutMs = 1000);
    // Required IDs are derived from the browser policy before normalization.
    // A real selected receipt, not renderer origin/owner text, supplies identity.
    void validateFocusedWorldDocuments(const DocumentReceipt &world, FocusedDocuments focused,
                                const QString &batch, const QString &stamp, quint64 revision,
                                const QString &documentsJson, std::function<void(bool)> completion,
                                int timeoutMs = 1000);
    // Original structurally checked annotation plus original browser scope.
    // This seals world-local graph identities, not semantic ranks/actions.
    // Production policy seals also require originalPlan from the retained
    // native request. Omitting it performs graph association only.
    void validateWorldObjects(const DocumentReceipt &world, std::optional<FocusedDocuments> focused,
                                const QString &batch, const QString &stamp, quint64 revision,
                                const QString &annotationJson, const QString &scopeJson,
                                std::function<void(bool)> completion, int timeoutMs = 1000,
                                std::optional<EnteredDocument> entered = std::nullopt,
                                std::optional<QString> originalPlan = std::nullopt,
                                std::optional<AnnotationDebit> annotationDebit = std::nullopt);
    // Like document capture, clear/destruction cancels and suppresses pending
    // callbacks. The request owner must retire UI work on its epoch change.
    void captureFragmentCatalogue(FragmentRequest request, FragmentCompletion completion, int timeoutMs = 1000);
    void revalidateFragmentCatalogue(const FragmentCatalogue &catalogue, std::function<void(bool)> completion,
                                    int timeoutMs = 1000);
    // One fixed control check after a fully sealed fresh selection collection.
    void validateSelectionCatalogue(const FragmentCatalogue &catalogue, bool requireFocus, std::function<void(bool)> completion,
                                    int timeoutMs = 1000);
    // Native-only projection input from the catalogue's original frame chain.
    // Two equal reads bind mappings and viewports across asynchronous replies.
    // This supplies geometry, not target exposure or an action permit.
    void captureFragmentGeometry(const FragmentCatalogue &catalogue,
        std::function<void(std::optional<QJsonObject>)> completion, int timeoutMs = 1000);
    void projectFragmentCatalogue(const FragmentCatalogue &catalogue,
        std::function<void(std::optional<QJsonObject>)> completion, int timeoutMs = 1000,
        std::shared_ptr<FerricHintWorkTicket> work = {});
    // Synchronous native association only; token/origin/focus freshness still
    // requires the asynchronous revalidation above.
    bool documentAssociationIsCurrent(const DocumentReceipt &receipt) const { return matchesReceipt(receipt); }
    Q_PROPERTY(int leaseCount READ leaseCount NOTIFY leasesChanged)
    int leaseCount() const { return frames_.size(); }
    int pendingScripts() const;
signals:
    void leasesChanged();
    void scriptSettled();
private:
    void capturePageFocusAt(const QString &owner, int depth, const QString &script,
        const QDeadlineTimer &deadline, FocusCompletion completion);
    void verifyPageFocusPath(const DocumentReceipt &receipt, int index,
        const QDeadlineTimer &deadline, std::function<void(bool)> completion);
    struct Lease { QWebEngineFrame frame; QString epoch; QString parent; };
    using Completion = std::function<void(const QVariant &)>;
    struct Pending;
    struct Capture;
    struct AncestryCapture;
    void verifyFocusedLeafNative(const QString &owner, Completion completion, int timeoutMs = 1000);
    void validateLocalDocuments(const DocumentReceipt &world, std::optional<FocusedDocuments> focused,
                                const QString &batch, const QString &stamp, quint64 revision,
                                const QString &documentsJson, const std::optional<QString> &annotationJson,
                                const QJsonValue &scope, std::optional<EnteredDocument> entered,
                                std::optional<QString> originalPlan,
                                const QJsonValue &annotationWork,
                                std::function<void(bool)> completion,
                                int timeoutMs, std::optional<QSizeF> viewport = std::nullopt,
                                std::function<void(const QString &)> fragmentCapture = {},
                                QJsonValue ordinaryGeometry = QJsonValue(QJsonValue::Null),
                                const QString &ordinaryInput = QString(),
                                FerricHintGraphAdmission graphAdmission = {}, quint8 graphBase = 0,
                                FerricHintDocumentAdmission documentAdmission = {});
    void validateWorldObjectsImpl(const DocumentReceipt &world, std::optional<FocusedDocuments> focused,
                                const QString &batch, const QString &stamp, quint64 revision,
                                const QString &annotationJson, const QString &scopeJson,
                                std::function<void(bool)> completion, int timeoutMs,
                                std::optional<EnteredDocument> entered, std::optional<QString> originalPlan,
                                std::optional<AnnotationDebit> annotationDebit, std::optional<QSizeF> viewport,
                                std::function<void(const QString &)> fragmentCapture, const QString &ordinaryGeometry,
                                const QString &ordinaryInput = QString(),
                                FerricHintGraphAdmission graphAdmission = {}, quint8 graphBase = 0,
                                FerricHintDocumentAdmission documentAdmission = {});
    void captureFragmentCatalogueImpl(FragmentRequest request, FragmentCompletion completion,
                                     int timeoutMs, quint8 graphBase);
    struct FocusScanResult {
        QString error;
        std::optional<QWebEngineFrame> child;
    };
    void scanFocusedChild(const QString &parent, std::function<void(FocusScanResult)> completion,
                          int timeoutMs = 1000);
    bool matchesReceipt(const DocumentReceipt &receipt) const;
    void advanceCapture(const std::shared_ptr<Capture> &capture);
    void captureDocumentImpl(const QString &owner, ReceiptCompletion completion, int timeoutMs,
                             std::optional<DocumentWork> work, std::optional<DocumentReceipt> original);
    void advanceAncestryCapture(const std::shared_ptr<AncestryCapture> &capture);
    void validateAncestryAt(const DocumentReceipt &receipt, qsizetype index, const QDeadlineTimer &deadline,
                           std::function<void(bool)> completion);
    QString ancestryScript(const DocumentReceipt &receipt, qsizetype index, const QString &token, bool capture) const;
    void releaseAncestry();
    void cancelPending();
    void evaluate(QWebEngineFrame frame, const QString &script, Completion completion,
                  int timeoutMs = 1000, std::shared_ptr<FerricHintWorkTicket> workTicket = {});
    void evaluateGraph(QWebEngineFrame frame, const QString &script, Completion completion,
                       int timeoutMs, const FerricHintGraphAdmission &admission, quint8 pass);
    void evaluateDocument(QWebEngineFrame frame, const QString &script, Completion completion,
                          int timeoutMs, const std::optional<DocumentWork> &work, quint8 document, quint8 pass);
    void attachFrame(const QWebEngineFrame &frame, const QString &parent, Completion completion,
                     const QDeadlineTimer &deadline = QDeadlineTimer(1000, Qt::PreciseTimer),
                     bool allowRebind = true);
    void deliver(const QJSValue &callback, const QVariant &value);
    bool reachable(const QString &owner) const;
    void execute(const QString &owner, const QString &script, const QJSValue &callback, bool action);
    QString documentProperty_;
    QString ancestryIssuer_;
    QHash<QString, Lease> frames_;
    QList<QWebEngineFrame> inspected_;
    quint64 generation_ = 0;
    std::shared_ptr<FerricFrameDispatcher> dispatcher_;
    QList<std::weak_ptr<Pending>> pending_;
};
// Independent lease namespaces and lifetimes over the same native authority.
class FerricHintFrameBroker : public FerricFrameBroker {
    Q_OBJECT
    QML_NAMED_ELEMENT(FerricHintFrameBroker)
public:
    explicit FerricHintFrameBroker(QObject *parent = nullptr)
        : FerricFrameBroker(parent, QStringLiteral("__ferric_hintFrameDocument")) {}
};
class FerricSelectionFrameBroker : public FerricFrameBroker {
    Q_OBJECT
    QML_NAMED_ELEMENT(FerricSelectionFrameBroker)
public:
    explicit FerricSelectionFrameBroker(QObject *parent = nullptr)
        : FerricFrameBroker(parent, QStringLiteral("__ferric_selectionFrameDocument")) {}
};
#endif
