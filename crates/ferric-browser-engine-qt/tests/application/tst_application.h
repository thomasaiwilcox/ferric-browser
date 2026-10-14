#pragma once
#include <QObject>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <QTcpServer>
#include <QTemporaryDir>
#include <QVariant>
#include <memory>

// Production policy stays in the linked engine; this object only drives and
// observes tests.
class ApplicationTests final : public QObject {
    Q_OBJECT
    std::unique_ptr<QQmlApplicationEngine> engine;
    QQuickWindow *window = nullptr;
    QTemporaryDir directory;
    QTcpServer server;
    QByteArray fixtureHtml;
    QVariant scriptResult;
    bool scriptDone = false;
    QString pageReceiver = "testWindow.activeWebView()";
    QVariant evaluate(const QString &source);
    QString text(const QString &property);
    bool command(const QString &value);
    static QString quoted(const QString &value);
    QVariant page(const QString &source, int world = 0);
    void typeText(const QString &text);
    QString hintLabel(const QString &name);
    bool chooseCapability(const QString &name);
    void key(int value, Qt::KeyboardModifiers modifiers = Qt::NoModifier);

  public:
    Q_INVOKABLE void accept(const QVariant &value);
  private slots:
    void hintConfirmationFreezesPendingRefresh();
    void hintCollisionCyclesActualMarkers();
    void rapidHintRejectsConsumedReplay();
    void hintRefreshRetainsInput();
    void hintRefreshDoesNotAutoFollow();
    void initTestCase();
    void init();
    void cleanup();
    void startupAndConfigPublication();
    void reservedCommandRecovery();
    void realHintKeyboardActivation();
    void mixedReservedKeysAndUnmappedDelivery();
    void heldPageKeyRetainsOwnerAcrossPolicyChange();
    void nativePointerRequiresLocalEntryAndClicksOnce();
    void nativePointerControlFamilies_data();
    void nativePointerControlFamilies();
    void nativePointerFrameAndShadow_data();
    void nativePointerFrameAndShadow();
    void nativePopupActionIsOneTransport();
    void popupWithoutUserActivationIsDenied();
    void fullscreenApiOutcomes_data();
    void fullscreenApiOutcomes();
    void clipboardPermissionOutcomes_data();
    void clipboardPermissionOutcomes();
    void selectorReloadActivatesAtBoundary_data();
    void selectorReloadActivatesAtBoundary();
    void selectorReloadRejectsInvalidAndStaleCandidates();
    void selectorReloadTimeoutPreservesConfiguration();
    void selectorDiscoveryUsesProductionPolicy_data();
    void selectorDiscoveryUsesProductionPolicy();
    void startupSelectorValidation_data();
    void startupSelectorValidation();
    void staleSelectorCallbackCannotPublish();
    void unavailableSelectorDocumentTimesOut();
    void productionHintLifetimeCycles();
    void scopedGridNativeCoordinates_data();
    void scopedGridNativeCoordinates();
    void mixedPageKeysAndBrowserSequence_data();
    void mixedPageKeysAndBrowserSequence();
    void pointerConfigurationInvalidatesHeldConfirmation();
    void rendererLossCancelsHeldConfirmation();
    void pointerObservationIntervalExperiment();
    void pointerInvalidationBeforeRelease_data();
    void pointerInvalidationBeforeRelease();
    void targetSearchNeedsConfirmation();
    void promptAcceptCancelAndNestedRestoration();
    void stalePromptCannotRestoreAfterConfigChange();
    void passThroughDocumentBoundaries();
    void scopedGridResetAndInvalidation();
    void scopedHintAndSearchJourneys_data();
    void scopedHintAndSearchJourneys();
    void durablePersistenceFailurePreservesConfiguration();
    void selectorValidationRejectsWholeCandidate();
};
