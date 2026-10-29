#include "public_page_host.h"

#include "public_page_owner.h"
#include "public_profile_owner.h"
#include "request_interceptor.h"

#include <QApplication>
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
        FerricPublicPageHost host(ui.get(), profile.profile(), owner, nullptr, tab);
        QVERIFY(host.isReady());
        const QString url = QStringLiteral("http://127.0.0.1:%1/positive").arg(server.serverPort());
        QSignalSpy loaded(owner->page(), &QWebEnginePage::loadFinished);
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "navigate", Qt::DirectConnection, Q_ARG(QString, url)));
        QVERIFY(requests.isEmpty());
        QVERIFY(QMetaObject::invokeMethod(host.chromeView()->rootObject(), "dispatchPendingEngineRequest"));
        QTRY_VERIFY_WITH_TIMEOUT(!loaded.isEmpty(), 15000);
        QVERIFY(loaded.last().at(0).toBool());
        QVERIFY(requests.contains("/positive"));
        QCOMPARE(owner->page()->url().toString(), url);
        QTRY_COMPARE(ui->property("current_url").toString(), url);
        QTRY_COMPARE(ui->property("load_state").toString(), QStringLiteral("complete"));
        QCOMPARE(ui->property("page_title").toString(), QStringLiteral("Captured navigation"));

        const QString staleUrl = QStringLiteral("http://127.0.0.1:%1/stale").arg(server.serverPort());
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "navigate", Qt::DirectConnection, Q_ARG(QString, staleUrl)));
        const QVariant captured = takeReparentRequest(ui.get());
        QVERIFY(captured.toMap().value("nativeTarget").toBool());
        QVERIFY(captured.toMap().value("nativePage").value<QObject *>() == &host);
        owner->close();
        QVERIFY(!host.executeCaptured(captured));
        QTest::qWait(100);
        QVERIFY(!requests.contains("/stale"));
        // A registered retired native owner cannot fall through to a Quick view.
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "navigate", Qt::DirectConnection, Q_ARG(QString, staleUrl)));
        QVERIFY(QMetaObject::invokeMethod(host.chromeView()->rootObject(), "dispatchPendingEngineRequest"));
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
            {"host", QVariant::fromValue(host.chromeView()->rootObject())}}));
        QVERIFY(replayObject);
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
