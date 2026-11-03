#include "public_page_host.h"

#include "public_page_owner.h"
#include "public_profile_owner.h"
#include "request_interceptor.h"
#include "credential_boundary.h"
#include "../credential_https_fixture.h"

#include <QApplication>
#include <QClipboard>
#include <QDebug>
#include <QEventLoop>
#include <QFileDialog>
#include <QMetaProperty>
#include <QPointer>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickItem>
#include <QQuickWidget>
#include <QSignalSpy>
#include <QStringList>
#include <QTemporaryDir>
#include <QTest>
#include <QTimer>
#include <QDialog>
#include <QDialogButtonBox>
#include <QPushButton>
#include <QWebEngineCertificateError>
#include <QWebEngineFrame>
#include <QWebEngineSettings>
#include <QWebEngineScript>
#include <QWebEngineScriptCollection>
#include <QJsonDocument>
#include <QJsonObject>
#include <QTcpServer>
#include <QTcpSocket>
#include <QVariantMap>
#include <QWebEnginePage>
#include <QWebEngineProfile>
#include <QWebEngineView>

#include <functional>
#include <memory>
#include <utility>

extern "C" bool cxx_qt_init_crate_ferric_browser_engine_qt();

namespace {

QString componentErrors(const QQmlComponent &component)
{
    QStringList messages;
    for (const auto &error : component.errors()) {
        messages.append(error.toString());
    }
    return messages.join(QLatin1Char('\n'));
}

QObject *makeBrowserUi(QQmlEngine &engine)
{
    QQmlComponent component(&engine);
    component.setData("import io.github.ferricbrowser 1.0\nBrowserUi {}", QUrl());
    if (!component.isReady()) {
        qCritical().noquote() << "BrowserUi QML component failed:" << componentErrors(component);
        return nullptr;
    }
    QObject *browserUi = component.create();
    if (!browserUi) {
        qCritical().noquote() << "BrowserUi QML creation failed:" << componentErrors(component);
    }
    return browserUi;
}

bool configureBrowserUi(QObject *browserUi, const QString &storageBase)
{
    return QMetaObject::invokeMethod(
        browserUi,
        "configure_profile",
        Qt::DirectConnection,
        Q_ARG(bool, false),
        Q_ARG(bool, true),
        Q_ARG(QString, QStringLiteral("F02e2 disposable")),
        Q_ARG(QString, QStringLiteral("f02e2-disposable")),
        Q_ARG(QString, storageBase));
}

QString browserUiProfileId(QObject *browserUi);

bool waitForProfileBootstrap(QObject *browserUi, QString *diagnostics)
{
    QEventLoop loop;
    QTimer pollTimer;
    QTimer timeoutTimer;
    pollTimer.setInterval(25);
    timeoutTimer.setSingleShot(true);

    bool pollInvocationResult = false;
    int pollInvocationCount = 0;
    auto pollConnection = QObject::connect(&pollTimer, &QTimer::timeout, &loop, [&] {
        pollInvocationResult = QMetaObject::invokeMethod(
            browserUi, "poll_config", Qt::DirectConnection);
        ++pollInvocationCount;
        const QString profileId = browserUiProfileId(browserUi);
        const int tabCount = browserUi->property("tab_count").toInt();
        const bool pending = browserUi->property("profile_bootstrap_pending").toBool();
        if (pollInvocationResult && !profileId.isEmpty() && tabCount > 0 && !pending) {
            loop.quit();
        }
    });
    auto timeoutConnection = QObject::connect(
        &timeoutTimer, &QTimer::timeout, &loop, &QEventLoop::quit);

    pollTimer.start();
    timeoutTimer.start(30000);
    loop.exec();

    pollTimer.stop();
    timeoutTimer.stop();
    QObject::disconnect(pollConnection);
    QObject::disconnect(timeoutConnection);

    const QString profileId = browserUiProfileId(browserUi);
    const int tabCount = browserUi->property("tab_count").toInt();
    const bool pending = browserUi->property("profile_bootstrap_pending").toBool();
    const bool ready = pollInvocationResult && !profileId.isEmpty() && tabCount > 0 && !pending;
    if (!ready && diagnostics) {
        *diagnostics = QStringLiteral(
            "poll invocation result=%1, poll count=%2, profile ID='%3', tab_count=%4, "
            "profile_bootstrap_pending=%5, profile_startup_notice='%6', status_text='%7'")
            .arg(pollInvocationResult)
            .arg(pollInvocationCount)
            .arg(profileId)
            .arg(tabCount)
            .arg(pending)
            .arg(browserUi->property("profile_startup_notice").toString())
            .arg(browserUi->property("status_text").toString());
    }
    return ready;
}

QString browserUiProfileId(QObject *browserUi)
{
    QString profileId;
    QMetaObject::invokeMethod(
        browserUi,
        "page_host_profile_id",
        Qt::DirectConnection,
        Q_RETURN_ARG(QString, profileId));
    return profileId;
}

QString browserUiTabAt(QObject *browserUi, int index)
{
    QString tab;
    QMetaObject::invokeMethod(
        browserUi,
        "tab_id_for_index",
        Qt::DirectConnection,
        Q_RETURN_ARG(QString, tab),
        Q_ARG(int, index));
    return tab;
}

bool requestHostOperation(
    QObject *browserUi,
    const char *method,
    const QString &tab,
    const QString &profileId,
    const QString &hostToken,
    const QString &pageOwnerToken)
{
    bool accepted = false;
    const bool invoked = QMetaObject::invokeMethod(
        browserUi,
        method,
        Qt::DirectConnection,
        Q_RETURN_ARG(bool, accepted),
        Q_ARG(QString, tab),
        Q_ARG(QString, profileId),
        Q_ARG(QString, hostToken),
        Q_ARG(QString, pageOwnerToken));
    return invoked && accepted;
}

bool browserUiTargetCurrent(
    QObject *browserUi,
    const QString &tab,
    const QString &generation,
    const QString &document)
{
    bool current = false;
    QMetaObject::invokeMethod(
        browserUi,
        "request_target_current",
        Qt::DirectConnection,
        Q_RETURN_ARG(bool, current),
        Q_ARG(QString, tab),
        Q_ARG(QString, generation),
        Q_ARG(QString, document));
    return current;
}

bool notifyNavigationStarted(QObject *browserUi, int index, const QString &url)
{
    return QMetaObject::invokeMethod(
        browserUi,
        "navigation_started_for",
        Qt::DirectConnection,
        Q_ARG(int, index),
        Q_ARG(QString, url));
}

QVariant takeReparentRequest(QObject *browserUi)
{
    QVariant request;
    if (!QMetaObject::invokeMethod(
            browserUi,
            "take_engine_request",
            Qt::DirectConnection,
            Q_RETURN_ARG(QVariant, request))) {
        return {};
    }
    return request;
}

using JavaScriptResultCallback = std::function<void(const QVariant &)>;

QVariant waitForJavaScriptResult(
    const std::function<void(JavaScriptResultCallback)> &submit, int timeoutMs = 10000)
{
    struct ResultState {
        QVariant value;
        bool completed = false;
    };
    const auto state = std::make_shared<ResultState>();
    QEventLoop loop;
    const QPointer<QEventLoop> guardedLoop(&loop);
    QTimer timeout;
    timeout.setSingleShot(true);
    QObject::connect(&timeout, &QTimer::timeout, &loop, &QEventLoop::quit);
    submit([state, guardedLoop](const QVariant &result) {
        state->value = result;
        state->completed = true;
        if (guardedLoop) {
            guardedLoop->quit();
        }
    });
    if (!state->completed) {
        timeout.start(timeoutMs);
        loop.exec();
    }
    return state->completed ? state->value : QVariant();
}

QVariant runJavaScript(QWebEnginePage *page, const QString &script)
{
    return waitForJavaScriptResult([page, script](JavaScriptResultCallback callback) {
        page->runJavaScript(script, std::move(callback));
    });
}

QVariant runWorld(QWebEnginePage *page, const QString &script, quint32 world) {
    return waitForJavaScriptResult([page, script, world](JavaScriptResultCallback callback) {
        page->mainFrame().runJavaScript(script, world, std::move(callback));
    });
}

void acceptFixtureCertificate(QWebEnginePage *page, const QUrl &fixture) {
    QObject::connect(page, &QWebEnginePage::certificateError, page, [fixture](QWebEngineCertificateError error) {
        if (error.url().host() == fixture.host() && error.url().port() == fixture.port()) error.acceptCertificate();
    });
}

bool pageViewHasFocus(QWidget *view)
{
    QWidget *focus = QApplication::focusWidget();
    return view && focus && (focus == view || view->isAncestorOf(focus));
}

} // namespace

class PublicPageHostIntegrationTest final : public QObject {
    Q_OBJECT

private:
    void verifySamePageReparentsAcrossHosts()
    {
        JavaScriptResultCallback lateCompletion;
        const QVariant timedOut = waitForJavaScriptResult(
            [&lateCompletion](JavaScriptResultCallback callback) {
                lateCompletion = std::move(callback);
            },
            0);
        QVERIFY(!timedOut.isValid());
        QVERIFY(lateCompletion);
        QCOMPARE(waitForJavaScriptResult([&lateCompletion](JavaScriptResultCallback callback) {
                     lateCompletion(QVariant(QStringLiteral("late")));
                     QTimer::singleShot(0, [callback = std::move(callback)] {
                         callback(QVariant(42));
                     });
                 }),
                 QVariant(42));
        lateCompletion = {};
        QCOMPARE(waitForJavaScriptResult([](JavaScriptResultCallback callback) {
                     callback(QVariant(7));
                 }),
                 QVariant(7));

        QTemporaryDir storage;
        QVERIFY(storage.isValid());
        QQmlEngine engine;
        std::unique_ptr<QObject> browserUi(makeBrowserUi(engine));
        QVERIFY2(browserUi, "BrowserUi QML registration failed");
        QVERIFY(configureBrowserUi(browserUi.get(), storage.path()));

        QString primaryBootstrapDiagnostics;
        QVERIFY2(waitForProfileBootstrap(browserUi.get(), &primaryBootstrapDiagnostics),
                 qPrintable(primaryBootstrapDiagnostics));
        const QString profileId = browserUiProfileId(browserUi.get());
        const QString tab = browserUiTabAt(browserUi.get(), 0);
        QVERIFY(!tab.isEmpty());

        FerricBrowserRequestInterceptor interceptor;
        FerricPublicProfileOwner profileOwner(&interceptor);
        QVERIFY(profileOwner.isReady());
        QObject logicalPageLifetime;
        FerricPublicPageOwner *owner = profileOwner.createPage(&logicalPageLifetime);
        QVERIFY(owner);
        QVERIFY(!owner->isClosed());

        auto first = std::make_unique<FerricPublicPageHost>(
            browserUi.get(), profileOwner.profile(), owner, nullptr, tab);
        auto second = std::make_unique<FerricPublicPageHost>(
            browserUi.get(), profileOwner.profile());
        QVERIFY(first->isReady());
        QVERIFY(second->isReady());
        QCOMPARE(first->pageOwner(), owner);
        QVERIFY(!first->pageOwnerToken().isEmpty());
        QVERIFY(!first->hostToken().isEmpty());
        QCOMPARE(first->chromeView()->rootObject()->property("browserUi").value<QObject *>(),
                 browserUi.get());
        QCOMPARE(first->chromeView()->rootObject()->property("statusText").toString(),
                 browserUi->property("status_text").toString());
        const int statusIndex = browserUi->metaObject()->indexOfProperty("status_text");
        QVERIFY(statusIndex >= 0);
        const QMetaProperty statusProperty = browserUi->metaObject()->property(statusIndex);
        QSignalSpy statusChanged(browserUi.get(), statusProperty.notifySignal());
        QVERIFY(statusChanged.isValid());
        QVERIFY(statusProperty.write(browserUi.get(), QStringLiteral("Widget host active")));
        QCOMPARE(statusChanged.count(), 1);
        QCOMPARE(first->chromeView()->rootObject()->property("statusText").toString(),
                 QStringLiteral("Widget host active"));

        QWebEngineView *const originalView = first->pageView();
        QWebEnginePage *const originalPage = owner->page();
        QVERIFY(originalView);
        QVERIFY(originalPage);
        QCOMPARE(originalView->page(), originalPage);
        QCOMPARE(originalPage->profile(), profileOwner.profile());
        QCOMPARE(owner->profile(), profileOwner.profile());

        first->setGeometry(20, 30, 900, 640);
        first->show();
        first->activateWindow();
        first->pageView()->setFocus(Qt::OtherFocusReason);
        QVERIFY(first->pageView()->devicePixelRatioF() > 0.0);
        QTRY_VERIFY_WITH_TIMEOUT(pageViewHasFocus(first->pageView()), 3000);

        QSignalSpy loaded(originalPage, &QWebEnginePage::loadFinished);
        originalPage->setHtml(QStringLiteral(
            "<!doctype html><meta charset=utf-8><title>retained</title>"
            "<body>page-retained</body><script>"
            "window.__ferricDocumentNonce='e2-retained';"
            "history.pushState({}, '', '#retained');</script>"));
        if (loaded.isEmpty()) {
            QVERIFY(loaded.wait(10000));
        }
        QCOMPARE(runJavaScript(originalPage, QStringLiteral("window.__ferricDocumentNonce")),
                 QStringLiteral("e2-retained"));
        const QUrl originalUrl = originalPage->url();
        const quint64 documentGeneration = owner->documentGeneration();
        const quint64 pageGeneration = owner->pageGeneration();
        const quint64 ownerHostGeneration = owner->hostGeneration();
        const quint64 presentationGeneration = owner->presentationGeneration();
        const int originalTabCount = browserUi->property("tab_count").toInt();
        const int originalActiveIndex = browserUi->property("active_tab_index").toInt();
        const QString ownerToken = first->pageOwnerToken();

        // Direct transfer must capture current page focus even without a prior detach.
        second->setGeometry(first->geometry());
        second->show();
        second->activateWindow();
        second->chromeView()->setFocus(Qt::OtherFocusReason);
        QTRY_VERIFY_WITH_TIMEOUT(pageViewHasFocus(second->chromeView()), 3000);
        first->activateWindow();
        originalView->setFocus(Qt::OtherFocusReason);
        QTRY_VERIFY_WITH_TIMEOUT(pageViewHasFocus(originalView), 3000);
        QVERIFY(requestHostOperation(
            browserUi.get(), "attach_page_host", tab, profileId,
            second->hostToken(), ownerToken));
        const QVariant directAttach = takeReparentRequest(browserUi.get());
        QVERIFY(directAttach.isValid());
        QVERIFY(second->applyNativeRequest(directAttach));
        QCOMPARE(first->pageView(), nullptr);
        QCOMPARE(second->pageView(), originalView);
        QCOMPARE(originalView->page(), originalPage);
        second->activateWindow();
        QTRY_VERIFY_WITH_TIMEOUT(pageViewHasFocus(originalView), 3000);
        QVERIFY(requestHostOperation(
            browserUi.get(), "attach_page_host", tab, profileId,
            first->hostToken(), ownerToken));
        const QVariant directReturn = takeReparentRequest(browserUi.get());
        QVERIFY(directReturn.isValid());
        QVERIFY(first->applyNativeRequest(directReturn));
        QCOMPARE(second->pageView(), nullptr);
        QCOMPARE(first->pageView(), originalView);
        QCOMPARE(originalView->page(), originalPage);
        first->activateWindow();
        QTRY_VERIFY_WITH_TIMEOUT(pageViewHasFocus(originalView), 3000);

        // A previous focused transfer must not override a later choice of chrome focus.
        second->chromeView()->setFocus(Qt::OtherFocusReason);
        first->chromeView()->setFocus(Qt::OtherFocusReason);
        QTRY_VERIFY_WITH_TIMEOUT(pageViewHasFocus(first->chromeView()), 3000);
        QVERIFY(requestHostOperation(
            browserUi.get(), "attach_page_host", tab, profileId,
            second->hostToken(), ownerToken));
        const QVariant chromeFocusedAttach = takeReparentRequest(browserUi.get());
        QVERIFY(chromeFocusedAttach.isValid());
        QVERIFY(second->applyNativeRequest(chromeFocusedAttach));
        QCOMPARE(second->pageView(), originalView);
        second->activateWindow();
        QTRY_VERIFY_WITH_TIMEOUT(pageViewHasFocus(second->chromeView()), 3000);
        QVERIFY(!pageViewHasFocus(originalView));
        QVERIFY(requestHostOperation(
            browserUi.get(), "attach_page_host", tab, profileId,
            first->hostToken(), ownerToken));
        const QVariant chromeFocusedReturn = takeReparentRequest(browserUi.get());
        QVERIFY(chromeFocusedReturn.isValid());
        QVERIFY(first->applyNativeRequest(chromeFocusedReturn));
        QCOMPARE(first->pageView(), originalView);
        QCOMPARE(originalView->page(), originalPage);
        first->activateWindow();
        originalView->setFocus(Qt::OtherFocusReason);
        QTRY_VERIFY_WITH_TIMEOUT(pageViewHasFocus(originalView), 3000);

        QVERIFY(requestHostOperation(
            browserUi.get(),
            "detach_page_host",
            tab,
            profileId,
            first->hostToken(),
            ownerToken));
        const QVariant detachRequest = takeReparentRequest(browserUi.get());
        QVERIFY(detachRequest.isValid());
        const QVariantMap detach = detachRequest.toMap();
        QCOMPARE(detach.value(QStringLiteral("kind")).toString(), QStringLiteral("reparent-view"));
        QCOMPARE(detach.value(QStringLiteral("operation")).toString(), QStringLiteral("detach"));
        QCOMPARE(detach.value(QStringLiteral("tabId")).toString(), tab);
        QCOMPARE(detach.value(QStringLiteral("profileId")).toString(), profileId);
        QCOMPARE(detach.value(QStringLiteral("hostToken")).toString(), first->hostToken());
        QCOMPARE(detach.value(QStringLiteral("pageOwnerToken")).toString(), ownerToken);
        QVERIFY(!detach.value(QStringLiteral("generation")).toString().isEmpty());
        QVERIFY(!detach.value(QStringLiteral("documentId")).toString().isEmpty());
        QVERIFY(!detach.value(QStringLiteral("windowId")).toString().isEmpty());
        QVERIFY(browserUiTargetCurrent(
            browserUi.get(),
            tab,
            detach.value(QStringLiteral("generation")).toString(),
            detach.value(QStringLiteral("documentId")).toString()));
        QVERIFY(first->applyNativeRequest(detachRequest));
        QVERIFY(first->pageView()->isHidden());
        QCOMPARE(first->pageView(), originalView);
        QCOMPARE(owner->page(), originalPage);
        QVERIFY(!owner->isClosed());

        second->setGeometry(first->geometry());
        second->show();
        second->activateWindow();
        QVERIFY(requestHostOperation(
            browserUi.get(),
            "attach_page_host",
            tab,
            profileId,
            second->hostToken(),
            ownerToken));
        const QVariant attachRequest = takeReparentRequest(browserUi.get());
        QVERIFY(attachRequest.isValid());
        const QVariantMap attach = attachRequest.toMap();
        QCOMPARE(attach.value(QStringLiteral("kind")).toString(), QStringLiteral("reparent-view"));
        QCOMPARE(attach.value(QStringLiteral("operation")).toString(), QStringLiteral("attach"));
        QCOMPARE(attach.value(QStringLiteral("tabId")).toString(), tab);
        QCOMPARE(attach.value(QStringLiteral("profileId")).toString(), profileId);
        QCOMPARE(attach.value(QStringLiteral("hostToken")).toString(), second->hostToken());
        QCOMPARE(attach.value(QStringLiteral("pageOwnerToken")).toString(), ownerToken);
        QVERIFY(second->applyNativeRequest(attachRequest));

        QCOMPARE(first->pageView(), nullptr);
        QCOMPARE(second->pageView(), originalView);
        QCOMPARE(second->pageView()->page(), originalPage);
        QCOMPARE(owner->page(), originalPage);
        QCOMPARE(owner->profile(), profileOwner.profile());
        QCOMPARE(owner->documentGeneration(), documentGeneration);
        QCOMPARE(owner->pageGeneration(), pageGeneration);
        QCOMPARE(owner->hostGeneration(), ownerHostGeneration);
        QVERIFY(owner->presentationGeneration() > presentationGeneration);
        QCOMPARE(originalPage->url(), originalUrl);
        QCOMPARE(runJavaScript(originalPage, QStringLiteral("window.__ferricDocumentNonce")),
                 QStringLiteral("e2-retained"));
        QCOMPARE(runJavaScript(originalPage, QStringLiteral("history.length")), 2);
        QCOMPARE(browserUi->property("tab_count").toInt(), originalTabCount);
        QCOMPARE(browserUi->property("active_tab_index").toInt(), originalActiveIndex);
        QCOMPARE(browserUiTabAt(browserUi.get(), 0), tab);
        QCOMPARE(attach.value(QStringLiteral("windowId")).toString(),
                 detach.value(QStringLiteral("windowId")).toString());
        QCOMPARE(second->geometry(), first->geometry());
        QCOMPARE(second->pageView()->devicePixelRatioF(), first->chromeView()->devicePixelRatioF());
        QCOMPARE(second->nativeDialogParent(), static_cast<QWidget *>(originalView));
        QCOMPARE(originalView->parentWidget(), static_cast<QWidget *>(second.get()));
        QFileDialog dialog(second->nativeDialogParent());
        QCOMPARE(dialog.parentWidget(), originalView);
        QTRY_VERIFY_WITH_TIMEOUT(pageViewHasFocus(originalView), 3000);

        QTemporaryDir foreignStorage;
        QVERIFY(foreignStorage.isValid());
        std::unique_ptr<QObject> foreignUi(makeBrowserUi(engine));
        QVERIFY(foreignUi);
        QVERIFY(configureBrowserUi(foreignUi.get(), foreignStorage.path()));
        QString foreignBootstrapDiagnostics;
        QVERIFY2(waitForProfileBootstrap(foreignUi.get(), &foreignBootstrapDiagnostics),
                 qPrintable(foreignBootstrapDiagnostics));
        const QString foreignProfileId = browserUiProfileId(foreignUi.get());
        QVERIFY(foreignProfileId != profileId);

        FerricBrowserRequestInterceptor foreignInterceptor;
        FerricPublicProfileOwner foreignProfile(&foreignInterceptor);
        QVERIFY(foreignProfile.isReady());
        auto foreignHost = std::make_unique<FerricPublicPageHost>(
            foreignUi.get(), foreignProfile.profile());
        QVERIFY(foreignHost->isReady());
        QVERIFY(!requestHostOperation(
            browserUi.get(),
            "attach_page_host",
            tab,
            foreignProfileId,
            second->hostToken(),
            ownerToken));
        QVERIFY(!takeReparentRequest(browserUi.get()).isValid());

        QVERIFY(!requestHostOperation(
            browserUi.get(),
            "attach_page_host",
            tab,
            profileId,
            foreignHost->hostToken(),
            ownerToken));
        QVERIFY(!takeReparentRequest(browserUi.get()).isValid());

        const QString staleTab = QStringLiteral("00000000-0000-4000-8000-000000000001");
        QVERIFY(!requestHostOperation(
            browserUi.get(),
            "attach_page_host",
            staleTab,
            profileId,
            second->hostToken(),
            ownerToken));
        QVERIFY(!takeReparentRequest(browserUi.get()).isValid());

        QVERIFY(!takeReparentRequest(browserUi.get()).isValid());
        int secondTabIndex = -1;
        QVERIFY(QMetaObject::invokeMethod(
            browserUi.get(),
            "new_tab",
            Qt::DirectConnection,
            Q_RETURN_ARG(int, secondTabIndex)));
        QVERIFY(secondTabIndex > 0);
        const QString secondTab = browserUiTabAt(browserUi.get(), secondTabIndex);
        QVERIFY(!secondTab.isEmpty());
        // Drain only the bounded native setup effects generated by new_tab().
        bool newTabQueueDrained = false;
        for (int drained = 0; drained < 16; ++drained) {
            const QVariant setupRequest = takeReparentRequest(browserUi.get());
            if (!setupRequest.isValid()) {
                newTabQueueDrained = true;
                break;
            }
            QVERIFY(!setupRequest.toMap().isEmpty());
        }
        QVERIFY2(newTabQueueDrained, "new_tab setup exceeded the bounded request drain");
        QVERIFY(!requestHostOperation(
            browserUi.get(),
            "attach_page_host",
            secondTab,
            profileId,
            second->hostToken(),
            ownerToken));
        QVERIFY(!takeReparentRequest(browserUi.get()).isValid());

        auto retiredHost = std::make_unique<FerricPublicPageHost>(
            browserUi.get(), profileOwner.profile());
        const QString retiredHostToken = retiredHost->hostToken();
        retiredHost.reset();
        QVERIFY(!requestHostOperation(
            browserUi.get(),
            "attach_page_host",
            tab,
            profileId,
            retiredHostToken,
            ownerToken));
        QVERIFY(!takeReparentRequest(browserUi.get()).isValid());

        auto queuedRetiringHost = std::make_unique<FerricPublicPageHost>(
            browserUi.get(), profileOwner.profile());
        const QString queuedRetiringHostToken = queuedRetiringHost->hostToken();
        QVERIFY(requestHostOperation(
            browserUi.get(),
            "attach_page_host",
            tab,
            profileId,
            queuedRetiringHostToken,
            ownerToken));
        queuedRetiringHost.reset();
        QVERIFY(!takeReparentRequest(browserUi.get()).isValid());
        QCOMPARE(second->pageView(), originalView);

        bool selected = false;
        QVERIFY(QMetaObject::invokeMethod(
            browserUi.get(), "select_tab", Qt::DirectConnection,
            Q_RETURN_ARG(bool, selected), Q_ARG(int, 0)));
        QVERIFY(selected);
        QCOMPARE(browserUi->property("active_tab_index").toInt(), 0);
        QCOMPARE(browserUiTabAt(browserUi.get(), 0), tab);
        QVERIFY(!takeReparentRequest(browserUi.get()).isValid());

        QVERIFY(requestHostOperation(
            browserUi.get(),
            "attach_page_host",
            tab,
            profileId,
            second->hostToken(),
            ownerToken));
        const QVariant staleApplyRequest = takeReparentRequest(browserUi.get());
        QVERIFY(staleApplyRequest.isValid());
        const QVariantMap staleProjection = staleApplyRequest.toMap();
        QCOMPARE(staleProjection.value(QStringLiteral("kind")).toString(),
                 QStringLiteral("reparent-view"));
        const QString staleGeneration = staleProjection.value(QStringLiteral("generation")).toString();
        const QString staleDocument = staleProjection.value(QStringLiteral("documentId")).toString();
        QVERIFY(browserUiTargetCurrent(
            browserUi.get(), tab, staleGeneration, staleDocument));
        QWebEngineView *const viewBeforeStaleApply = second->pageView();
        QWebEnginePage *const pageBeforeStaleApply = originalPage;
        // A native load-start notification preserves the current target. Drive
        // the public navigation command that creates the replacement target.
        const QString nextUrl = QStringLiteral("https://example.test/next");
        QVERIFY(QMetaObject::invokeMethod(
            browserUi.get(), "navigate", Qt::DirectConnection,
            Q_ARG(QString, nextUrl)));
        const QVariant navigationRequest = takeReparentRequest(browserUi.get());
        QVERIFY(navigationRequest.isValid());
        const QVariantMap navigation = navigationRequest.toMap();
        QCOMPARE(navigation.value(QStringLiteral("kind")).toString(), QStringLiteral("navigate"));
        QCOMPARE(navigation.value(QStringLiteral("tabId")).toString(), tab);
        QCOMPARE(navigation.value(QStringLiteral("url")).toString(), nextUrl);
        const QString nextGeneration = navigation.value(QStringLiteral("generation")).toString();
        const QString nextDocument = navigation.value(QStringLiteral("documentId")).toString();
        QVERIFY(!nextGeneration.isEmpty());
        QVERIFY(!nextDocument.isEmpty());
        QVERIFY(nextGeneration != staleGeneration);
        QVERIFY(nextDocument != staleDocument);
        QVERIFY(browserUiTargetCurrent(browserUi.get(), tab, nextGeneration, nextDocument));
        QVERIFY(notifyNavigationStarted(
            browserUi.get(), 0, nextUrl));
        QVERIFY(browserUiTargetCurrent(browserUi.get(), tab, nextGeneration, nextDocument));
        QVERIFY(!browserUiTargetCurrent(browserUi.get(), tab, staleGeneration, staleDocument));
        QVERIFY(!second->applyNativeRequest(staleApplyRequest));
        QCOMPARE(second->pageView(), viewBeforeStaleApply);
        QCOMPARE(second->pageView()->page(), pageBeforeStaleApply);
        QCOMPARE(second->pageOwner(), owner);
        QVERIFY(!takeReparentRequest(browserUi.get()).isValid());
        QCOMPARE(browserUiTabAt(browserUi.get(), 0), tab);

        // Keep owner retirement independent of the navigation-invalidated
        // binding above, so rejection cannot pass merely on a stale target.
        QObject retiringPageLifetime;
        FerricPublicPageOwner *retiringOwner = profileOwner.createPage(&retiringPageLifetime);
        QVERIFY(retiringOwner);
        QVERIFY(!retiringOwner->isClosed());
        auto retiringHost = std::make_unique<FerricPublicPageHost>(
            browserUi.get(), profileOwner.profile(), retiringOwner, nullptr, secondTab);
        QVERIFY(retiringHost->isReady());
        QCOMPARE(retiringHost->pageOwner(), retiringOwner);
        QVERIFY(requestHostOperation(
            browserUi.get(),
            "detach_page_host",
            secondTab,
            profileId,
            retiringHost->hostToken(),
            retiringHost->pageOwnerToken()));
        const QVariant staleNativeRequest = takeReparentRequest(browserUi.get());
        QVERIFY(staleNativeRequest.isValid());
        const QVariantMap retiringRequest = staleNativeRequest.toMap();
        QCOMPARE(retiringRequest.value(QStringLiteral("kind")).toString(),
                 QStringLiteral("reparent-view"));
        const QString retiringGeneration = retiringRequest.value(QStringLiteral("generation")).toString();
        const QString retiringDocument = retiringRequest.value(QStringLiteral("documentId")).toString();
        QVERIFY(browserUiTargetCurrent(browserUi.get(), secondTab, retiringGeneration, retiringDocument));
        QWebEngineView *const viewBeforeRetirement = retiringHost->pageView();
        retiringOwner->close();
        QVERIFY(retiringOwner->isClosed());
        QVERIFY(browserUiTargetCurrent(browserUi.get(), secondTab, retiringGeneration, retiringDocument));
        QVERIFY(!retiringHost->applyNativeRequest(staleNativeRequest));
        QCOMPARE(retiringHost->pageView(), viewBeforeRetirement);
        QCOMPARE(retiringHost->pageOwner(), retiringOwner);
        QVERIFY(!takeReparentRequest(browserUi.get()).isValid());
    }

private slots:
    void productionCredentialBoundaryAndChoice()
    {
        FixtureServer server;
        server.setDocuments(R"HTML(<!doctype html><title>Credential receiver</title>
            <script>window.responses=[];addEventListener('message',e=>{
              if(e.data && e.data.type==='ferric.credentials.response.v1') responses.push(e.data);});</script>
            <iframe src='/frame'></iframe>)HTML", "<!doctype html><title>Hostile child</title>");
        QVERIFY(server.start());
        QTemporaryDir storage;
        QQmlEngine engine;
        std::unique_ptr<QObject> ui(makeBrowserUi(engine));
        QVERIFY(ui && configureBrowserUi(ui.get(), storage.path()));
        QString diagnostics;
        QVERIFY2(waitForProfileBootstrap(ui.get(), &diagnostics), qPrintable(diagnostics));
        FerricBrowserRequestInterceptor interceptor;
        FerricPublicProfileOwner profile(&interceptor);
        QObject lifetime;
        auto *owner = profile.createPage(&lifetime);
        FerricPublicPageHost host(ui.get(), profile.profile(), owner, nullptr, browserUiTabAt(ui.get(), 0));
        auto *boundary = host.prepareCredentialBoundary();
        QVERIFY(boundary && boundary->isReady());
        acceptFixtureCertificate(owner->page(), server.pageUrl());
        QSignalSpy captured(boundary, &FerricCredentialBoundary::requestCaptured);
        QSignalSpy chosen(boundary, &FerricCredentialBoundary::providerChosen);
        QSignalSpy cancelRequested(boundary, &FerricCredentialBoundary::cancelRequested);
        QSignalSpy loaded(owner->page(), &QWebEnginePage::loadFinished);
        host.show(); host.activateWindow(); host.pageView()->setFocus();
        const QString url = server.pageUrl().toString();
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "navigate", Qt::DirectConnection, Q_ARG(QString, url)));
        QVERIFY(QMetaObject::invokeMethod(host.chromeView()->rootObject(), "dispatchPendingEngineRequest"));
        QTRY_VERIFY_WITH_TIMEOUT(!loaded.isEmpty(), 15000);
        QVERIFY(loaded.last().at(0).toBool());
        QTRY_VERIFY_WITH_TIMEOUT(runWorld(owner->page(), "typeof __ferricCredentialMediator === 'object'", 42).toBool(), 5000);
        QCOMPARE(runWorld(owner->page(), "typeof __ferricCredentialMediator", 0).toString(), QStringLiteral("undefined"));
        QCOMPARE(runWorld(owner->page(), "typeof __ferricCredentialMediator", 1).toString(), QStringLiteral("undefined"));
        QVERIFY(!runWorld(owner->page(), "typeof qt !== 'undefined' && !!qt.webChannelTransport", 0).toBool());
        QVERIFY(!runWorld(owner->page(), "typeof qt !== 'undefined' && !!qt.webChannelTransport", 1).toBool());
        const QString forged = "({type:'ferric.credentials.request.v1',id:'forged',op:'get',options:{challenge:'AA'}})";
        runWorld(owner->page(), "dispatchEvent(new MessageEvent('message',{source:window,origin:location.origin,data:" + forged + "}))", 0);
        QTRY_VERIFY_WITH_TIMEOUT(!owner->page()->mainFrame().children().isEmpty(), 5000);
        auto child = owner->page()->mainFrame().children().first();
        waitForJavaScriptResult([&](JavaScriptResultCallback callback) {
            child.runJavaScript("parent.postMessage(" + forged + ", '*')", 0, std::move(callback));
        });
        QTest::qWait(100);
        QCOMPARE(captured.count(), 0);
        runWorld(owner->page(), "window.qt={webChannelTransport:{send:()=>{}}};postMessage({type:'ferric.credentials.request.v1',id:'phone',op:'get',origin:'https://evil.invalid',options:{challenge:'AA'}},location.origin)", 0);
        QTRY_COMPARE_WITH_TIMEOUT(captured.count(), 1, 5000);
        QVERIFY(boundary->request());
        QCOMPARE(boundary->request()->origin, QStringLiteral("https://localhost:%1").arg(server.pageUrl().port()));
        QCOMPARE(boundary->request()->id, QStringLiteral("phone"));
        QVERIFY(!boundary->complete("{\"skippedChoice\":true}"));
        QCOMPARE(chosen.count(), 0);
        QTRY_VERIFY_WITH_TIMEOUT(host.isActiveWindow(), 3000);
        QVERIFY(boundary->showProviderChoice(true));
        auto *phone = host.findChild<QPushButton *>(QStringLiteral("credentialPhoneProvider"));
        QVERIFY(phone && phone->isEnabled());
        QTRY_VERIFY_WITH_TIMEOUT(phone->window()->isActiveWindow(), 3000);
        QTest::mouseClick(phone, Qt::LeftButton);
        QCOMPARE(chosen.count(), 1);
        QVERIFY(chosen.first().first().toBool());
        QVERIFY(boundary->complete("{\"ok\":true}"));
        QVERIFY(!boundary->complete("{\"ok\":true}"));
        QTRY_VERIFY_WITH_TIMEOUT(!boundary->request(), 5000);
        QTRY_COMPARE_WITH_TIMEOUT(runWorld(owner->page(), "responses.length", 0).toInt(), 1, 5000);
        QVERIFY(runWorld(owner->page(), "responses[0].response.ok", 0).toBool());

        runWorld(owner->page(), "postMessage({type:'ferric.credentials.request.v1',id:'native',op:'create',options:{challenge:'AA'}},location.origin)", 0);
        QTRY_COMPARE_WITH_TIMEOUT(captured.count(), 2, 5000);
        host.activateWindow();
        QTRY_VERIFY_WITH_TIMEOUT(host.isActiveWindow(), 3000);
        QVERIFY(boundary->showProviderChoice(false));
        auto *native = host.findChild<QPushButton *>(QStringLiteral("credentialNativeProvider"));
        QVERIFY(native);
        QVERIFY(!host.findChild<QPushButton *>(QStringLiteral("credentialPhoneProvider"))->isEnabled());
        QTRY_VERIFY_WITH_TIMEOUT(native->window()->isActiveWindow(), 3000);
        QTest::mouseClick(native, Qt::LeftButton);
        QCOMPARE(chosen.count(), 2);
        QVERIFY(!chosen.last().first().toBool());
        QVERIFY(boundary->complete("{\"native\":true}"));
        QTRY_VERIFY_WITH_TIMEOUT(!boundary->request(), 5000);
        QTRY_COMPARE_WITH_TIMEOUT(runWorld(owner->page(), "responses.length", 0).toInt(), 2, 5000);

        runWorld(owner->page(), "postMessage({type:'ferric.credentials.request.v1',id:'abort',op:'get',options:{challenge:'AA'}},location.origin);postMessage({type:'ferric.credentials.cancel.v1',id:'abort'},location.origin)", 0);
        QTRY_COMPARE_WITH_TIMEOUT(runWorld(owner->page(), "responses.length", 0).toInt(), 3, 5000);
        QCOMPARE(runWorld(owner->page(), "responses[2].response.error", 0).toString(), QStringLiteral("AbortError"));
        QCOMPARE(captured.count(), 2);
        QCOMPARE(cancelRequested.count(), 1);
    }

    void productionCredentialBoundaryInvalidation()
    {
        FixtureServer server;
        server.setDocuments("<!doctype html><title>Credential lifetime</title>", "");
        QVERIFY(server.start());
        QTemporaryDir storage;
        QQmlEngine engine;
        std::unique_ptr<QObject> ui(makeBrowserUi(engine));
        QVERIFY(ui && configureBrowserUi(ui.get(), storage.path()));
        QString diagnostics;
        QVERIFY2(waitForProfileBootstrap(ui.get(), &diagnostics), qPrintable(diagnostics));
        FerricBrowserRequestInterceptor interceptor;
        FerricPublicProfileOwner profile(&interceptor);
        QObject lifetime;
        auto *owner = profile.createPage(&lifetime);
        const auto tab = browserUiTabAt(ui.get(), 0);
        const auto profileId = browserUiProfileId(ui.get());
        FerricPublicPageHost host(ui.get(), profile.profile(), owner, nullptr, tab);
        FerricPublicPageHost destination(ui.get(), profile.profile());
        auto *boundary = host.prepareCredentialBoundary();
        QVERIFY(boundary && boundary->isReady());
        acceptFixtureCertificate(owner->page(), server.pageUrl());
        QSignalSpy captured(boundary, &FerricCredentialBoundary::requestCaptured);
        QSignalSpy cancelled(boundary, &FerricCredentialBoundary::cancelled);
        QSignalSpy loaded(owner->page(), &QWebEnginePage::loadFinished);
        const QString url = server.pageUrl().toString();
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "navigate", Qt::DirectConnection, Q_ARG(QString, url)));
        QVERIFY(QMetaObject::invokeMethod(host.chromeView()->rootObject(), "dispatchPendingEngineRequest"));
        QTRY_VERIFY_WITH_TIMEOUT(!loaded.isEmpty(), 15000);
        QVERIFY(loaded.last().at(0).toBool());
        runWorld(owner->page(), "postMessage({type:'ferric.credentials.request.v1',id:'reload',op:'get',options:{challenge:'AA'}},location.origin)", 0);
        QTRY_COMPARE_WITH_TIMEOUT(captured.count(), 1, 5000);
        const auto previous = *boundary->request();
        loaded.clear();
        owner->page()->triggerAction(QWebEnginePage::Reload);
        QTRY_VERIFY_WITH_TIMEOUT(!loaded.isEmpty(), 15000);
        QVERIFY(loaded.last().at(0).toBool());
        QVERIFY(!boundary->request());
        QVERIFY(!boundary->complete("{\"late\":true}"));
        QCOMPARE(cancelled.count(), 1);
        runWorld(owner->page(), "postMessage({type:'ferric.credentials.request.v1',id:'transfer',op:'get',options:{challenge:'AA'}},location.origin)", 0);
        QTRY_COMPARE_WITH_TIMEOUT(captured.count(), 2, 5000);
        QVERIFY(boundary->request()->documentNonce != previous.documentNonce);
        const auto beforeTransfer = boundary->request()->owner;
        QVERIFY(requestHostOperation(ui.get(), "detach_page_host", tab, profileId, host.hostToken(), host.pageOwnerToken()));
        const auto detach = takeReparentRequest(ui.get());
        const auto ownerToken = host.pageOwnerToken();
        QVERIFY(host.applyNativeRequest(detach));
        QVERIFY(!boundary->request());
        QVERIFY(!owner->isCurrent(beforeTransfer));
        QCOMPARE(cancelled.count(), 2);
        QVERIFY(requestHostOperation(ui.get(), "attach_page_host", tab, profileId, destination.hostToken(), ownerToken));
        QVERIFY(destination.applyNativeRequest(takeReparentRequest(ui.get())));
        QCOMPARE(owner->findChild<FerricCredentialBoundary *>(), boundary);
        QTest::qWait(100);
        runWorld(owner->page(), "postMessage({type:'ferric.credentials.request.v1',id:'survivor',op:'get',options:{challenge:'AA'}},location.origin)", 0);
        QTRY_COMPARE_WITH_TIMEOUT(captured.count(), 3, 5000);
        QVERIFY(boundary->request());
        // Advance the Rust target without dispatching a Qt load. The prior
        // credential cannot acquire the newly selected document's authority.
        const QString replacementUrl = url + QStringLiteral("?next");
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "navigate", Qt::DirectConnection, Q_ARG(QString, replacementUrl)));
        QVERIFY(!boundary->request());
        QVERIFY(!boundary->complete("{\"late\":true}"));
        owner->close();
        QVERIFY(!boundary->request());
        QVERIFY(!boundary->complete("{\"late\":true}"));
        QCOMPARE(cancelled.count(), 3);
    }

    void credentialShimSemantics()
    {
        FixtureServer server;
        server.setDocuments("<!doctype html><title>Credential shim semantics</title>", "");
        QVERIFY(server.start());
        QTemporaryDir storage;
        QQmlEngine engine;
        std::unique_ptr<QObject> ui(makeBrowserUi(engine));
        QVERIFY(ui && configureBrowserUi(ui.get(), storage.path()));
        QString diagnostics;
        QVERIFY2(waitForProfileBootstrap(ui.get(), &diagnostics), qPrintable(diagnostics));
        FerricBrowserRequestInterceptor interceptor;
        FerricPublicProfileOwner profile(&interceptor);
        QObject lifetime;
        auto *owner = profile.createPage(&lifetime);
        FerricPublicPageHost host(ui.get(), profile.profile(), owner, nullptr, browserUiTabAt(ui.get(), 0));
        auto *boundary = host.prepareCredentialBoundary();
        QVERIFY(boundary && boundary->isReady());
        // This Widgets fixture installs the same shim independently; production
        // Quick attachment installs it through the opted-in native integration.
        QFile source(QStringLiteral(FERRIC_TEST_CREDENTIAL_SHIM));
        QVERIFY(source.open(QIODevice::ReadOnly));
        QWebEngineScript shim;
        shim.setName("fixture-credential-shim");
        shim.setWorldId(QWebEngineScript::MainWorld);
        shim.setInjectionPoint(QWebEngineScript::DocumentCreation);
        shim.setRunsOnSubFrames(false);
        shim.setSourceCode(R"JS(
            window.originalPlatformCapability = PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable;
            window.nativeCredentialCalls = 0;
            for (const operation of ['create', 'get']) {
                const original = CredentialsContainer.prototype[operation];
                CredentialsContainer.prototype[operation] = function (...args) {
                    window.nativeCredentialCalls++;
                    return Reflect.apply(original, this, args);
                };
            }
        )JS"
            + QString::fromUtf8(source.readAll()));
        owner->page()->scripts().insert(shim);
        acceptFixtureCertificate(owner->page(), server.pageUrl());
        QSignalSpy captured(boundary, &FerricCredentialBoundary::requestCaptured);
        QSignalSpy delivered(boundary, &FerricCredentialBoundary::responseCompleted);
        QSignalSpy loaded(owner->page(), &QWebEnginePage::loadFinished);
        host.show(); host.activateWindow(); host.pageView()->setFocus();
        const QString url = server.pageUrl().toString();
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "navigate", Qt::DirectConnection, Q_ARG(QString, url)));
        QVERIFY(QMetaObject::invokeMethod(host.chromeView()->rootObject(), "dispatchPendingEngineRequest"));
        QTRY_VERIFY_WITH_TIMEOUT(!loaded.isEmpty(), 15000);
        QVERIFY(loaded.last().at(0).toBool());
        QVERIFY(runWorld(owner->page(), "typeof CredentialsContainer.prototype.__ferricCredentialShimV1.disable === 'function'", 0).toBool());
        QVERIFY(runWorld(owner->page(), "PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable === originalPlatformCapability", 0).toBool());
        const int capturedBeforeBlob = captured.count();
        runWorld(owner->page(), R"JS(
            window.blobNativeCallsBefore = nativeCredentialCalls;
            window.blobDelegationsFinished = 0;
            const blobAbort = new AbortController(); blobAbort.abort('native blob fixture');
            navigator.credentials.create({publicKey:{rp:{name:'Fixture'},
                user:{id:new Uint8Array([1]),name:'fixture',displayName:'Fixture'},
                challenge:new Uint8Array([1]),pubKeyCredParams:[{type:'public-key',alg:-7}],
                extensions:{credBlob:new Uint8Array([1])}},signal:blobAbort.signal})
                .catch(() => window.blobDelegationsFinished++);
            navigator.credentials.get({publicKey:{challenge:new Uint8Array([1]),
                extensions:{getCredBlob:true}},signal:blobAbort.signal})
                .catch(() => window.blobDelegationsFinished++);
            navigator.credentials.create({publicKey:{rp:{name:'Fixture'},
                user:{id:new Uint8Array([1]),name:'fixture',displayName:'Fixture'},
                challenge:new Uint8Array([1]),pubKeyCredParams:[{type:'public-key',alg:-7}],
                extensions:{appidExclude:'https://legacy.example/appid'}},signal:blobAbort.signal})
                .catch(() => window.blobDelegationsFinished++);
        )JS", 0);
        QTRY_VERIFY(runWorld(owner->page(), "blobDelegationsFinished === 3", 0).toBool());
        QVERIFY(runWorld(owner->page(), "nativeCredentialCalls - blobNativeCallsBefore === 3", 0).toBool());
        QCOMPARE(captured.count(), capturedBeforeBlob);
        runWorld(owner->page(), R"JS(
            window.finishedCredential = null; window.credentialFailure = null;
            const challengeStorage = new Uint8Array([9,1,2,3,8]);
            const userStorage = new Uint8Array([9,4,5,8]);
            navigator.credentials.create({publicKey:{rp:{name:'Fixture'},
                user:{name:'fixture',displayName:'Fixture',id:new DataView(userStorage.buffer,1,2)},
                challenge:challengeStorage.subarray(1,4),pubKeyCredParams:[{type:'public-key',alg:-7}],
                extensions:{prf:{eval:{first:new Uint8Array([6,7])}}}}})
                .then(value => finishedCredential=value, error => credentialFailure=error.name);
            challengeStorage.fill(0); userStorage.fill(0);
        )JS", 0);
        QTRY_COMPARE(captured.count(), 1);
        QVERIFY(boundary->request());
        const auto options = QJsonDocument::fromJson(boundary->request()->options).object();
        QCOMPARE(options.value("challenge").toString(), QStringLiteral("AQID"));
        QCOMPARE(options.value("user").toObject().value("id").toString(), QStringLiteral("BAU"));
        QCOMPARE(options.value("extensions").toObject().value("prf").toObject().value("eval").toObject().value("first").toString(), QStringLiteral("Bgc"));
        QTRY_VERIFY(host.isActiveWindow());
        QVERIFY(boundary->showProviderChoice(true));
        auto *dialog = host.findChild<QDialog *>("ferricCredentialProviderChoice");
        QVERIFY(dialog); QTRY_VERIFY(dialog->isActiveWindow());
        auto *phone = dialog->findChild<QPushButton *>("credentialPhoneProvider");
        QVERIFY(phone); QTest::mouseClick(phone, Qt::LeftButton);
        // Synthetic bytes exercise IDL reconstruction only. They are not a
        // valid attestation and no authenticator, helper or RP is involved.
        const QByteArray result = R"JSON({"credential":{"id":"AQID","rawId":"AQID","type":"public-key","authenticatorAttachment":"cross-platform","response":{"clientDataJSON":"e30","authenticatorData":"AA","attestationObject":"AA","publicKey":"AQI","publicKeyAlgorithm":-7,"transports":["hybrid"]},"clientExtensionResults":{"prf":{"results":{"first":"AQI"}}}}})JSON";
        QVERIFY(boundary->complete(result));
        QVERIFY(!boundary->complete(result));
        QTRY_COMPARE(delivered.count(), 1);
        QVERIFY(delivered.last().at(0).toBool());
        QTRY_VERIFY(runWorld(owner->page(), "finishedCredential instanceof PublicKeyCredential", 0).toBool());
        QVERIFY(runWorld(owner->page(), "finishedCredential.response instanceof AuthenticatorAttestationResponse", 0).toBool());
        QCOMPARE(runWorld(owner->page(), "Array.from(new Uint8Array(finishedCredential.rawId)).join(',')", 0).toString(), QStringLiteral("1,2,3"));
        QCOMPARE(runWorld(owner->page(), "finishedCredential.response.getPublicKeyAlgorithm()", 0).toInt(), -7);
        QCOMPARE(runWorld(owner->page(), "Array.from(new Uint8Array(finishedCredential.getClientExtensionResults().prf.results.first)).join(',')", 0).toString(), QStringLiteral("1,2"));
        QCOMPARE(runWorld(owner->page(), "finishedCredential.toJSON().id", 0).toString(), QStringLiteral("AQID"));
        QCOMPARE(runWorld(owner->page(), "PublicKeyCredential.prototype.toJSON.call(finishedCredential).id", 0).toString(), QStringLiteral("AQID"));
        QVERIFY(runWorld(owner->page(), "const nativeTransports=AuthenticatorAttestationResponse.prototype.getTransports;AuthenticatorAttestationResponse.prototype.getTransports=function(){return nativeTransports.call(this).concat('prototype-fixture')};finishedCredential.response.getTransports().includes('prototype-fixture')", 0).toBool());
        QVERIFY(runWorld(owner->page(), "finishedCredential.response.getTransports=()=>['fixture'];finishedCredential.response.getTransports()[0]==='fixture'", 0).toBool());
        runWorld(owner->page(), R"JS(
            window.preAborted = false; const aborted = new AbortController(); aborted.abort('fixture reason');
            navigator.credentials.get({publicKey:{challenge:new Uint8Array([1])},signal:aborted.signal})
                .catch(error => preAborted=error === 'fixture reason');
        )JS", 0);
        QTRY_VERIFY(runWorld(owner->page(), "preAborted", 0).toBool());
        QCOMPARE(captured.count(), 1);
        runWorld(owner->page(), R"JS(
            window.abortedCorrectly = false; window.cancelling = new AbortController();
            navigator.credentials.get({publicKey:{challenge:new Uint8Array([1])},signal:cancelling.signal})
                .catch(error => abortedCorrectly=error === 'late fixture abort');
        )JS", 0);
        QTRY_COMPARE(captured.count(), 2);
        runWorld(owner->page(), "cancelling.abort('late fixture abort')", 0);
        QTRY_VERIFY(runWorld(owner->page(), "abortedCorrectly", 0).toBool());
        QTRY_VERIFY(!boundary->request());
        QVERIFY(!boundary->complete(result));
    }

    void capturedCommandsUseProductionDispatcher()
    {
        FixtureServer server;
        server.setDocuments(R"HTML(<!doctype html><title>Native commands</title>
            <body><p>needle needle Needle</p><form method='post' action='/posted'>
            <input name='field' value='original'><button>Submit</button></form>
            <div style='height:4000px'>Tall page</div></body>)HTML", "");
        QVERIFY(server.start());
        QTemporaryDir storage;
        QQmlEngine engine;
        std::unique_ptr<QObject> ui(makeBrowserUi(engine));
        QVERIFY(ui && configureBrowserUi(ui.get(), storage.path()));
        QString diagnostics;
        QVERIFY2(waitForProfileBootstrap(ui.get(), &diagnostics), qPrintable(diagnostics));
        FerricBrowserRequestInterceptor interceptor;
        FerricPublicProfileOwner profile(&interceptor);
        QObject lifetime;
        auto *owner = profile.createPage(&lifetime);
        FerricPublicPageHost host(ui.get(), profile.profile(), owner, nullptr, browserUiTabAt(ui.get(), 0));
        QVERIFY(host.isReady());
        acceptFixtureCertificate(owner->page(), server.pageUrl());
        QSignalSpy loaded(owner->page(), &QWebEnginePage::loadFinished);
        QSignalSpy results(&host, &FerricPublicPageHost::capturedCommandFinished);
        host.resize(900, 600); host.show(); host.activateWindow(); host.pageView()->setFocus();
        auto dispatch = [&] {
            return QMetaObject::invokeMethod(host.chromeView()->rootObject(), "dispatchPendingEngineRequest");
        };
        auto action = [&](const char *method) {
            return QMetaObject::invokeMethod(ui.get(), method, Qt::DirectConnection) && dispatch();
        };
        auto command = [&](const QString &text) {
            bool accepted = false;
            return QMetaObject::invokeMethod(ui.get(), "execute_command", Qt::DirectConnection,
                Q_RETURN_ARG(bool, accepted), Q_ARG(QString, text)) && accepted && dispatch();
        };
        const QString first = server.pageUrl().toString();
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "navigate", Qt::DirectConnection, Q_ARG(QString, first)));
        QVERIFY(dispatch());
        QTRY_VERIFY_WITH_TIMEOUT(!loaded.isEmpty(), 15000);
        QVERIFY(loaded.last().at(0).toBool());
        // The compositor owns a disposable clipboard. Exercise the native
        // retained field through typed browser commands, without JS editing.
        host.pageView()->setFocus();
        runWorld(owner->page(), "document.querySelector('input').focus();document.querySelector('input').select()", 0);
        QApplication::clipboard()->setText("native fixture paste");
        QVERIFY(command("edit paste"));
        QTRY_COMPARE(runWorld(owner->page(), "document.querySelector('input').value", 0).toString(), QStringLiteral("native fixture paste"));
        runWorld(owner->page(), "document.querySelector('input').select()", 0);
        QVERIFY(command("edit cut"));
        QTRY_COMPARE(runWorld(owner->page(), "document.querySelector('input').value", 0).toString(), QString());
        QCOMPARE(QApplication::clipboard()->text(), QStringLiteral("native fixture paste"));
        QVERIFY(command("edit paste-plain"));
        QTRY_COMPARE(runWorld(owner->page(), "document.querySelector('input').value", 0).toString(), QStringLiteral("native fixture paste"));
        QVERIFY(command("zoom 1.25"));
        QTRY_COMPARE(owner->page()->zoomFactor(), 1.25);
        QVERIFY(command("jseval 'window.nativeCommandProof = 7'"));
        QTRY_VERIFY(!results.isEmpty());
        QCOMPARE(runWorld(owner->page(), "window.nativeCommandProof", 1).toInt(), 7);
        QVERIFY(!runWorld(owner->page(), "window.nativeCommandProof", 0).isValid());
        results.clear();
        QVERIFY(command("scroll-to bottom"));
        QTRY_VERIFY(!results.isEmpty());
        QVERIFY(runWorld(owner->page(), "scrollY", 0).toDouble() > 500);
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "enter_search", Qt::DirectConnection, Q_ARG(bool, false)));
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "search_changed", Qt::DirectConnection, Q_ARG(QString, QStringLiteral("Needle"))));
        results.clear(); QVERIFY(dispatch());
        QTRY_VERIFY(!results.isEmpty());
        QCOMPARE(results.last().at(1).toMap().value("count").toInt(), 1);
        QVERIFY(action("escape"));
        loaded.clear(); QVERIFY(command("reload"));
        QTRY_VERIFY_WITH_TIMEOUT(!loaded.isEmpty(), 15000);
        QCOMPARE(server.requestCount("GET", "/page"), 2);
        QVERIFY(!host.findChild<QDialog *>("nativeFormReloadConfirmation"));
        loaded.clear();
        const QString second = first + "?second";
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "navigate", Qt::DirectConnection, Q_ARG(QString, second)));
        QVERIFY(dispatch());
        QTRY_VERIFY_WITH_TIMEOUT(!loaded.isEmpty(), 15000);
        loaded.clear(); QVERIFY(command("back"));
        QTRY_COMPARE_WITH_TIMEOUT(owner->page()->url().toString(), first, 15000);
        QTRY_VERIFY_WITH_TIMEOUT(!loaded.isEmpty(), 15000);
        loaded.clear(); QVERIFY(command("forward"));
        QTRY_COMPARE_WITH_TIMEOUT(owner->page()->url().toString(), second, 15000);
        QTRY_VERIFY_WITH_TIMEOUT(!loaded.isEmpty(), 15000);
        // A real POST entry requires an owned confirmation before any repeat.
        loaded.clear();
        runWorld(owner->page(), "document.querySelector('form').submit()", 0);
        QTRY_VERIFY_WITH_TIMEOUT(!loaded.isEmpty(), 15000);
        QCOMPARE(server.requestCount("POST", "/posted"), 1);
        QVERIFY(command("reload"));
        QTRY_VERIFY(host.findChild<QDialog *>("nativeFormReloadConfirmation"));
        auto *dialog = host.findChild<QDialog *>("nativeFormReloadConfirmation");
        dialog->reject();
        QTest::qWait(100);
        QCOMPARE(server.requestCount("POST", "/posted"), 1);
        QTRY_VERIFY(!host.findChild<QDialog *>("nativeFormReloadConfirmation"));
        QVERIFY(command("reload"));
        QTRY_VERIFY(host.findChild<QDialog *>("nativeFormReloadConfirmation"));
        dialog = host.findChild<QDialog *>("nativeFormReloadConfirmation");
        auto *resend = dialog->findChild<QPushButton *>("nativeFormReloadResend");
        QVERIFY(resend); loaded.clear(); QTest::mouseClick(resend, Qt::LeftButton);
        QTRY_VERIFY_WITH_TIMEOUT(!loaded.isEmpty(), 15000);
        QCOMPARE(server.requestCount("POST", "/posted"), 2);
        QTRY_VERIFY(!host.findChild<QDialog *>("nativeFormReloadConfirmation"));
        QVERIFY(command("reload"));
        QTRY_VERIFY(host.findChild<QDialog *>("nativeFormReloadConfirmation"));
        QPointer<QDialog> closingDialog = host.findChild<QDialog *>("nativeFormReloadConfirmation");
        owner->close();
        QTRY_VERIFY(!closingDialog || !closingDialog->isVisible());
        QVERIFY(!host.pageView()->isEnabled());
        QVERIFY(!host.pageView()->isVisible());
        QCOMPARE(server.requestCount("POST", "/posted"), 2);

    }

    void capturedNavigationUsesProductionDispatcher()
    {
        QTcpServer server;
        QVERIFY(server.listen(QHostAddress::LocalHost));
        QStringList requests;
        connect(&server, &QTcpServer::newConnection, this, [&] {
            while (auto *socket = server.nextPendingConnection()) {
                auto buffer = std::make_shared<QByteArray>();
                connect(socket, &QTcpSocket::readyRead, socket, [&, socket, buffer] {
                    buffer->append(socket->readAll());
                    if (!buffer->contains("\r\n\r\n")) return;
                    requests.append(QString::fromUtf8(buffer->split(' ').value(1)));
                    const QByteArray body("<!doctype html><title>Captured navigation</title><body>real page</body>");
                    socket->write("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\nContent-Length: "
                                  + QByteArray::number(body.size()) + "\r\n\r\n" + body);
                    socket->disconnectFromHost();
                });
                connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
            }
        });
        QTemporaryDir storage;
        QQmlEngine engine;
        std::unique_ptr<QObject> ui(makeBrowserUi(engine));
        QVERIFY(ui);
        QVERIFY(configureBrowserUi(ui.get(), storage.path()));
        QString diagnostics;
        QVERIFY2(waitForProfileBootstrap(ui.get(), &diagnostics), qPrintable(diagnostics));
        const QString tab = browserUiTabAt(ui.get(), 0);
        FerricBrowserRequestInterceptor interceptor;
        FerricPublicProfileOwner profile(&interceptor);
        QObject lifetime;
        auto *owner = profile.createPage(&lifetime);
        auto host = std::make_unique<FerricPublicPageHost>(ui.get(), profile.profile(), owner, nullptr, tab);
        QVERIFY(host->isReady());
        const QString url = QStringLiteral("http://127.0.0.1:%1/positive").arg(server.serverPort());
        QSignalSpy loaded(owner->page(), &QWebEnginePage::loadFinished);
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "navigate", Qt::DirectConnection, Q_ARG(QString, url)));
        QVERIFY(requests.isEmpty());
        QVERIFY(QMetaObject::invokeMethod(host->chromeView()->rootObject(), "dispatchPendingEngineRequest"));
        QTRY_VERIFY_WITH_TIMEOUT(!loaded.isEmpty(), 15000);
        QVERIFY(loaded.last().at(0).toBool());
        QVERIFY(requests.contains("/positive"));
        QCOMPARE(owner->page()->url().toString(), url);
        QTRY_COMPARE(ui->property("current_url").toString(), url);
        QTRY_COMPARE(ui->property("load_state").toString(), QStringLiteral("complete"));
        QCOMPARE(ui->property("page_title").toString(), QStringLiteral("Captured navigation"));

        // Leave an actual browser command queued across a same-URL native
        // replacement. It must not acquire the replacement's stamp on dequeue.
        auto *wakeup = host->chromeView()->rootObject()->property("requestWakeup").value<QObject *>();
        auto *pump = host->chromeView()->rootObject()->property("requestPump").value<QObject *>();
        QVERIFY(wakeup && pump);
        wakeup->setProperty("enabled", false); pump->setProperty("running", false);
        bool queued = false;
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "execute_command", Qt::DirectConnection,
            Q_RETURN_ARG(bool, queued), Q_ARG(QString, QStringLiteral("jseval 'document.documentElement.dataset.nativeQueuedProof = 1'"))));
        QVERIFY(queued);
        loaded.clear(); owner->page()->triggerAction(QWebEnginePage::Reload);
        QTRY_VERIFY_WITH_TIMEOUT(!loaded.isEmpty(), 15000);
        QVERIFY(loaded.last().at(0).toBool());
        QVERIFY(QMetaObject::invokeMethod(host->chromeView()->rootObject(), "dispatchPendingEngineRequest"));
        QTest::qWait(100);
        QCOMPARE(runWorld(owner->page(), "document.documentElement.dataset.nativeQueuedProof === undefined", 0).toBool(), true);

        const QString staleUrl = QStringLiteral("http://127.0.0.1:%1/stale").arg(server.serverPort());
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "navigate", Qt::DirectConnection, Q_ARG(QString, staleUrl)));
        QVariant captured = takeReparentRequest(ui.get());
        QVERIFY(captured.toMap().value("nativeTarget").toBool());
        QCOMPARE(captured.toMap().value("nativeHost").toString(), host->hostToken());
        QVERIFY(!captured.toMap().contains("nativePage"));
        // Real document replacement must invalidate even a still-current Rust
        // target. Do not simulate the document epoch or load lifecycle here.
        QSignalSpy reloaded(owner->page(), &QWebEnginePage::loadFinished);
        owner->page()->triggerAction(QWebEnginePage::Reload);
        QTRY_VERIFY_WITH_TIMEOUT(!reloaded.isEmpty(), 15000);
        QVERIFY(reloaded.last().at(0).toBool());
        QVERIFY(!host->executeCaptured(captured));
        QVERIFY(!requests.contains("/stale"));
        // A new browser intent produces the fresh envelope used for independent
        // owner retirement and host-destruction checks below.
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "navigate", Qt::DirectConnection, Q_ARG(QString, staleUrl)));
        captured = takeReparentRequest(ui.get());
        QVERIFY(captured.toMap().value("nativeTarget").toBool());
        owner->close();
        QVERIFY(!host->executeCaptured(captured));
        QTest::qWait(100);
        QVERIFY(!requests.contains("/stale"));
        delete owner;
        QObject replacementLifetime;
        auto *replacementOwner = profile.createPage(&replacementLifetime);
        QVERIFY(replacementOwner);
        FerricPublicPageHost replacement(ui.get(), profile.profile(), replacementOwner, nullptr, tab);
        QVERIFY(replacement.isReady());
        QSignalSpy replacementLoads(replacementOwner->page(), &QWebEnginePage::loadStarted);
        QVERIFY(!replacement.executeCaptured(captured));
        QQmlComponent replay(&engine);
        replay.setData(R"QML(
            import QtQml
            import "qrc:/qt/qml/io/github/ferricbrowser/qml/scripts/EngineRequests.js" as EngineRequests
            QtObject {
                required property var ui
                required property var host
                function execute(request) { return EngineRequests.execute(request, ui, host, host) }
            }
        )QML", QUrl());
        QVERIFY2(replay.isReady(), qPrintable(componentErrors(replay)));
        std::unique_ptr<QObject> replayObject(replay.createWithInitialProperties({
            {"ui", QVariant::fromValue(ui.get())},
            {"host", QVariant::fromValue(host->chromeView()->rootObject())}}));
        QVERIFY(replayObject);
        QVERIFY(QMetaObject::invokeMethod(replayObject.get(), "execute", Q_ARG(QVariant, captured)));
        // Retaining an envelope after destroying its host is safe: resolution
        // uses the current registry, never a raw QObject saved in the envelope.
        host.reset();
        replayObject->setProperty("host", QVariant::fromValue(replacement.chromeView()->rootObject()));
        QVERIFY(QMetaObject::invokeMethod(replayObject.get(), "execute", Q_ARG(QVariant, captured)));
        QTest::qWait(100);
        QCOMPARE(replacementLoads.count(), 0);
        QVERIFY(!requests.contains("/stale"));
    }

    void rustBrowserUiRequestReparentsSamePageAcrossHosts()
    {
        verifySamePageReparentsAcrossHosts();
    }

    void rustBrowserUiRequestReparentsSamePageAcrossHostsWithInitializer()
    {
        verifySamePageReparentsAcrossHosts();
    }

    void rustBrowserUiRequestReparentsSamePageAcrossHostsWithProfileBootstrap()
    {
        verifySamePageReparentsAcrossHosts();
    }
};

int main(int argc, char *argv[])
{
    QApplication application(argc, argv);
    if (!cxx_qt_init_crate_ferric_browser_engine_qt()) {
        qCritical() << "CXX-Qt crate initialization failed before QML module use";
        return 1;
    }

    PublicPageHostIntegrationTest test;
    return QTest::qExec(&test, argc, argv);
}
#include "tst_public_page_host_integration.moc"
