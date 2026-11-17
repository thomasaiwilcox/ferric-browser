#include "tst_application.h"
#include "../../src/browser_key_router.h"
#include <QQmlContext>
#include <QQmlEngine>
#include <QElapsedTimer>
#include <QEventLoop>
#include <QtWebEngineCore/qtwebenginecoreglobal.h>
#include <QFile>
#include <QCryptographicHash>
#include <QJSValue>
#include <QGuiApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QKeyEvent>
#include <QQuickItem>
#include <QScopeGuard>
#include <QSet>
#include <QTimer>
#include <QtTest>
#include <atomic>
#include <algorithm>
#include <cmath>
#include <vector>
#include <chrono>
#include <functional>

namespace {
QJsonObject runtimeFingerprint() {
    return {{"qt", QString::fromLatin1(qVersion())},
        {"webengine", QString::fromLatin1(qWebEngineVersion())},
        {"chromium", QString::fromLatin1(qWebEngineChromiumVersion())},
        {"security_patch", QString::fromLatin1(qWebEngineChromiumSecurityPatchVersion())}};
}
qint64 monotonicNsecs() {
    return std::chrono::duration_cast<std::chrono::nanoseconds>(
        std::chrono::steady_clock::now().time_since_epoch()).count();
}

// Production rendering observer only. QML/item state is read on the GUI
// thread. Render-thread callbacks capture shared atomic state, never test
// stack references or QML objects. Polling wakes the waiter, not the clock.
class RenderedStateObserver {
    struct State {
        std::atomic<quint64> requested{0}, armed{0}, synchronized{0}, completed{0};
        std::atomic<qint64> started{0}, elapsed{0}, predicateNsecs{0};
    };
    std::shared_ptr<State> state = std::make_shared<State>();
    QList<QMetaObject::Connection> connections;
    quint64 generation = 0;
public:
    RenderedStateObserver(QQuickWindow *window, std::function<bool()> ready) {
        const auto shared = state;
        connections << QObject::connect(window, &QQuickWindow::afterAnimating, window,
            [shared, ready = std::move(ready)] {
                const auto requested = shared->requested.load();
                if (!requested) { shared->armed.store(0); return; }
                const auto began = monotonicNsecs();
                const bool matched = ready();
                shared->predicateNsecs.fetch_add(monotonicNsecs() - began);
                shared->armed.store(matched ? requested : 0);
            }, Qt::DirectConnection);
        connections << QObject::connect(window, &QQuickWindow::afterSynchronizing, window,
            [shared] { shared->synchronized.store(shared->armed.load()); }, Qt::DirectConnection);
        connections << QObject::connect(window, &QQuickWindow::frameSwapped, window,
            [shared] {
                const auto current = shared->synchronized.exchange(0);
                if (!current || current != shared->requested.load() || shared->completed.load() == current) return;
                shared->elapsed.store(monotonicNsecs() - shared->started.load());
                shared->completed.store(current);
            }, Qt::DirectConnection);
    }
    ~RenderedStateObserver() {
        for (const auto &connection : connections) QObject::disconnect(connection);
    }
    double predicateMillis() const { return state->predicateNsecs.load() / 1e6; }
    double measure(const std::function<void()> &deliver, int deadlineMs = 5000) {
        state->predicateNsecs.store(0);
        state->requested.store(0);
        state->armed.store(0);
        state->synchronized.store(0);
        const auto current = ++generation;
        state->started.store(monotonicNsecs());
        state->requested.store(current);
        deliver();
        QEventLoop loop;
        QTimer notify, deadline;
        notify.setInterval(5);
        deadline.setSingleShot(true);
        QObject::connect(&notify, &QTimer::timeout, &loop, [&] {
            if (state->completed.load() == current) loop.quit();
        });
        QObject::connect(&deadline, &QTimer::timeout, &loop, &QEventLoop::quit);
        notify.start(); deadline.start(deadlineMs);
        if (state->completed.load() != current) loop.exec();
        state->requested.store(0);
        return state->completed.load() == current ? state->elapsed.load() / 1e6 : -1;
    }
};
void gesture(QQuickWindow *window, int key, const QString &text = {}) {
    QKeyEvent press(QEvent::KeyPress, key, Qt::NoModifier, text);
    QCoreApplication::sendEvent(window, &press);
    QKeyEvent release(QEvent::KeyRelease, key, Qt::NoModifier, text);
    QCoreApplication::sendEvent(window, &release);
}
}

void ApplicationTests::sitePolicyCommandRenderPerformance() {
    const bool smoke = qEnvironmentVariableIsSet("FERRIC_SITE_POLICY_PERF_SMOKE");
    if (!smoke && !qEnvironmentVariableIsSet("FERRIC_SITE_POLICY_PERF"))
        QSKIP("Opt-in 1000-sample production input/render baseline");
    const int warmups = smoke ? 1 : 10, measured = smoke ? 2 : 1000;
    QJsonArray samples;
    int failures = 1, failedIndex = -1;
    QString stage = "setup";
    double p95 = -1, p99 = -1, maximum = -1;
    const auto report = qScopeGuard([&] {
        QJsonObject result{{"journey", "normal-colon-to-command-render"}, {"warmups", warmups},
            {"run_kind", smoke ? "functional-smoke-not-baseline" : "performance-baseline"},
            {"requested_measured_samples", measured}, {"completed_measured_samples", samples.size()},
            {"samples_ms", samples}, {"qpa", QGuiApplication::platformName()}, {"runtime", runtimeFingerprint()},
            {"dpr", window->devicePixelRatio()}, {"viewport_width", window->width()},
            {"viewport_height", window->height()}, {"failures", failures},
            {"failed_sample_index", failures ? failedIndex : -1}, {"stage", stage},
            {"p95_ms", p95}, {"p99_ms", p99}, {"max_ms", maximum},
            {"p95_ceiling_ms", 20}, {"p99_ceiling_ms", 50},
            {"method", "nearest rank; render-signal timestamps; reset frame outside interval"},
            {"boundary", "constructed Qt input through production router to matching Quick frameSwapped; excludes compositor presentation"}};
        qInfo().noquote() << "FERRIC_SITE_POLICY_RENDER_JSON " + QJsonDocument(result).toJson(QJsonDocument::Compact);
    });
    auto *input = window->findChild<QQuickItem *>("commandLineInput");
    auto *surface = window->findChild<QQuickItem *>("commandSurface");
    auto *ui = evaluate("testWindow.browserUi").value<QObject *>();
    QVERIFY(input && surface && ui);
    window->requestActivate();
    QTRY_VERIFY(window->isActive());
    evaluate("testWindow.activeWebView().forceActiveFocus()");
    RenderedStateObserver commandFrame(window, [=] {
        return ui->property("mode").toString() == "command" && surface->isVisible()
            && surface->width() > 0 && surface->height() > 0 && input->isVisible()
            && input->width() > 0 && input->height() > 0 && input->hasActiveFocus()
            && input->property("text").toString().isEmpty();
    });
    RenderedStateObserver normalFrame(window, [=] {
        return ui->property("mode").toString() == "normal" && !surface->isVisible();
    });
    for (int index = 0; index < warmups + measured; ++index) {
        failedIndex = index;
        stage = index < warmups ? "warmup-entry" : "measured-entry";
        const auto elapsed = commandFrame.measure([&] { gesture(window, Qt::Key_Colon, ":"); });
        qInfo().noquote() << "FERRIC_SITE_POLICY_SAMPLE_JSON " + QJsonDocument(QJsonObject{
            {"journey", "normal-colon-to-command-render"}, {"iteration", index},
            {"warmup", index < warmups}, {"elapsed_ms", elapsed}, {"smoke", smoke}}).toJson(QJsonDocument::Compact);
        QVERIFY2(elapsed >= 0, "Command state did not reach a matching synchronized/swapped frame within 5s");
        if (index >= warmups) samples.append(elapsed);
        stage = index < warmups ? "warmup-reset" : "measured-reset";
        const auto reset = normalFrame.measure([&] { gesture(window, Qt::Key_Escape); });
        QVERIFY2(reset >= 0, "Normal reset frame timed out outside the measured interval");
    }
    if (smoke) { failures = 0; stage = "smoke-complete"; return; }
    std::vector<double> ordered;
    ordered.reserve(samples.size());
    for (const auto value : samples) ordered.push_back(value.toDouble());
    std::sort(ordered.begin(), ordered.end());
    p95 = ordered[std::ceil(ordered.size() * .95) - 1];
    p99 = ordered[std::ceil(ordered.size() * .99) - 1];
    maximum = ordered.back();
    stage = "thresholds";
    failedIndex = -1;
    QVERIFY2(p95 <= 20, "Normal colon-to-render p95 exceeds the existing 20ms ceiling");
    QVERIFY2(p99 <= 50, "Normal colon-to-render p99 exceeds the existing 50ms ceiling");
    failures = 0;
    stage = "complete";
}

void ApplicationTests::sitePolicyHintRenderPerformance_data() {
    QTest::addColumn<int>("count");
    QTest::addColumn<double>("zoom");
    QTest::newRow("500-80") << 500 << .8;
    QTest::newRow("500-100") << 500 << 1.;
    QTest::newRow("500-125") << 500 << 1.25;
    QTest::newRow("1000-100") << 1000 << 1.;
    QTest::newRow("5000-100") << 5000 << 1.;
}
void ApplicationTests::sitePolicyHintRenderPerformance() {
    const bool smoke = qEnvironmentVariableIsSet("FERRIC_SITE_POLICY_PERF_SMOKE");
    if (!smoke && !qEnvironmentVariableIsSet("FERRIC_SITE_POLICY_PERF"))
        QSKIP("Opt-in cold-document production Hint/render baseline");
    const int warmups = smoke ? 1 : 10, measured = smoke ? 2 : 100;
    QFETCH(int, count);
    QFETCH(double, zoom);
    QJsonArray samples, markerCounts, epochs, viewports, observerCosts, candidateDigests;
    const auto policyCase = qEnvironmentVariable("FERRIC_P15_HINT_POLICY", "ordinary");
    QJsonObject failedEndpoint;
    int failures = 1, failedIndex = -1, renderedMarkers = 0;
    QString stage = "setup";
    double p95 = -1, maximum = -1;
    const auto report = qScopeGuard([&] {
        const QJsonObject result{{"journey", "cold-document-hint-to-usable-render"},
            {"count", count}, {"zoom", zoom}, {"dpr", window->devicePixelRatio()},
            {"warmups", warmups}, {"requested_measured_samples", measured},
            {"run_kind", smoke ? "functional-smoke-not-baseline" : "performance-baseline"},
            {"samples_ms", samples}, {"completed_measured_samples", samples.size()},
            {"page_viewports", viewports}, {"generation_observer_predicate_ms", observerCosts},
            {"observer_policy", "inclusive timings; generation-wide observer cost until waiter return, not a latency component; never subtracted"},
            {"rendered_marker_counts", markerCounts},
            {"policy_case", policyCase}, {"configured_site_rule_count", policyCase == "nonmatching" ? 256 : 0},
            {"candidate_projection_sha256", candidateDigests},
            {"candidate_projection", "Ordered href fragment, kind, frame path, text, label and geometry; realm-local IDs excluded"},
            {"collection_epochs", epochs}, {"failures", failures},
            {"failed_sample_index", failures ? failedIndex : -1}, {"stage", stage},
            {"p95_ms", p95}, {"max_ms", maximum}, {"failed_endpoint", failedEndpoint},
            {"qpa", QGuiApplication::platformName()}, {"runtime", runtimeFingerprint()},
            {"viewport_width", window->width()}, {"viewport_height", window->height()},
            {"boundary", "fresh document/collector realm in warm process; constructed Qt input to production synchronized/swapped Hint overlay; no compositor presentation"}};
        qInfo().noquote() << "FERRIC_SITE_POLICY_RENDER_JSON " + QJsonDocument(result).toJson(QJsonDocument::Compact);
    });
    QFile fixture(QFINDTESTDATA("../fixtures/site-policy/render-targets.html"));
    QVERIFY(fixture.open(QIODevice::ReadOnly));
    fixtureHtml = fixture.readAll();
    QVERIFY(command("set --temp hints.auto_follow=never"));
    QTRY_COMPARE(evaluate("JSON.parse(testWindow.browserUi.config_json).hints.auto_follow").toString(),QString("never"));
    QVERIFY(command("set --temp hints.min_chars=2"));
    QTRY_COMPARE(evaluate("JSON.parse(testWindow.browserUi.config_json).hints.min_chars").toInt(),2);
    auto *ui = evaluate("testWindow.browserUi").value<QObject *>();
    auto *view = evaluate("testWindow.activeWebView()").value<QObject *>();
    auto *repeater = window->findChild<QQuickItem *>("hintMarkerRepeater");
    QVERIFY(ui && view && repeater);
    auto *overlay = repeater->parentItem();
    QVERIFY(overlay);
    std::unique_ptr<QSignalSpy> addedMarkers, removedMarkers;
    if (qEnvironmentVariableIsSet("FERRIC_P15_OVERLAY_DIAGNOSTIC")) {
        addedMarkers = std::make_unique<QSignalSpy>(repeater, SIGNAL(itemAdded(int,QQuickItem*)));
        removedMarkers = std::make_unique<QSignalSpy>(repeater, SIGNAL(itemRemoved(int,QQuickItem*)));
        QVERIFY(addedMarkers->isValid() && removedMarkers->isValid());
    }
    QVERIFY(command("site-policy-list --all"));
    QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.runtime_policyWorkspace.catalogue.catalogue===true").toBool(), 10000);
    const auto catalogue = QJsonDocument::fromJson(evaluate(
        "JSON.stringify(testWindow.runtime_policyWorkspace.catalogue)").toString().toUtf8()).object();
    const auto activeRules = catalogue.value("active").toArray();
    QCOMPARE(activeRules.size(), policyCase == "nonmatching" ? 256 : 0);
    for (const auto &entry : activeRules) {
        const auto rule = entry.toObject();
        QVERIFY(rule.value("matched").isBool() && !rule.value("matched").toBool());
        QVERIFY(rule.value("id").toString().startsWith("nonmatching-"));
    }
    int startingEpoch = 0;
    const auto length = [](const QVariant &value) {
        if (value.metaType() == QMetaType::fromType<QJSValue>())
            return value.value<QJSValue>().property("length").toInt();
        return int(value.toList().size());
    };
    RenderedStateObserver visibleFrame(window, [&] {
        if (ui->property("mode").toString() != "hint" || !ui->property("hint_visible").toBool()
            || window->property("hintRefreshPending").toBool()
            || window->property("hintTrackingView").value<QObject *>() != view
            || window->property("hintCollectionEpoch").toInt() <= startingEpoch
            || !overlay->isVisible() || ui->property("hint_values").toString().isEmpty()
            || length(overlay->property("laidOutHints")) != count) return false;
        const auto displayed = length(overlay->property("displayedMarkers"));
        if (displayed <= 0 || repeater->property("count").toInt() != displayed
            || (count == 500 && displayed != 500)) return false;
        for (int index = 0; index < displayed; ++index) {
            QQuickItem *marker = nullptr;
            if (!QMetaObject::invokeMethod(repeater, "itemAt", Qt::DirectConnection,
                    Q_RETURN_ARG(QQuickItem *, marker), Q_ARG(int, index))
                || !marker || !marker->isVisible() || marker->opacity() <= 0
                || marker->width() <= 0 || marker->height() <= 0) return false;
            bool label = false;
            for (auto *child : marker->childItems())
                if (child->isVisible() && child->width() > 0 && child->height() > 0
                    && !child->property("text").toString().isEmpty()) { label = true; break; }
            if (!label) return false;
        }
        renderedMarkers = displayed;
        return true;
    });
    RenderedStateObserver normalFrame(window, [&] {
        return ui->property("mode").toString() == "normal" && !overlay->isVisible();
    });
    for (int index = 0; index < warmups + measured; ++index) {
        failedIndex = index; stage = "fresh-document";
        const auto url = QString("http://127.0.0.1:%1/site-policy-render?count=%2&iteration=%3").arg(server.serverPort()).arg(count).arg(index);
        QVERIFY(command("open " + url));
        QTRY_VERIFY_WITH_TIMEOUT(page("location.href===" + quoted(url) + " && document.body.dataset.ready==='true'").toBool(), 15000);
        QCOMPARE(page("document.querySelectorAll('a').length").toInt(), count);
        // Workload zoom is explicit fixture preparation, outside timing. A
        // saved setting alone does not establish the current native view value.
        QVERIFY(view->setProperty("zoomFactor", zoom));
        QTRY_COMPARE_WITH_TIMEOUT(view->property("zoomFactor").toDouble(), zoom, 5000);
        const auto viewportReady = QString("Math.abs(innerWidth-%1)<=1 && Math.abs(innerHeight-%2)<=1 && Number(document.body.dataset.layoutWidth)===innerWidth && Number(document.body.dataset.layoutHeight)===innerHeight")
            .arg(QString::number(view->property("width").toDouble() / zoom, 'g', 17),
                 QString::number(view->property("height").toDouble() / zoom, 'g', 17));
        QTRY_VERIFY_WITH_TIMEOUT(page(viewportReady).toBool(), 5000);
        const auto beforeViewport = QJsonObject::fromVariantMap(page("({width:innerWidth,height:innerHeight,layout_width:Number(document.body.dataset.layoutWidth),layout_height:Number(document.body.dataset.layoutHeight)})").toMap());
        QVERIFY(beforeViewport["width"].toDouble() > 0 && beforeViewport["height"].toDouble() > 0);
        QVERIFY(page("typeof window.__ferric_browserHintState==='undefined'", 1).toBool());
        window->requestActivate(); QTRY_VERIFY(window->isActive());
        evaluate("testWindow.activeWebView().forceActiveFocus()");
        startingEpoch = window->property("hintCollectionEpoch").toInt();
        stage = index < warmups ? "warmup-entry" : "measured-entry";
        if (addedMarkers) { addedMarkers->clear(); removedMarkers->clear(); }
        const auto elapsed = visibleFrame.measure([&] { gesture(window, Qt::Key_F, "f"); }, 30000);
        if (addedMarkers) qInfo().noquote() << "FERRIC_P15_OVERLAY_CREATION_JSON " + QJsonDocument(QJsonObject{
            {"iteration",index},{"added",addedMarkers->size()},{"removed",removedMarkers->size()},
            {"visible_markers",renderedMarkers}}).toJson(QJsonDocument::Compact);
        qInfo().noquote() << "FERRIC_SITE_POLICY_SAMPLE_JSON " + QJsonDocument(QJsonObject{
            {"journey", "cold-document-hint-to-usable-render"}, {"iteration", index},
            {"count", count}, {"zoom", zoom}, {"warmup", index < warmups},
            {"elapsed_ms", elapsed}, {"smoke", smoke}}).toJson(QJsonDocument::Compact);
        if (elapsed < 0) {
            failedEndpoint = QJsonObject{{"mode", ui->property("mode").toString()},
                {"status", ui->property("status_text").toString()},
                {"hint_visible", ui->property("hint_visible").toBool()},
                {"refresh_pending", window->property("hintRefreshPending").toBool()},
                {"tracking_view_matches", window->property("hintTrackingView").value<QObject *>() == view},
                {"starting_epoch", startingEpoch}, {"current_epoch", window->property("hintCollectionEpoch").toInt()},
                {"overlay_visible", overlay->isVisible()}, {"overlay_opacity", overlay->opacity()},
                {"laid_out_count", length(overlay->property("laidOutHints"))},
                {"displayed_count", length(overlay->property("displayedMarkers"))},
                {"repeater_count", repeater->property("count").toInt()},
                {"rust_values_count", QJsonDocument::fromJson(ui->property("hint_values").toString().toUtf8()).array().size()},
                {"actual_zoom", view->property("zoomFactor").toDouble()}};
        }
        QVERIFY2(elapsed >= 0, "Cold Hint session/delegates did not reach the matching rendered frame");
        // Observe complete candidate coverage outside the timed interval in
        // both controls. Retain an ordered projection for paired comparison;
        // ports and realm-local element IDs change on each fresh document.
        const auto hints = QJsonDocument::fromJson(ui->property("hint_values").toString().toUtf8()).array();
        QCOMPARE(hints.size(), count);
        QSet<QString> fragments;
        QJsonArray projection;
        for (const auto &entry : hints) {
            const auto hint = entry.toObject();
            const auto fragment = QUrl(hint.value("href").toString()).fragment();
            QVERIFY(!fragments.contains(fragment));
            fragments.insert(fragment);
            projection.append(QJsonArray{fragment, hint.value("kind"), hint.value("frame_path"),
                hint.value("text"), hint.value("label"), hint.value("x"), hint.value("y"),
                hint.value("width"), hint.value("height")});
        }
        for (int target = 0; target < count; ++target)
            QVERIFY(fragments.contains(QString("target-%1").arg(target)));
        if (index >= warmups) {
            candidateDigests.append(QString::fromLatin1(QCryptographicHash::hash(
                QJsonDocument(projection).toJson(QJsonDocument::Compact), QCryptographicHash::Sha256).toHex()));
            samples.append(elapsed); markerCounts.append(renderedMarkers);
            epochs.append(window->property("hintCollectionEpoch").toInt());
            observerCosts.append(visibleFrame.predicateMillis());
            const auto afterViewport = QJsonObject::fromVariantMap(page("({width:innerWidth,height:innerHeight,layout_width:Number(document.body.dataset.layoutWidth),layout_height:Number(document.body.dataset.layoutHeight)})").toMap());
            QVERIFY(afterViewport["width"].toDouble() > 0 && afterViewport["height"].toDouble() > 0);
            viewports.append(QJsonObject{{"before_css", beforeViewport}, {"after_css", afterViewport},
                {"actual_zoom", view->property("zoomFactor").toDouble()},
                {"view_width", view->property("width").toDouble()}, {"view_height", view->property("height").toDouble()}});
        }
        stage = "untimed-reset";
        QVERIFY(normalFrame.measure([&] { gesture(window, Qt::Key_Escape); }) >= 0);
    }
    if (smoke) { failures = 0; stage = "smoke-complete"; return; }
    std::vector<double> ordered;
    for (const auto value : samples) ordered.push_back(value.toDouble());
    std::sort(ordered.begin(), ordered.end());
    p95 = ordered[std::size_t(std::ceil(ordered.size() * .95)) - 1];
    maximum = ordered.back(); stage = "thresholds"; failedIndex = -1;
    if (count == 500) QVERIFY2(p95 <= 200, "500-target cold Hint-to-render p95 exceeds the existing 200ms ceiling");
    failures = 0; stage = "complete";
}

void ApplicationTests::sitePolicyPassiveRefreshPerformance_data() {
    QTest::addColumn<int>("count");
    QTest::newRow("1000-targets") << 1000;
    QTest::newRow("500-visible") << 500;
}

void ApplicationTests::sitePolicyPassiveRefreshPerformance() {
    const bool smoke = qEnvironmentVariableIsSet("FERRIC_SITE_POLICY_PERF_SMOKE");
    if (!smoke && !qEnvironmentVariableIsSet("FERRIC_SITE_POLICY_PERF"))
        QSKIP("Opt-in production passive refresh/render baseline with 1000 actual mutations");
    QFETCH(int, count);
    constexpr int mutationsPerSample = 1000;
    const int warmups = smoke ? 1 : 10, measured = smoke ? 2 : 100;
    QJsonArray samples, markerCounts, epochs, mutations, observerCosts, sessionIds;
    QJsonObject viewport, failedEndpoint;
    int failures = 1, failedIndex = -1, renderedMarkers = 0;
    QString stage = "setup";
    double p95 = -1, maximum = -1;
    const auto report = qScopeGuard([&] {
        const QJsonObject result{{"journey", "1000-mutations-to-passive-hint-render"},
            {"count", count}, {"mutations_per_sample", mutationsPerSample}, {"zoom", 1},
            {"warmups", warmups}, {"requested_measured_samples", measured},
            {"run_kind", smoke ? "functional-smoke-not-baseline" : "performance-baseline"},
            {"samples_ms", samples}, {"completed_measured_samples", samples.size()},
            {"rendered_marker_counts", markerCounts}, {"collection_epochs", epochs},
            {"published_session_ids", sessionIds},
            {"mutation_receipts", mutations}, {"page_viewport", viewport},
            {"generation_observer_predicate_ms", observerCosts},
            {"observer_policy", "inclusive timings; generation-wide observer cost until waiter return; never subtracted"},
            {"failures", failures}, {"failed_sample_index", failures ? failedIndex : -1},
            {"stage", stage}, {"failed_endpoint", failedEndpoint}, {"p95_ms", p95}, {"max_ms", maximum},
            {"qpa", QGuiApplication::platformName()}, {"runtime", runtimeFingerprint()},
            {"dpr", window->devicePixelRatio()}, {"viewport_width", window->width()},
            {"viewport_height", window->height()},
            {"method", "nearest rank; 1000 style mutations per sample; one continuous Hint invocation; no direct refresh/collector call"},
            {"boundary", "browser JavaScript dispatch through production dirty observer/timer/collection to matching Quick frameSwapped; excludes compositor presentation"}};
        qInfo().noquote() << "FERRIC_SITE_POLICY_RENDER_JSON " + QJsonDocument(result).toJson(QJsonDocument::Compact);
    });
    QFile fixture(QFINDTESTDATA("../fixtures/site-policy/render-refresh-targets.html"));
    QVERIFY(fixture.open(QIODevice::ReadOnly));
    fixtureHtml = fixture.readAll();
    QVERIFY(command("set --temp hints.auto_follow=never"));
    QTRY_COMPARE(evaluate("JSON.parse(testWindow.browserUi.config_json).hints.auto_follow").toString(),QString("never"));
    QVERIFY(command("set --temp hints.min_chars=2"));
    QTRY_COMPARE(evaluate("JSON.parse(testWindow.browserUi.config_json).hints.min_chars").toInt(),2);
    const auto url = QString("http://127.0.0.1:%1/site-policy-passive-refresh?count=%2")
        .arg(server.serverPort()).arg(count);
    QVERIFY(command("open " + url));
    QTRY_VERIFY_WITH_TIMEOUT(page("location.href===" + quoted(url) + " && document.body.dataset.ready==='true'").toBool(), 15000);
    auto *workspace = evaluate("testWindow.hintWorkspace").value<QObject *>();
    auto *ui = evaluate("testWindow.browserUi").value<QObject *>();
    auto *view = evaluate("testWindow.activeWebView()").value<QObject *>();
    auto *repeater = window->findChild<QQuickItem *>("hintMarkerRepeater");
    QVERIFY(workspace && ui && view && repeater);
    auto *overlay = repeater->parentItem();
    QVERIFY(overlay);
    QVERIFY(view->setProperty("zoomFactor", 1.));
    QTRY_COMPARE(view->property("zoomFactor").toDouble(), 1.);
    const auto viewportReady = QString("Math.abs(innerWidth-%1)<=1 && Math.abs(innerHeight-%2)<=1 && Number(document.body.dataset.layoutWidth)===innerWidth && Number(document.body.dataset.layoutHeight)===innerHeight")
        .arg(QString::number(view->property("width").toDouble(), 'g', 17),
             QString::number(view->property("height").toDouble(), 'g', 17));
    QTRY_VERIFY_WITH_TIMEOUT(page(viewportReady).toBool(), 5000);
    viewport = QJsonObject::fromVariantMap(page("({width:innerWidth,height:innerHeight,layout_width:Number(document.body.dataset.layoutWidth),layout_height:Number(document.body.dataset.layoutHeight)})").toMap());
    QVERIFY(viewport["width"].toDouble() > 0 && viewport["height"].toDouble() > 0);
    QCOMPARE(page("document.querySelectorAll('a').length").toInt(), count);
    window->requestActivate(); QTRY_VERIFY(window->isActive());
    evaluate("testWindow.activeWebView().forceActiveFocus()");
    const auto array = [&](const QVariant &value) {
        return value.metaType() == QMetaType::fromType<QJSValue>()
            ? value.value<QJSValue>() : engine->toScriptValue(value);
    };
    struct Geometry { double x, y, width, height; };
    QHash<QString, Geometry> original;
    int startingEpoch = window->property("hintCollectionEpoch").toInt();
    int expectedRevision = 0, callbacksBefore = 0;
    double projectedShift = 0;
    QString sessionId;
    RenderedStateObserver visibleFrame(window, [&] {
        if (ui->property("mode").toString() != "hint" || !ui->property("hint_visible").toBool()
            || window->property("hintRefreshPending").toBool()
            || window->property("hintTrackingView").value<QObject *>() != view
            || window->property("hintCollectionEpoch").toInt() <= startingEpoch
            || !overlay->isVisible()) return false;
        if (expectedRevision) {
            const auto receipt = frameCallbackResult.toMap();
            if (frameCallbackCount != callbacksBefore + 1
                || receipt["revision"].toInt() != expectedRevision
                || receipt["targets"].toInt() != count
                || receipt["mutations"].toInt() != mutationsPerSample
                || receipt["total_mutations"].toInt() != mutationsPerSample * expectedRevision) return false;
            // Ordinary refresh preserves HintInteraction meanings, but publishes
            // a new HintSession authority for the freshly collected snapshot.
            const auto currentSession = workspace->property("hintPresentationSessionId").toString();
            if (currentSession.isEmpty() || currentSession == sessionId) return false;
        }
        const auto rows = array(overlay->property("laidOutHints"));
        if (!rows.isArray() || rows.property("length").toInt() != count) return false;
        if (expectedRevision) {
            // layout_key contains the assigned label and full candidate identity.
            // Check every target, rather than accepting any new collection epoch
            // or a single representative whose geometry happened to change.
            QSet<QString> seen;
            for (int index = 0; index < count; ++index) {
                const auto row = rows.property(quint32(index));
                const auto identity = row.property("layout_key").toString();
                const auto found = original.constFind(identity);
                if (found == original.cend() || seen.contains(identity)) return false;
                seen.insert(identity);
                const auto close = [](double a, double b) { return std::isfinite(a) && std::abs(a - b) < .01; };
                if (!close(row.property("x").toNumber(), found->x + projectedShift)
                    || !close(row.property("y").toNumber(), found->y)
                    || !close(row.property("width").toNumber(), found->width)
                    || !close(row.property("height").toNumber(), found->height)) return false;
            }
        }
        const auto displayed = array(overlay->property("displayedMarkers"));
        const auto displayedCount = displayed.property("length").toInt();
        if (displayedCount <= 0 || repeater->property("count").toInt() != displayedCount
            || (count == 500 && displayedCount != 500)) return false;
        for (int index = 0; index < displayedCount; ++index) {
            QQuickItem *marker = nullptr;
            const auto row = displayed.property(quint32(index));
            if (!QMetaObject::invokeMethod(repeater, "itemAt", Qt::DirectConnection,
                    Q_RETURN_ARG(QQuickItem *, marker), Q_ARG(int, index))
                || !marker || !marker->isVisible() || marker->opacity() <= 0
                || marker->width() <= 0 || marker->height() <= 0
                || std::abs(marker->x() - row.property("marker_x").toNumber()) > .01
                || std::abs(marker->y() - row.property("marker_y").toNumber()) > .01) return false;
            bool label = false;
            for (auto *child : marker->childItems()) {
                if (child->isVisible() && child->width() > 0 && child->height() > 0
                    && child->property("text").toString() == row.property("label").toString()) {
                    label = true; break;
                }
            }
            if (!label) return false;
        }
        renderedMarkers = displayedCount;
        return true;
    });
    stage = "untimed-initial-session";
    QVERIFY2(visibleFrame.measure([&] { gesture(window, Qt::Key_F, "f"); }, 30000) >= 0,
             "Initial Hint session did not render the required candidates and marker delegates");
    sessionId = workspace->property("hintPresentationSessionId").toString();
    QVERIFY(!sessionId.isEmpty());
    const auto navigationTarget = evaluate("testWindow.browserUi.navigation_target_for_index(testWindow.activeWebView().tabIndex)").toMap();
    QVERIFY(!navigationTarget["tabId"].toString().isEmpty()
        && !navigationTarget["documentId"].toString().isEmpty());
    const auto selectorSnapshot = evaluate("testWindow.browserUi.hint_selector_policy('all')").toString();
    QVERIFY(!selectorSnapshot.isEmpty());
    const auto initial = array(overlay->property("laidOutHints"));
    for (int index = 0; index < count; ++index) {
        const auto row = initial.property(quint32(index));
        const auto identity = row.property("layout_key").toString();
        QVERIFY(!identity.isEmpty() && !original.contains(identity));
        const Geometry geometry{row.property("x").toNumber(), row.property("y").toNumber(),
            row.property("width").toNumber(), row.property("height").toNumber()};
        QVERIFY(std::isfinite(geometry.x) && std::isfinite(geometry.y)
            && std::isfinite(geometry.width) && std::isfinite(geometry.height));
        original.insert(identity, geometry);
    }
    const auto scaleX = overlay->width() / viewport["width"].toDouble();
    QVERIFY(std::isfinite(scaleX) && scaleX >= .2 && scaleX <= 5);
    for (int index = 0; index < warmups + measured; ++index) {
        failedIndex = index; expectedRevision = index + 1;
        projectedShift = expectedRevision % 2 ? scaleX : 0;
        callbacksBefore = frameCallbackCount;
        startingEpoch = window->property("hintCollectionEpoch").toInt();
        sessionId = workspace->property("hintPresentationSessionId").toString();
        stage = index < warmups ? "warmup-passive-refresh" : "measured-passive-refresh";
        const auto elapsed = visibleFrame.measure([&] {
            // Dispatch only the DOM mutation. The ordinary production observer,
            // dirty poll, collection and presentation own all subsequent work.
            evaluate("testWindow.activeWebView().runJavaScript('window.refreshFixture.mutate()',0,function(v){testProbe.frameOperationComplete(v)})");
        }, 30000);
        qInfo().noquote() << "FERRIC_SITE_POLICY_SAMPLE_JSON " + QJsonDocument(QJsonObject{
            {"journey", "1000-mutations-to-passive-hint-render"}, {"iteration", index},
            {"warmup", index < warmups}, {"count", count}, {"mutations", mutationsPerSample}, {"elapsed_ms", elapsed},
            {"before_session_id", sessionId},
            {"after_session_id", workspace->property("hintPresentationSessionId").toString()},
            {"before_collection_epoch", startingEpoch},
            {"after_collection_epoch", window->property("hintCollectionEpoch").toInt()},
            {"mutation_receipt", QJsonObject::fromVariantMap(frameCallbackResult.toMap())},
            {"smoke", smoke}}).toJson(QJsonDocument::Compact);
        if (elapsed < 0) {
            failedEndpoint = QJsonObject{{"mode", ui->property("mode").toString()},
                {"status", ui->property("status_text").toString()}, {"starting_epoch", startingEpoch},
                {"current_epoch", window->property("hintCollectionEpoch").toInt()},
                {"refresh_pending", window->property("hintRefreshPending").toBool()},
                {"session_id", workspace->property("hintPresentationSessionId").toString()},
                {"pre_refresh_session_id", sessionId}, {"expected_revision", expectedRevision},
                {"laid_out_count", array(overlay->property("laidOutHints")).property("length").toInt()},
                {"rendered_marker_count", renderedMarkers},
                {"mutation_receipt", QJsonObject::fromVariantMap(frameCallbackResult.toMap())}};
        }
        QVERIFY2(elapsed >= 0, "Production passive refresh did not render the stable identities, labels and changed geometries");
        QCOMPARE(evaluate("testWindow.browserUi.navigation_target_for_index(testWindow.activeWebView().tabIndex)").toMap(), navigationTarget);
        QCOMPARE(evaluate("testWindow.browserUi.hint_selector_policy('all')").toString(), selectorSnapshot);
        if (index >= warmups) {
            samples.append(elapsed); markerCounts.append(renderedMarkers);
            epochs.append(window->property("hintCollectionEpoch").toInt());
            sessionIds.append(workspace->property("hintPresentationSessionId").toString());
            mutations.append(QJsonObject::fromVariantMap(frameCallbackResult.toMap()));
            observerCosts.append(visibleFrame.predicateMillis());
        }
    }
    if (smoke) { failures = 0; stage = "smoke-complete"; return; }
    std::vector<double> ordered;
    for (const auto value : samples) ordered.push_back(value.toDouble());
    std::sort(ordered.begin(), ordered.end());
    p95 = ordered[std::size_t(std::ceil(ordered.size() * .95)) - 1];
    maximum = ordered.back(); failures = 0; stage = "complete";
}

// P00 measurement-only draft. Append beside RenderedStateObserver in the
// performance translation unit; not compiled or qualified yet.
void ApplicationTests::sitePolicyReadyHintPerformance_data() {
    QTest::addColumn<int>("count");
    QTest::addColumn<double>("zoom");
    QTest::addColumn<QString>("operation");
    for (const auto operation : {"prefix", "selection", "query", "queued-prefix"}) {
        for (const auto size : {500, 1000, 5000}) {
            for (const auto scale : {.8, 1., 1.25}) {
                if (size != 500 && scale != 1.) continue;
                const auto name = QString("%1-%2-%3").arg(size).arg(qRound(scale * 100)).arg(operation).toUtf8();
                QTest::newRow(name.constData()) << size << scale << QString::fromLatin1(operation);
            }
        }
    }
}

void ApplicationTests::sitePolicyReadyHintPerformance() {
    const bool smoke = qEnvironmentVariableIsSet("FERRIC_SITE_POLICY_PERF_SMOKE");
    if (!smoke && !qEnvironmentVariableIsSet("FERRIC_SITE_POLICY_PERF"))
        QSKIP("Opt-in production ready/queued Hint input to matching rendered frame");
    QFETCH(int, count);
    QFETCH(double, zoom);
    QFETCH(QString, operation);
    const bool queued = operation == "queued-prefix";
    const bool queryInput = operation == "query";
    const int warmups = smoke ? 1 : 10, measured = smoke ? 2 : 100;
    QJsonArray samples, observerCosts, markerCounts, sessionIds, epochs;
    QJsonObject viewport, failedEndpoint;
    QString stage = "setup";
    int failures = 1, failedIndex = -1, renderedMarkers = 0;
    double p95 = -1, maximum = -1;
    const auto report = qScopeGuard([&] {
        const QJsonObject row{{"journey", queued ? "queued-prefix-to-hint-render" : "ready-" + operation + "-to-hint-render"},
            {"count", count}, {"zoom", zoom}, {"dpr", window->devicePixelRatio()},
            {"warmups", warmups}, {"requested_measured_samples", measured},
            {"run_kind", smoke ? "functional-smoke-not-baseline" : "performance-baseline"},
            {"samples_ms", samples}, {"completed_measured_samples", samples.size()},
            {"generation_observer_predicate_ms", observerCosts},
            {"observer_policy", "inclusive; never subtract validation/observer work"},
            {"rendered_marker_counts", markerCounts}, {"session_ids", sessionIds},
            {"collection_epochs", epochs}, {"page_viewport", viewport},
            {"failures", failures}, {"failed_sample_index", failures ? failedIndex : -1},
            {"stage", stage}, {"failed_endpoint", failedEndpoint}, {"p95_ms", p95}, {"max_ms", maximum},
            {"qpa", QGuiApplication::platformName()}, {"runtime", runtimeFingerprint()},
            {"viewport_width", window->width()}, {"viewport_height", window->height()},
            {"boundary", queued
                ? "start immediately before genuinely queued prefix key; earlier invocation outside interval; includes remaining production collection/replay to matching Quick frameSwapped"
                : "established session; constructed native key through production router/model/delegates to matching Quick frameSwapped"},
            {"excludes", "compositor presentation and final activation/fresh proof; no p99 claim"}};
        qInfo().noquote() << "FERRIC_SITE_POLICY_RENDER_JSON " + QJsonDocument(row).toJson(QJsonDocument::Compact);
    });
    QFile fixture(QFINDTESTDATA("../fixtures/site-policy/render-targets.html"));
    QVERIFY(fixture.open(QIODevice::ReadOnly));
    fixtureHtml = fixture.readAll();
    QVERIFY(command("set --temp hints.auto_follow=never"));
    QTRY_COMPARE(evaluate("JSON.parse(testWindow.browserUi.config_json).hints.auto_follow").toString(),QString("never"));
    QVERIFY(command("set --temp hints.min_chars=2"));
    QTRY_COMPARE(evaluate("JSON.parse(testWindow.browserUi.config_json).hints.min_chars").toInt(),2);
    const auto url = QString("http://127.0.0.1:%1/site-policy-ready?count=%2").arg(server.serverPort()).arg(count)
        + (queryInput ? QStringLiteral("&text=1") : QString());
    QVERIFY(command("open " + url));
    QTRY_VERIFY_WITH_TIMEOUT(page("location.href===" + quoted(url) + " && document.body.dataset.ready==='true'").toBool(), 15000);
    QCOMPARE(page("document.querySelectorAll('a').length").toInt(), count);
    auto *workspace = evaluate("testWindow.hintWorkspace").value<QObject *>();
    auto *ui = evaluate("testWindow.browserUi").value<QObject *>();
    auto *view = evaluate("testWindow.activeWebView()").value<QObject *>();
    auto *repeater = window->findChild<QQuickItem *>("hintMarkerRepeater");
    QVERIFY(workspace && ui && view && repeater);
    auto *overlay = repeater->parentItem();
    QVERIFY(overlay);
    QVERIFY(view->setProperty("zoomFactor", zoom));
    QTRY_COMPARE(view->property("zoomFactor").toDouble(), zoom);
    const auto viewportReady = QString("Math.abs(innerWidth-%1)<=1 && Math.abs(innerHeight-%2)<=1 && Number(document.body.dataset.layoutWidth)===innerWidth && Number(document.body.dataset.layoutHeight)===innerHeight")
        .arg(QString::number(view->property("width").toDouble() / zoom, 'g', 17),
             QString::number(view->property("height").toDouble() / zoom, 'g', 17));
    QTRY_VERIFY_WITH_TIMEOUT(page(viewportReady).toBool(), 5000);
    viewport = QJsonObject::fromVariantMap(page("({width:innerWidth,height:innerHeight,layout_width:Number(document.body.dataset.layoutWidth),layout_height:Number(document.body.dataset.layoutHeight)})").toMap());
    window->requestActivate(); QTRY_VERIFY(window->isActive());
    evaluate("testWindow.activeWebView().forceActiveFocus()");
    const auto object = [&](const QVariant &value) {
        return value.metaType() == QMetaType::fromType<QJSValue>() ? value.value<QJSValue>() : engine->toScriptValue(value);
    };
    QString wantedPrefix, wantedQuery, wantedActive, wantedSession, previousSession;
    QStringList wantedMatches;
    int wantedEpoch = -1;
    bool compareState = false;
    const auto matchesFrame = [&] {
        if (ui->property("mode").toString() != "hint" || !ui->property("hint_visible").toBool()
            || window->property("hintRefreshPending").toBool() || !overlay->isVisible()
            || window->property("hintTrackingView").value<QObject *>() != view) return false;
        const auto rows = object(overlay->property("laidOutHints"));
        const auto displayed = object(overlay->property("displayedMarkers"));
        if (rows.property("length").toInt() != count || displayed.property("length").toInt() <= 0
            || displayed.property("length").toInt() != repeater->property("count").toInt()
            || (count == 500 && displayed.property("length").toInt() != (compareState ? wantedMatches.size() : 500))) return false;
        const auto session = workspace->property("hintPresentationSessionId").toString();
        if (session.isEmpty()) return false;
        const auto state = object(workspace->property("hintState"));
        if (compareState) {
            if ((!wantedSession.isEmpty() && session != wantedSession)
                || (wantedSession.isEmpty() && (session == previousSession
                    || window->property("hintCollectionEpoch").toInt() <= wantedEpoch))
                || (!wantedSession.isEmpty() && window->property("hintCollectionEpoch").toInt() != wantedEpoch)
                || state.property("mode").toString() != (queryInput ? "text" : "label")
                || state.property("prefix").toString() != wantedPrefix
                || state.property("query").toString() != wantedQuery
                || state.property("active_label").toString() != wantedActive
                || object(workspace->property("pendingHintInputs")).property("length").toInt() != 0) return false;
            const auto matches = state.property("matching_labels");
            if (matches.property("length").toInt() != wantedMatches.size()) return false;
            for (int index = 0; index < wantedMatches.size(); ++index)
                if (matches.property(quint32(index)).toString() != wantedMatches[index]) return false;
        }
        const auto prefix = state.property("prefix").toString();
        const auto active = state.property("active_label").toString();
        QSet<QString> matching, expectedClusters, seenClusters;
        const auto labels = state.property("matching_labels");
        for (int index = 0; index < labels.property("length").toInt(); ++index)
            matching.insert(labels.property(quint32(index)).toString());
        for (int index = 0; index < count; ++index) {
            const auto row = rows.property(quint32(index));
            if (matching.contains(row.property("label").toString()))
                expectedClusters.insert(row.property("cluster_key").toString());
        }
        if (displayed.property("length").toInt() != expectedClusters.size()) return false;
        bool activeVisible = active.isEmpty();
        for (int index = 0; index < displayed.property("length").toInt(); ++index) {
            QQuickItem *marker = nullptr;
            const auto row = displayed.property(quint32(index));
            const auto label = row.property("label").toString();
            const auto cluster = row.property("cluster_key").toString();
            if (!matching.contains(label) || !expectedClusters.contains(cluster) || seenClusters.contains(cluster)) return false;
            seenClusters.insert(cluster);
            if (!QMetaObject::invokeMethod(repeater, "itemAt", Qt::DirectConnection,
                    Q_RETURN_ARG(QQuickItem *, marker), Q_ARG(int, index))
                || !marker || !marker->isVisible() || marker->opacity() <= 0
                || marker->width() <= 0 || marker->height() <= 0) return false;
            const auto richText = !prefix.isEmpty() && label.startsWith(prefix)
                ? "<u>" + prefix.toHtmlEscaped() + "</u>" + label.mid(prefix.size()).toHtmlEscaped()
                : label.toHtmlEscaped();
            bool textMatches = false;
            for (auto *child : marker->childItems())
                if (child->isVisible() && child->width() > 0 && child->height() > 0
                    && child->property("text").toString() == richText) { textMatches = true; break; }
            if (!textMatches || marker->z() != (label == active ? 3 : 1)) return false;
            if (label == active) activeVisible = true;
        }
        renderedMarkers = displayed.property("length").toInt();
        return activeVisible;
    };
    RenderedStateObserver rendered(window, matchesFrame);
    RenderedStateObserver normal(window, [&] {
        return ui->property("mode").toString() == "normal" && !overlay->isVisible();
    });
    stage = "untimed-initial-session";
    QVERIFY2(rendered.measure([&] { gesture(window, Qt::Key_F, "f"); }, 30000) >= 0, "Initial Hint overlay unavailable");
    if (count == 500) QCOMPARE(renderedMarkers, 500);
    const auto navigationTarget = evaluate("testWindow.browserUi.navigation_target_for_index(testWindow.activeWebView().tabIndex)").toMap();
    QVERIFY(!navigationTarget["tabId"].toString().isEmpty() && !navigationTarget["documentId"].toString().isEmpty());
    const auto selectorSnapshot = evaluate("testWindow.browserUi.hint_selector_policy('all')").toString();
    QVERIFY(!selectorSnapshot.isEmpty());
    QStringList allMatches;
    const auto initial = object(workspace->property("hintState")).property("matching_labels");
    for (int index = 0; index < initial.property("length").toInt(); ++index)
        allMatches.append(initial.property(quint32(index)).toString());
    QCOMPARE(allMatches.size(), count);
    const auto prefix = allMatches.front().left(1);
    QVERIFY(prefix.size() == 1 && prefix.front() >= QLatin1Char('a') && prefix.front() <= QLatin1Char('z'));
    QStringList prefixMatches;
    for (const auto &label : allMatches) if (label.startsWith(prefix)) prefixMatches.append(label);
    QVERIFY(!prefixMatches.isEmpty());
    const int keyCode = Qt::Key_A + prefix.front().unicode() - 'a';
    const auto settleUnfilteredFrame = [&] {
        wantedPrefix.clear();
        wantedQuery.clear();
        wantedMatches = allMatches;
        wantedActive = allMatches.front();
        compareState = true;
        QElapsedTimer deadline, stable;
        deadline.start();
        const auto ready = [&] {
            const auto session = workspace->property("hintPresentationSessionId").toString();
            const auto epoch = window->property("hintCollectionEpoch").toInt();
            if (session.isEmpty()) { stable.invalidate(); return false; }
            if (!stable.isValid() || session != wantedSession || epoch != wantedEpoch) {
                wantedSession = session;
                wantedEpoch = epoch;
                stable.start();
            }
            if (!matchesFrame()) { stable.invalidate(); return false; }
            return stable.elapsed() >= 200;
        };
        // Passive collection can legitimately replace the authority during
        // setup. Establish a fresh frame for the final stable stamp before
        // arming the timed key, whose start and end must share that stamp.
        for (;;) {
            const auto wait = 5000 - int(deadline.elapsed());
            if (wait <= 0 || !QTest::qWaitFor(ready, wait)) return false;
            const auto remaining = 5000 - int(deadline.elapsed());
            if (remaining <= 0) return false;
            if (rendered.measure([&] { window->update(); }, std::min(remaining, 1000)) >= 0
                && matchesFrame()) return true;
            stable.invalidate();
        }
    };
    QStringList queryMatches;
    if (queryInput) {
        stage = "untimed-enter-text-mode";
        const auto hints = QJsonDocument::fromJson(ui->property("hint_values").toString().toUtf8()).array();
        QCOMPARE(hints.size(), count);
        QSet<QString> textLabels, candidateLabels;
        for (const auto &value : hints) {
            const auto candidate = value.toObject();
            const auto label = candidate["label"].toString();
            const auto text = candidate["text"].toString();
            QVERIFY(!label.isEmpty() && !candidateLabels.contains(label));
            QVERIFY(text == "x" || text == "z");
            QVERIFY(!candidate["href"].toString().contains(QLatin1Char('z'), Qt::CaseInsensitive));
            candidateLabels.insert(label);
            if (text == "z") textLabels.insert(label);
        }
        for (const auto &label : allMatches) {
            QVERIFY(candidateLabels.contains(label));
            if (textLabels.contains(label)) queryMatches.append(label);
        }
        QCOMPARE(queryMatches.size(), (count + 2) / 3);
        gesture(window, Qt::Key_Slash, "/");
    }
    if (!queued) {
        stage = "untimed-ready-starting-frame";
        QVERIFY2(settleUnfilteredFrame(), "Ready Hint setup did not establish a stable unfiltered frame");
    }
    for (int index = 0; index < warmups + measured; ++index) {
        failedIndex = index;
        const bool selection = operation == "selection";
        wantedPrefix = selection || queryInput ? QString() : prefix;
        wantedQuery = queryInput ? QStringLiteral("z") : QString();
        wantedMatches = selection ? allMatches : queryInput ? queryMatches : prefixMatches;
        wantedActive = selection ? allMatches[1] : wantedMatches.front();
        bool noSession = true, genuinelyQueued = true;
        if (queued) {
            stage = "untimed-close-and-invocation";
            previousSession = workspace->property("hintPresentationSessionId").toString();
            wantedEpoch = window->property("hintCollectionEpoch").toInt();
            compareState = false;
            QVERIFY(normal.measure([&] { gesture(window, Qt::Key_Escape); }) >= 0);
            gesture(window, Qt::Key_F, "f");
            // No event pumping: the existing Rust no-session response proves
            // the next key genuinely enters the production pending-input queue.
            noSession = evaluate("JSON.parse(testWindow.browserUi.update_hint_interaction('clear','')).error").toString() == "no-session";
            wantedSession.clear();
        }
        compareState = true;
        stage = index < warmups ? "warmup-input" : "measured-input";
        const auto preparedMarkers = renderedMarkers;
        const auto elapsed = rendered.measure([&] {
            if (selection) gesture(window, Qt::Key_Tab);
            else if (queryInput) gesture(window, Qt::Key_Z, wantedQuery);
            else gesture(window, keyCode, prefix);
            if (queued) genuinelyQueued = object(workspace->property("pendingHintInputs")).property("length").toInt() == 1;
        }, 30000);
        qInfo().noquote() << "FERRIC_SITE_POLICY_SAMPLE_JSON " + QJsonDocument(QJsonObject{
            {"journey", queued ? "queued-prefix-to-hint-render" : "ready-" + operation + "-to-hint-render"},
            {"count", count}, {"zoom", zoom}, {"iteration", index}, {"warmup", index < warmups},
            {"elapsed_ms", elapsed}, {"smoke", smoke}, {"no_session_before_queued_key", noSession},
            {"genuinely_queued", genuinelyQueued}, {"rendered_marker_count", renderedMarkers},
            {"prepared_marker_count", preparedMarkers}, {"previous_session_id", previousSession},
            {"prepared_session_id", wantedSession}, {"prepared_collection_epoch", wantedEpoch},
            {"session_id", workspace->property("hintPresentationSessionId").toString()},
            {"collection_epoch", window->property("hintCollectionEpoch").toInt()}}).toJson(QJsonDocument::Compact);
        if (elapsed < 0 || !noSession || !genuinelyQueued) failedEndpoint = QJsonObject{
            {"mode", ui->property("mode").toString()}, {"status", ui->property("status_text").toString()},
            {"state", QJsonObject::fromVariantMap(object(workspace->property("hintState")).toVariant().toMap())},
            {"pending", object(workspace->property("pendingHintInputs")).property("length").toInt()},
            {"epoch", window->property("hintCollectionEpoch").toInt()},
            {"expected_epoch", wantedEpoch}, {"expected_session_id", wantedSession},
            {"actual_session_id", workspace->property("hintPresentationSessionId").toString()},
            {"expected_active", wantedActive}, {"expected_match_count", wantedMatches.size()},
            {"displayed_count", object(overlay->property("displayedMarkers")).property("length").toInt()},
            {"laid_out_count", object(overlay->property("laidOutHints")).property("length").toInt()},
            {"no_session_before_queued_key", noSession}, {"genuinely_queued", genuinelyQueued},
            {"elapsed_ms", elapsed}, {"rendered_marker_count", renderedMarkers}};
        QVERIFY2(noSession && genuinelyQueued, "Queued-input sample did not prove a real pending key");
        QVERIFY2(elapsed >= 0, "Input state and actual delegates did not reach a matching rendered frame");
        QCOMPARE(evaluate("testWindow.browserUi.navigation_target_for_index(testWindow.activeWebView().tabIndex)").toMap(), navigationTarget);
        QCOMPARE(evaluate("testWindow.browserUi.hint_selector_policy('all')").toString(), selectorSnapshot);
        if (index >= warmups) {
            samples.append(elapsed); observerCosts.append(rendered.predicateMillis()); markerCounts.append(renderedMarkers);
            sessionIds.append(workspace->property("hintPresentationSessionId").toString());
            epochs.append(window->property("hintCollectionEpoch").toInt());
        }
        if (!queued && index + 1 < warmups + measured) {
            stage = "untimed-reset";
            const bool cleared = evaluate("testWindow.hintWorkspace.applyHintInteraction('clear','')").toBool();
            const bool swapped = cleared && settleUnfilteredFrame();
            if (!swapped) {
                const auto actualState = object(workspace->property("hintState"));
                failedEndpoint = QJsonObject{{"clear_accepted", cleared},
                    {"mode", ui->property("mode").toString()}, {"hint_mode", actualState.property("mode").toString()},
                    {"prefix", actualState.property("prefix").toString()}, {"query", actualState.property("query").toString()},
                    {"active", actualState.property("active_label").toString()}, {"expected_active", wantedActive},
                    {"match_count", actualState.property("matching_labels").property("length").toInt()},
                    {"displayed_count", object(overlay->property("displayedMarkers")).property("length").toInt()},
                    {"collection_epoch", window->property("hintCollectionEpoch").toInt()}, {"expected_epoch", wantedEpoch},
                    {"session_id", workspace->property("hintPresentationSessionId").toString()}, {"expected_session_id", wantedSession}};
            }
            QVERIFY2(swapped, "Ready Hint setup did not restore the unfiltered rendered state");
        }
    }
    if (smoke) { failures = 0; stage = "smoke-complete"; return; }
    std::vector<double> ordered;
    for (const auto value : samples) ordered.push_back(value.toDouble());
    std::sort(ordered.begin(), ordered.end());
    p95 = ordered[std::size_t(std::ceil(ordered.size() * .95)) - 1]; maximum = ordered.back();
    failures = 0; failedIndex = -1; stage = "complete";
}

// P00 measurement-only draft. Requires frameCallbackNsecs to be recorded in
// the existing native frameOperationComplete callback on the same steady clock.
void ApplicationTests::sitePolicyActivationPerformance_data() {
    sitePolicyHintRenderPerformance_data();
}

void ApplicationTests::sitePolicyActivationPerformance() {
    const bool smoke = qEnvironmentVariableIsSet("FERRIC_SITE_POLICY_PERF_SMOKE");
    if (!smoke && !qEnvironmentVariableIsSet("FERRIC_SITE_POLICY_PERF"))
        QSKIP("Opt-in production selected Hint through fresh proof to actual DOM activation");
    QFETCH(int, count);
    QFETCH(double, zoom);
    const int warmups = smoke ? 1 : 10, measured = smoke ? 2 : 100;
    const auto policyCase = qEnvironmentVariable("FERRIC_P15_ACTIVATION_POLICY", "ordinary");
    QVERIFY(policyCase == "ordinary" || policyCase == "anchor");
    QJsonArray samples, receipts, preparationCosts;
    QJsonObject viewport, failedEndpoint;
    QString stage = "setup";
    int failures = 1, failedIndex = -1;
    double p95 = -1, maximum = -1;
    const auto report = qScopeGuard([&] {
        const QJsonObject row{{"journey", "selected-hint-enter-to-dom-activation"},
            {"policy_case", policyCase},
            {"count", count}, {"zoom", zoom}, {"dpr", window->devicePixelRatio()},
            {"warmups", warmups}, {"requested_measured_samples", measured},
            {"run_kind", smoke ? "functional-smoke-not-baseline" : "performance-baseline"},
            {"samples_ms", samples}, {"completed_measured_samples", samples.size()},
            {"activation_receipts", receipts}, {"untimed_preparation_predicate_ms", preparationCosts},
            {"page_viewport", viewport}, {"failures", failures},
            {"failed_sample_index", failures ? failedIndex : -1}, {"stage", stage},
            {"failed_endpoint", failedEndpoint}, {"p95_ms", p95}, {"max_ms", maximum},
            {"qpa", QGuiApplication::platformName()}, {"runtime", runtimeFingerprint()},
            {"viewport_width", window->width()}, {"viewport_height", window->height()},
            {"poll_interval_ms", 5},
            {"boundary", "constructed Enter on an already selected real Hint; production fresh validation/action to callback observing exact DOM click receipt; polling/transport included"},
            {"excludes", "collection, label preparation, DOM paint/compositor presentation; no p99 or native-transform/trust qualification"}};
        qInfo().noquote() << "FERRIC_SITE_POLICY_RENDER_JSON " + QJsonDocument(row).toJson(QJsonDocument::Compact);
    });
    QFile fixture(QFINDTESTDATA("../fixtures/site-policy/render-targets.html"));
    QVERIFY(fixture.open(QIODevice::ReadOnly));
    fixtureHtml = fixture.readAll();
    // Derive the same layout with non-navigating buttons. Ordinary Link uses
    // browser navigation; it cannot stand in for a DOM activation benchmark.
    for (const auto &old : {QByteArray("a {"), QByteArray("createElement('a')"),
             QByteArray("anchor.href = '#target-' + index;"), QByteArray("querySelectorAll('a')")})
        QCOMPARE(fixtureHtml.count(old), 1);
    fixtureHtml.replace("a {", "button { border:0; padding:0;");
    fixtureHtml.replace("createElement('a')", "createElement('button')");
    fixtureHtml.replace("anchor.href = '#target-' + index;", "anchor.type = 'button'; anchor.id = 'target-' + index;");
    fixtureHtml.replace("querySelectorAll('a')", "querySelectorAll('button')");
    // Observe only real activation. The fixture never calls click itself.
    fixtureHtml += R"(<script>
window.activationFixture={count:0,id:'',name:'',trusted:false};
document.addEventListener('click',event=>{
 const button=event.target.closest&&event.target.closest('button');if(!button)return;
 window.activationFixture={count:window.activationFixture.count+1,
 id:button.id,name:button.getAttribute('aria-label'),trusted:event.isTrusted};
},true);
</script>)";
    QVERIFY(command("set --temp hints.auto_follow=never"));
    QTRY_COMPARE(evaluate("JSON.parse(testWindow.browserUi.config_json).hints.auto_follow").toString(),QString("never"));
    QVERIFY(command("set --temp hints.min_chars=2"));
    QTRY_COMPARE(evaluate("JSON.parse(testWindow.browserUi.config_json).hints.min_chars").toInt(),2);
    const auto url = QString("http://127.0.0.1:%1/site-policy-activation?count=%2").arg(server.serverPort()).arg(count);
    QVERIFY(command("open " + url));
    QTRY_VERIFY_WITH_TIMEOUT(page("location.href===" + quoted(url) + " && document.body.dataset.ready==='true' && !!window.activationFixture").toBool(), 15000);
    QCOMPARE(page("document.querySelectorAll('button').length").toInt(), count);
    auto *workspace = evaluate("testWindow.hintWorkspace").value<QObject *>();
    auto *ui = evaluate("testWindow.browserUi").value<QObject *>();
    auto *view = evaluate("testWindow.activeWebView()").value<QObject *>();
    auto *repeater = window->findChild<QQuickItem *>("hintMarkerRepeater");
    QVERIFY(workspace && ui && view && repeater);
    auto *overlay = repeater->parentItem();
    QVERIFY(overlay);
    QVERIFY(view->setProperty("zoomFactor", zoom));
    QTRY_COMPARE(view->property("zoomFactor").toDouble(), zoom);
    const auto viewportReady = QString("Math.abs(innerWidth-%1)<=1 && Math.abs(innerHeight-%2)<=1 && Number(document.body.dataset.layoutWidth)===innerWidth && Number(document.body.dataset.layoutHeight)===innerHeight")
        .arg(QString::number(view->property("width").toDouble() / zoom, 'g', 17),
             QString::number(view->property("height").toDouble() / zoom, 'g', 17));
    QTRY_VERIFY_WITH_TIMEOUT(page(viewportReady).toBool(), 5000);
    viewport = QJsonObject::fromVariantMap(page("({width:innerWidth,height:innerHeight,layout_width:Number(document.body.dataset.layoutWidth),layout_height:Number(document.body.dataset.layoutHeight)})").toMap());
    window->requestActivate(); QTRY_VERIFY(window->isActive());
    evaluate("testWindow.activeWebView().forceActiveFocus()");
    const auto object = [&](const QVariant &value) {
        return value.metaType() == QMetaType::fromType<QJSValue>() ? value.value<QJSValue>() : engine->toScriptValue(value);
    };
    QString selectedLabel;
    int startingEpoch = -1;
    RenderedStateObserver prepared(window, [&] {
        if (ui->property("mode").toString() != "hint" || !ui->property("hint_visible").toBool()
            || window->property("hintRefreshPending").toBool() || !overlay->isVisible()
            || window->property("hintTrackingView").value<QObject *>() != view
            || window->property("hintCollectionEpoch").toInt() <= startingEpoch) return false;
        const auto rows = object(overlay->property("laidOutHints"));
        const auto displayed = object(overlay->property("displayedMarkers"));
        if (rows.property("length").toInt() != count || displayed.property("length").toInt() <= 0
            || displayed.property("length").toInt() != repeater->property("count").toInt()
            || (count == 500 && displayed.property("length").toInt() != (selectedLabel.isEmpty() ? 500 : 1))) return false;
        const auto state = object(workspace->property("hintState"));
        if (!selectedLabel.isEmpty() && (state.property("prefix").toString() != selectedLabel
            || state.property("active_label").toString() != selectedLabel
            || state.property("matching_labels").property("length").toInt() != 1)) return false;
        bool selectedVisible = selectedLabel.isEmpty();
        for (int index = 0; index < displayed.property("length").toInt(); ++index) {
            QQuickItem *marker = nullptr;
            const auto label = displayed.property(quint32(index)).property("label").toString();
            if (!QMetaObject::invokeMethod(repeater, "itemAt", Qt::DirectConnection,
                    Q_RETURN_ARG(QQuickItem *, marker), Q_ARG(int, index))
                || !marker || !marker->isVisible() || marker->opacity() <= 0
                || marker->width() <= 0 || marker->height() <= 0) return false;
            if (label == selectedLabel) {
                const auto expected = "<u>" + label.toHtmlEscaped() + "</u>";
                bool textMatches = false;
                for (auto *child : marker->childItems())
                    if (child->isVisible() && child->width() > 0 && child->height() > 0
                        && child->property("text").toString() == expected) { textMatches = true; break; }
                if (!textMatches || marker->z() != 3) return false;
                selectedVisible = true;
            }
        }
        return selectedVisible;
    });
    const auto navigationTarget = evaluate("testWindow.browserUi.navigation_target_for_index(testWindow.activeWebView().tabIndex)").toMap();
    QVERIFY(!navigationTarget["tabId"].toString().isEmpty() && !navigationTarget["documentId"].toString().isEmpty());
    const auto selectorSnapshot = evaluate("testWindow.browserUi.hint_selector_policy('all')").toString();
    QVERIFY(!selectorSnapshot.isEmpty());
    for (int index = 0; index < warmups + measured; ++index) {
        failedIndex = index; stage = "untimed-preparation";
        QCOMPARE(page("window.activationFixture.count").toInt(), index);
        selectedLabel.clear(); startingEpoch = window->property("hintCollectionEpoch").toInt();
        QVERIFY2(prepared.measure([&] { gesture(window, Qt::Key_F, "f"); }, 30000) >= 0, "Hint preparation unavailable");
        const auto collectionObserverMs = prepared.predicateMillis();
        const auto hints = QJsonDocument::fromJson(ui->property("hint_values").toString().toUtf8()).array();
        QCOMPARE(hints.size(), count);
        QSet<QString> names;
        QJsonObject selected;
        for (const auto &entry : hints) {
            const auto hint = entry.toObject();
            names.insert(hint.value("text").toString());
            if (policyCase == "anchor" && hint.value("label").toString() == "m") selected = hint;
        }
        QCOMPARE(names.size(), count);
        for (int target = 0; target < count; ++target)
            QVERIFY(names.contains(QString("Target %1").arg(target)));
        if (policyCase == "ordinary") selected = hints.first().toObject();
        else {
            const auto annotation = QJsonDocument::fromJson(
                workspace->property("hintCollectionAnnotation").toString().toUtf8()).object();
            const auto outcomes = annotation.value("outcomes").toArray();
            QCOMPARE(outcomes.size(), 1);
            const auto matches = outcomes.first().toObject().value("result").toObject()
                .value("anchor-complete").toObject().value("matches").toArray();
            QCOMPARE(matches.size(), 1);
        }
        QCOMPARE(selected.value("text").toString(), QString("Target 0"));
        selectedLabel = selected["label"].toString();
        QCOMPARE(selected["kind"].toString(), QString("button"));
        const auto targetName = selected["text"].toString();
        QVERIFY(!selectedLabel.isEmpty() && !targetName.isEmpty());
        const auto targetIds = page("Array.from(document.querySelectorAll('button')).filter(b=>b.getAttribute('aria-label')==="
            + quoted(targetName) + ").map(b=>b.id)").toList();
        QCOMPARE(targetIds.size(), 1);
        const auto targetId = targetIds.first().toString();
        QVERIFY(!targetId.isEmpty());
        for (const auto character : selectedLabel)
            QVERIFY(character >= QLatin1Char('a') && character <= QLatin1Char('z'));
        const auto labelPrepared=prepared.measure([&] {
            for (const auto character : selectedLabel) {
                gesture(window, Qt::Key_A + character.unicode() - 'a', QString(character));
            }
        });
        if(labelPrepared<0) {
            QQuickItem *marker=nullptr;
            QMetaObject::invokeMethod(repeater,"itemAt",Qt::DirectConnection,Q_RETURN_ARG(QQuickItem *,marker),Q_ARG(int,0));
            QJsonArray texts;
            if(marker)for(auto *child:marker->childItems())if(child->property("text").isValid())texts.append(child->property("text").toString());
            failedEndpoint=QJsonObject{{"mode",ui->property("mode").toString()},{"status",ui->property("status_text").toString()},
                {"selected_label",selectedLabel},{"hint_visible",ui->property("hint_visible").toBool()},
                {"published_hints",QJsonDocument::fromJson(ui->property("config_json").toString().toUtf8()).object().value("hints")},
                {"activation",QJsonObject::fromVariantMap(page("window.activationFixture").toMap())},
                {"state",QJsonObject::fromVariantMap(object(workspace->property("hintState")).toVariant().toMap())},
                {"refresh_pending",workspace->property("hintRefreshPending").toBool()},
                {"session",workspace->property("hintPresentationSessionId").toString()},
                {"collection_epoch",window->property("hintCollectionEpoch").toInt()},{"starting_epoch",startingEpoch},
                {"rows",object(overlay->property("laidOutHints")).property("length").toInt()},
                {"displayed",object(overlay->property("displayedMarkers")).property("length").toInt()},
                {"repeater",repeater->property("count").toInt()},{"overlay_visible",overlay->isVisible()},
                {"marker_z",marker?marker->z():-1},{"marker_texts",texts}};
        }
        QVERIFY(labelPrepared>=0);
        QCOMPARE(page("window.activationFixture.count").toInt(), index);
        const auto session = workspace->property("hintPresentationSessionId").toString();
        QVERIFY(!session.isEmpty());
        const int expectedCount = index + 1;
        QEventLoop loop;
        QTimer poll, deadline;
        poll.setInterval(5); deadline.setSingleShot(true);
        int previousCallbacks = frameCallbackCount;
        bool pending = false;
        qint64 completedAt = 0;
        QVariantMap receipt;
        QObject::connect(&poll, &QTimer::timeout, &loop, [&] {
            if (pending && frameCallbackCount == previousCallbacks) return;
            if (pending) {
                pending = false;
                const auto value = frameCallbackResult.toMap();
                if (value["count"].toInt() == expectedCount && value["id"].toString() == targetId
                    && value["name"].toString() == targetName) {
                    receipt = value; completedAt = frameCallbackNsecs; loop.quit(); return;
                }
            }
            previousCallbacks = frameCallbackCount; pending = true;
            evaluate("testWindow.activeWebView().runJavaScript('window.activationFixture',0,function(v){testProbe.frameOperationComplete(v)})");
        });
        QObject::connect(&deadline, &QTimer::timeout, &loop, &QEventLoop::quit);
        stage = index < warmups ? "warmup-activation" : "measured-activation";
        const auto started = monotonicNsecs();
        gesture(window, Qt::Key_Return, "\r");
        poll.start(); deadline.start(5000); loop.exec(); poll.stop(); deadline.stop();
        const auto elapsed = completedAt >= started ? (completedAt - started) / 1e6 : -1.;
        qInfo().noquote() << "FERRIC_SITE_POLICY_SAMPLE_JSON " + QJsonDocument(QJsonObject{
            {"journey", "selected-hint-enter-to-dom-activation"}, {"count", count}, {"zoom", zoom},
            {"iteration", index}, {"warmup", index < warmups}, {"elapsed_ms", elapsed},
            {"smoke", smoke}, {"activation_receipt", QJsonObject::fromVariantMap(receipt)},
            {"pre_activation_session_id", session}}).toJson(QJsonDocument::Compact);
        if (elapsed < 0) failedEndpoint = QJsonObject{{"mode", ui->property("mode").toString()},
            {"status", ui->property("status_text").toString()},
            {"last_receipt", QJsonObject::fromVariantMap(frameCallbackResult.toMap())}};
        QVERIFY2(elapsed >= 0, "The production selected Hint did not produce its exact DOM activation receipt");
        QTRY_COMPARE(ui->property("mode").toString(), QString("normal"));
        QCOMPARE(page("window.activationFixture.count").toInt(), expectedCount);
        QCOMPARE(evaluate("testWindow.browserUi.navigation_target_for_index(testWindow.activeWebView().tabIndex)").toMap(), navigationTarget);
        QCOMPARE(evaluate("testWindow.browserUi.hint_selector_policy('all')").toString(), selectorSnapshot);
        if (index >= warmups) {
            samples.append(elapsed); receipts.append(QJsonObject::fromVariantMap(receipt));
            preparationCosts.append(QJsonObject{{"collection", collectionObserverMs}, {"label", prepared.predicateMillis()}});
        }
    }
    if (smoke) { failures = 0; stage = "smoke-complete"; return; }
    std::vector<double> ordered;
    for (const auto value : samples) ordered.push_back(value.toDouble());
    std::sort(ordered.begin(), ordered.end());
    p95 = ordered[std::size_t(std::ceil(ordered.size() * .95)) - 1]; maximum = ordered.back();
    failures = 0; failedIndex = -1; stage = "complete";
}

// P00 measurement-only draft beside the existing RenderedStateObserver.
void ApplicationTests::sitePolicyRootRenderPerformance_data() {
    QTest::addColumn<int>("count");
    QTest::addColumn<double>("zoom");
    QTest::addColumn<QString>("shape");
    for (const auto shape : {"shadow-2", "slots", "same-origin-2", "foreign-2", "clipping"})
        for (const auto size : {500, 1000, 5000})
            for (const auto scale : {.8, 1., 1.25}) {
                if (size != 500 && scale != 1.) continue;
                const auto tag = QString("%1-%2-%3").arg(shape).arg(size).arg(qRound(scale * 100)).toUtf8();
                QTest::newRow(tag.constData()) << size << scale << QString::fromLatin1(shape);
            }
}

void ApplicationTests::sitePolicyRootRenderPerformance() {
    const bool smoke = qEnvironmentVariableIsSet("FERRIC_SITE_POLICY_PERF_SMOKE");
    if (!smoke && !qEnvironmentVariableIsSet("FERRIC_SITE_POLICY_PERF"))
        QSKIP("Opt-in ordinary rooted production collection/render baseline");
    QFETCH(int, count);
    QFETCH(double, zoom);
    QFETCH(QString, shape);
    const bool foreign = shape == "foreign-2";
    const bool frames = foreign || shape == "same-origin-2";
    const int expectedShadowDepth = shape == "shadow-2" ? 2 : shape == "slots" ? 1 : 0;
    const int warmups = smoke ? 1 : 10, measured = smoke ? 2 : 100;
    QJsonArray samples, observerCosts, markerCounts, rootFacts, observedPaths, truncationFlags;
    QJsonObject failedEndpoint;
    QString stage = "setup";
    int failures = 1, failedIndex = -1, renderedMarkers = 0;
    double p95 = -1, maximum = -1;
    const auto report = qScopeGuard([&] {
        const QJsonObject row{{"journey", "rooted-buttons-command-to-usable-render"},
            {"shape", shape}, {"family", "buttons"}, {"count", count}, {"zoom", zoom},
            {"dpr", window->devicePixelRatio()}, {"warmups", warmups},
            {"requested_measured_samples", measured},
            {"run_kind", smoke ? "functional-smoke-not-baseline" : "performance-baseline"},
            {"samples_ms", samples}, {"completed_measured_samples", samples.size()},
            {"generation_observer_predicate_ms", observerCosts},
            {"observer_policy", "inclusive; no validation or observer cost subtracted"},
            {"rendered_marker_counts", markerCounts}, {"fixture_root_facts", rootFacts},
            {"observed_frame_paths", observedPaths}, {"failures", failures},
            {"collection_truncated", truncationFlags},
            {"failed_sample_index", failures ? failedIndex : -1}, {"stage", stage},
            {"failed_endpoint", failedEndpoint}, {"p95_ms", p95}, {"max_ms", maximum},
            {"qpa", QGuiApplication::platformName()}, {"runtime", runtimeFingerprint()},
            {"viewport_width", window->width()}, {"viewport_height", window->height()},
            {"entry", "public BrowserUi command: hint buttons; native-key entry is a separate journey"},
            {"boundary", "fresh document/collector realm, production ordinary rooted discovery/broker/Rust/layout to matching Quick frameSwapped; no compositor presentation"},
            {"limits", "inspection counters and component collection/fresh timings require separate measurements; no p99 claim"}};
        qInfo().noquote() << "FERRIC_SITE_POLICY_RENDER_JSON " + QJsonDocument(row).toJson(QJsonDocument::Compact);
    });
    QFile fixture(QFINDTESTDATA("../fixtures/site-policy/root-render-targets.html"));
    QVERIFY(fixture.open(QIODevice::ReadOnly));
    fixtureHtml = fixture.readAll();
    QVERIFY(command("set --temp hints.auto_follow=never"));
    QTRY_COMPARE(evaluate("JSON.parse(testWindow.browserUi.config_json).hints.auto_follow").toString(),QString("never"));
    QVERIFY(command("set --temp hints.min_chars=2"));
    QTRY_COMPARE(evaluate("JSON.parse(testWindow.browserUi.config_json).hints.min_chars").toInt(),2);
    auto *ui = evaluate("testWindow.browserUi").value<QObject *>();
    auto *view = evaluate("testWindow.activeWebView()").value<QObject *>();
    auto *provider = evaluate("testWindow.hintFrameProvider").value<QObject *>();
    auto *repeater = window->findChild<QQuickItem *>("hintMarkerRepeater");
    QVERIFY(ui && view && provider && repeater);
    auto *overlay = repeater->parentItem();
    QVERIFY(overlay);
    const auto object = [&](const QVariant &value) {
        return value.metaType() == QMetaType::fromType<QJSValue>() ? value.value<QJSValue>() : engine->toScriptValue(value);
    };
    QSet<QString> expectedNames, currentPaths;
    for (int index = 0; index < count; ++index) expectedNames.insert(QString("Leaf %1").arg(index));
    int startingEpoch = 0;
    QJsonObject viewport;
    RenderedStateObserver rendered(window, [&] {
        if (ui->property("mode").toString() != "hint" || !ui->property("hint_visible").toBool()
            || window->property("hintRefreshPending").toBool() || !overlay->isVisible()
            || window->property("hintTrackingView").value<QObject *>() != view
            || window->property("hintCollectionEpoch").toInt() <= startingEpoch
            || (count == 500 && window->property("hintCollectionTruncated").toBool())) return false;
        const auto hints = QJsonDocument::fromJson(ui->property("hint_values").toString().toUtf8()).array();
        const auto rows = object(overlay->property("laidOutHints"));
        const auto displayed = object(overlay->property("displayedMarkers"));
        if (hints.size() != count || rows.property("length").toInt() != count
            || displayed.property("length").toInt() <= 0
            || displayed.property("length").toInt() != repeater->property("count").toInt()
            || (count == 500 && displayed.property("length").toInt() != 500)) return false;
        QSet<QString> names, identities, paths;
        const auto sources = object(provider->property("sources"));
        const auto rootOwner = provider->property("rootOwner").toString();
        for (const auto value : hints) {
            const auto hint = value.toObject();
            const auto name = hint["text"].toString();
            const auto path = hint["frame_path"].toString();
            const auto owner = hint["frame_owner"].toString();
            const auto identity = owner + "|" + path + "|" + hint["element_id"].toVariant().toString();
            if (hint["kind"].toString() != "button" || !expectedNames.contains(name)
                || names.contains(name) || identities.contains(identity)) return false;
            names.insert(name); identities.insert(identity); paths.insert(path);
            for (const auto key : {"x", "y", "width", "height"})
                if (!hint[key].isDouble() || !std::isfinite(hint[key].toDouble())) return false;
            if (hint["x"].toDouble() < -.01 || hint["y"].toDouble() < -.01
                || hint["width"].toDouble() <= 0 || hint["height"].toDouble() <= 0
                || hint["x"].toDouble() + hint["width"].toDouble() > viewport["width"].toDouble() + .01
                || hint["y"].toDouble() + hint["height"].toDouble() > viewport["height"].toDouble() + .01) return false;
            if (shape == "same-origin-2" && path != "0.0.0") return false;
            if (foreign) {
                if (owner.isEmpty() || rootOwner.isEmpty()) return false;
                QString current = owner;
                QSet<QString> seen;
                int hops = 0;
                while (current != rootOwner && hops < 8) {
                    if (current.isEmpty() || seen.contains(current)) return false;
                    seen.insert(current);
                    const auto source = sources.property(current);
                    if (!source.isObject()) return false;
                    current = source.property("parent").toString(); ++hops;
                }
                if (current != rootOwner || hops != 2 || provider->property("leaseCount").toInt() != 3) return false;
            } else if (!owner.isEmpty()) return false;
        }
        for (int index = 0; index < displayed.property("length").toInt(); ++index) {
            QQuickItem *marker = nullptr;
            const auto row = displayed.property(quint32(index));
            const auto label = row.property("label").toString();
            if (label.isEmpty() || !QMetaObject::invokeMethod(repeater, "itemAt", Qt::DirectConnection,
                    Q_RETURN_ARG(QQuickItem *, marker), Q_ARG(int, index))
                || !marker || !marker->isVisible() || marker->opacity() <= 0
                || !std::isfinite(marker->opacity()) || !std::isfinite(marker->width())
                || !std::isfinite(marker->height()) || marker->width() <= 0 || marker->height() <= 0) return false;
            const auto x = row.property("marker_x").toNumber(), y = row.property("marker_y").toNumber();
            if (!std::isfinite(x) || !std::isfinite(y) || !std::isfinite(marker->x()) || !std::isfinite(marker->y())
                || std::abs(marker->x() - x) > .01 || std::abs(marker->y() - y) > .01) return false;
            bool textMatches = false;
            for (auto *child : marker->childItems())
                if (child->isVisible() && child->width() > 0 && child->height() > 0
                    && child->property("text").toString() == label.toHtmlEscaped()) { textMatches = true; break; }
            if (!textMatches) return false;
        }
        currentPaths = paths; renderedMarkers = displayed.property("length").toInt();
        return true;
    });
    RenderedStateObserver normal(window, [&] {
        return ui->property("mode").toString() == "normal" && !overlay->isVisible();
    });
    for (int index = 0; index < warmups + measured; ++index) {
        failedIndex = index; stage = "fresh-document";
        const auto url = QString("http://127.0.0.1:%1/site-policy-root-render?count=%2&shape=%3&iteration=%4")
            .arg(server.serverPort()).arg(count).arg(shape).arg(index);
        QVERIFY(command("open " + url));
        QTRY_VERIFY_WITH_TIMEOUT(page("location.href===" + quoted(url) + " && document.body.dataset.ready==='true' && window.rootFixture.ready && window.rootFixture.targetCount===" + QString::number(count)).toBool(), 15000);
        QVERIFY(view->setProperty("zoomFactor", zoom));
        QTRY_COMPARE(view->property("zoomFactor").toDouble(), zoom);
        const auto readyViewport = QString("window.rootFixture.ready && window.rootFixture.readyGeneration===window.rootFixture.layoutGeneration && Math.abs(innerWidth-%1)<=1 && Math.abs(innerHeight-%2)<=1 && Number(document.body.dataset.layoutWidth)===innerWidth && Number(document.body.dataset.layoutHeight)===innerHeight")
            .arg(QString::number(view->property("width").toDouble() / zoom, 'g', 17), QString::number(view->property("height").toDouble() / zoom, 'g', 17));
        QTRY_VERIFY_WITH_TIMEOUT(page(readyViewport).toBool(), 5000);
        window->requestActivate(); QTRY_VERIFY(window->isActive());
        evaluate("testWindow.activeWebView().forceActiveFocus()");
        if (foreign) {
            QVERIFY(page("window.rootFixture.focus()").toBool());
            QTRY_VERIFY_WITH_TIMEOUT(page("window.rootFixture.ready && window.rootFixture.focused").toBool(), 5000);
        }
        const auto facts = QJsonObject::fromVariantMap(page("window.rootFixture.snapshot()").toMap());
        QCOMPARE(facts["targetCount"].toInt(), count);
        QCOMPARE(facts["physicalButtons"].toInt(), count + (shape == "clipping" ? 2 : 0));
        QCOMPARE(facts["frameHops"].toInt(), frames ? 2 : 0);
        QCOMPARE(facts["shadowRoots"].toInt(), expectedShadowDepth * count);
        QCOMPARE(facts["slots"].toInt(), shape == "slots" ? count : 0);
        QCOMPARE(facts["shadowDepthMin"].toInt(), expectedShadowDepth);
        QCOMPARE(facts["shadowDepthMax"].toInt(), expectedShadowDepth);
        QCOMPARE(facts["clippedSentinels"].toInt(), shape == "clipping" ? 2 : 0);
        viewport = facts["viewport"].toObject();
        QVERIFY(viewport["width"].toDouble() > 0 && viewport["height"].toDouble() > 0);
        QVERIFY(page("typeof window.__ferric_browserHintState==='undefined'", 1).toBool());
        startingEpoch = window->property("hintCollectionEpoch").toInt();
        currentPaths.clear();
        renderedMarkers = 0;
        bool accepted = false;
        stage = index < warmups ? "warmup-entry" : "measured-entry";
        const auto elapsed = rendered.measure([&] { accepted = command("hint buttons"); }, 30000);
        qInfo().noquote() << "FERRIC_SITE_POLICY_SAMPLE_JSON " + QJsonDocument(QJsonObject{
            {"journey", "rooted-buttons-command-to-usable-render"}, {"shape", shape}, {"count", count},
            {"zoom", zoom}, {"iteration", index}, {"warmup", index < warmups}, {"elapsed_ms", elapsed},
            {"command_accepted", accepted}, {"smoke", smoke}, {"fixture_root_facts", facts},
            {"rendered_marker_count", renderedMarkers}, {"lease_count", provider->property("leaseCount").toInt()},
            {"collection_truncated", window->property("hintCollectionTruncated").toBool()}}).toJson(QJsonDocument::Compact);
        if (elapsed < 0 || !accepted) failedEndpoint = QJsonObject{
            {"mode", ui->property("mode").toString()}, {"status", ui->property("status_text").toString()},
            {"epoch", window->property("hintCollectionEpoch").toInt()},
            {"refresh_pending", window->property("hintRefreshPending").toBool()},
            {"laid_out_count", object(overlay->property("laidOutHints")).property("length").toInt()},
            {"displayed_count", object(overlay->property("displayedMarkers")).property("length").toInt()},
            {"collection_truncated", window->property("hintCollectionTruncated").toBool()},
            {"lease_count", provider->property("leaseCount").toInt()}, {"fixture_root_facts", facts}};
        QVERIFY2(accepted && elapsed >= 0, "Ordinary rooted buttons did not reach the exact candidate/delegate endpoint");
        if (index >= warmups) {
            samples.append(elapsed); observerCosts.append(rendered.predicateMillis()); markerCounts.append(renderedMarkers);
            rootFacts.append(facts);
            truncationFlags.append(window->property("hintCollectionTruncated").toBool());
            QStringList paths(currentPaths.begin(), currentPaths.end()); paths.sort();
            observedPaths.append(QJsonArray::fromStringList(paths));
        }
        stage = "untimed-reset";
        QVERIFY(normal.measure([&] { gesture(window, Qt::Key_Escape); }) >= 0);
        QTRY_COMPARE(provider->property("leaseCount").toInt(), 0);
    }
    if (smoke) { failures = 0; stage = "smoke-complete"; return; }
    std::vector<double> ordered;
    for (const auto value : samples) ordered.push_back(value.toDouble());
    std::sort(ordered.begin(), ordered.end());
    p95 = ordered[std::size_t(std::ceil(ordered.size() * .95)) - 1]; maximum = ordered.back();
    failedIndex = -1; stage = "thresholds";
    if (count == 500) QVERIFY2(p95 <= 200, "500-visible rooted Hint render p95 exceeds the existing 200ms ceiling");
    failures = 0; stage = "complete";
}

// Independent P00 component measurements. A failed render row cannot prevent
// these raw provider operations, or the explicitly separate one-leaf controls.
#include <limits>
#include <tuple>
#include <utility>
void ApplicationTests::sitePolicyRootStagesPerformance_data() {
    QTest::addColumn<int>("count");
    QTest::addColumn<double>("zoom");
    QTest::addColumn<QString>("shape");
    QTest::addColumn<bool>("smallControl");
    QTest::addColumn<QString>("metric");
    for (const auto shape : {"shadow-2", "slots", "same-origin-2", "foreign-2", "clipping"}) {
        for (const auto size : {500, 1000, 5000})
            for (const auto scale : {.8, 1., 1.25}) {
                if (size != 500 && scale != 1.) continue;
                for (const auto metric : {"collect", "fresh"}) {
                    const auto tag = QString("%1-%2-%3-%4").arg(shape).arg(size).arg(qRound(scale * 100)).arg(metric).toUtf8();
                    QTest::newRow(tag.constData()) << size << scale << QString::fromLatin1(shape) << false << QString::fromLatin1(metric);
                }
            }
        const auto tag = QString("%1-independent-small-control").arg(shape).toUtf8();
        QTest::newRow(tag.constData()) << 1 << 1. << QString::fromLatin1(shape) << true << QString("fresh");
    }
}

void ApplicationTests::sitePolicyRootStagesPerformance() {
    const bool smoke = qEnvironmentVariableIsSet("FERRIC_SITE_POLICY_PERF_SMOKE");
    if (!smoke && !qEnvironmentVariableIsSet("FERRIC_SITE_POLICY_PERF"))
        QSKIP("Opt-in ordinary raw rooted collection and fresh-proof controls");
    QFETCH(int, count);
    QFETCH(double, zoom);
    QFETCH(QString, shape);
    QFETCH(bool, smallControl);
    QFETCH(QString, metric);
    const bool measureFresh = metric == "fresh";
    const bool foreign = shape == "foreign-2", frames = foreign || shape == "same-origin-2";
    const int depth = shape == "shadow-2" ? 2 : shape == "slots" ? 1 : 0;
    const int warmups = smoke ? 1 : 10, measured = smoke ? 2 : 100;
    QJsonArray collections, freshSamples, inspections, truncations, factsLog, geometries;
    QJsonObject failedEndpoint;
    QString stage = "setup", freshAvailability = measureFresh ? "not-attempted-no-valid-collection" : "not-requested-collection-row";
    int failures = 1, failedIndex = -1, cleanupFailures = 0;
    double collectionP95 = -1, collectionMax = -1, freshP95 = -1, freshMax = -1;
    const auto optionalNumber = [](double value) { return value >= 0 ? QJsonValue(value) : QJsonValue(QJsonValue::Null); };
    const auto report = qScopeGuard([&] {
        const QJsonObject row{{"journey", measureFresh ? "rooted-provider-fresh" : "rooted-provider-collection"}, {"metric", metric},
            {"shape", shape}, {"family", "buttons"}, {"count", count}, {"zoom", zoom},
            {"independent_small_control", smallControl}, {"dpr", window->devicePixelRatio()},
            {"warmups", warmups}, {"requested_measured_samples", measured},
            {"run_kind", smoke ? "functional-smoke-not-baseline" : "performance-baseline"},
            {"samples_ms", measureFresh ? freshSamples : collections},
            {"completed_measured_samples", measureFresh ? freshSamples.size() : collections.size()},
            {"collection_samples_ms", collections}, {"completed_collection_samples", collections.size()},
            {"collection_role", measureFresh ? "setup observations; the independent collect row owns its baseline" : "primary measurement"},
            {"fresh_samples_ms", freshSamples}, {"completed_fresh_samples", freshSamples.size()},
            {"inspected_nodes", inspections}, {"collection_truncated", truncations},
            {"fixture_root_facts", factsLog}, {"fresh_geometries", geometries},
            {"fresh_availability", freshAvailability}, {"cleanup_failures", cleanupFailures},
            {"failures", failures}, {"failed_sample_index", failures ? failedIndex : -1},
            {"stage", stage}, {"failed_endpoint", failedEndpoint},
            {"p95_ms", optionalNumber(measureFresh ? freshP95 : collectionP95)},
            {"max_ms", optionalNumber(measureFresh ? freshMax : collectionMax)},
            {"fresh_p95_ms", optionalNumber(freshP95)}, {"fresh_max_ms", optionalNumber(freshMax)},
            {"qpa", QGuiApplication::platformName()}, {"runtime", runtimeFingerprint()},
            {"viewport_width", window->width()}, {"viewport_height", window->height()},
            {"boundary", "unmodified production FrameProvider.collect/fresh to native callback observation on the same steady clock; includes transport; no Hint overlay or activation"},
            {"control_scope", smallControl ? "separate one-leaf availability control; never a replacement for a large failed workload" : "prescribed-size ordinary root workload"},
            {"method", "fresh document per iteration; 10warmups/100samples; nearest rank; no p99 or compositor claim"}};
        qInfo().noquote() << "FERRIC_SITE_POLICY_RENDER_JSON " + QJsonDocument(row).toJson(QJsonDocument::Compact);
    });
    QFile fixture(QFINDTESTDATA("../fixtures/site-policy/root-render-targets.html"));
    QVERIFY(fixture.open(QIODevice::ReadOnly)); fixtureHtml = fixture.readAll();
    auto *ui = evaluate("testWindow.browserUi").value<QObject *>();
    auto *view = evaluate("testWindow.activeWebView()").value<QObject *>();
    auto *provider = evaluate("testWindow.hintFrameProvider").value<QObject *>();
    QVERIFY(ui && view && provider);
    const auto object = [&](const QVariant &value) {
        return value.metaType() == QMetaType::fromType<QJSValue>() ? value.value<QJSValue>() : engine->toScriptValue(value);
    };
    struct Reply { double elapsed; QVariant value; bool completed; int callbacks; };
    const auto measure = [&](const std::function<void()> &submit) {
        const int before = frameCallbackCount;
        QEventLoop loop; QTimer notify, deadline;
        notify.setInterval(5); deadline.setSingleShot(true);
        QObject::connect(&notify, &QTimer::timeout, &loop, [&] { if (frameCallbackCount != before) loop.quit(); });
        QObject::connect(&deadline, &QTimer::timeout, &loop, &QEventLoop::quit);
        const auto start = monotonicNsecs(); submit();
        notify.start(); deadline.start(5000);
        if (frameCallbackCount == before) loop.exec();
        const bool completed = frameCallbackCount == before + 1 && frameCallbackNsecs >= start;
        return Reply{completed ? (frameCallbackNsecs - start) / 1e6 : -1., frameCallbackResult, completed, frameCallbackCount - before};
    };
    const auto cleanup = [&] {
        bool ok = true;
        const auto script = evaluate("testWindow.hintStopTrackingScript('')").toString();
        const auto owners = object(evaluate("Object.keys(testWindow.hintFrameProvider.sources)"));
        QStringList ids;
        for (int index = 0; index < owners.property("length").toInt(); ++index) ids.append(owners.property(quint32(index)).toString());
        if (evaluate("testWindow.hintFrameProvider.view !== null").toBool()) {
            const auto reply = measure([&] { evaluate("testWindow.hintFrameProvider.root(" + quoted(script)
                + ",function(v){testProbe.frameOperationComplete(v)})"); });
            const auto value = object(reply.value);
            ok = reply.completed && value.isBool() && value.toBool();
        }
        for (const auto &id : ids) {
            const auto reply = measure([&] { evaluate("testWindow.hintFrameProvider.run(" + quoted(id) + "," + quoted(script)
                + ",function(v){testProbe.frameOperationComplete(v)})"); });
            const auto value = object(reply.value);
            ok = reply.completed && value.isBool() && value.toBool() && ok;
        }
        evaluate("testWindow.hintFrameProvider.stop();testWindow.closeHints()");
        if (!ok) ++cleanupFailures;
        return ok;
    };
    QSet<QString> expectedNames;
    for (int index = 0; index < count; ++index) expectedNames.insert(QString("Leaf %1").arg(index));
    for (int index = 0; index < warmups + measured; ++index) {
        failedIndex = index; stage = "fresh-document";
        const auto url = QString("http://127.0.0.1:%1/site-policy-root-stages?count=%2&shape=%3&iteration=%4%5")
            .arg(server.serverPort()).arg(count).arg(shape).arg(index).arg(smallControl ? "&control=fresh" : "");
        QVERIFY(command("open " + url));
        QTRY_VERIFY_WITH_TIMEOUT(page("location.href===" + quoted(url) + " && document.body.dataset.ready==='true' && window.rootFixture.ready && window.rootFixture.targetCount===" + QString::number(count)).toBool(), 15000);
        QVERIFY(view->setProperty("zoomFactor", zoom)); QTRY_COMPARE(view->property("zoomFactor").toDouble(), zoom);
        const auto readyViewport = QString("window.rootFixture.ready && window.rootFixture.readyGeneration===window.rootFixture.layoutGeneration && Math.abs(innerWidth-%1)<=1 && Math.abs(innerHeight-%2)<=1 && Number(document.body.dataset.layoutWidth)===innerWidth && Number(document.body.dataset.layoutHeight)===innerHeight")
            .arg(QString::number(view->property("width").toDouble() / zoom, 'g', 17), QString::number(view->property("height").toDouble() / zoom, 'g', 17));
        QTRY_VERIFY_WITH_TIMEOUT(page(readyViewport).toBool(), 5000);
        // Public options and cancellation in one synchronous evaluation. No
        // collection callback can run between them; Normal retains this family.
        QVERIFY(evaluate("(function(){var ok=testWindow.browserUi.execute_command('hint buttons');testWindow.closeHints();return ok&&testWindow.browserUi.mode==='normal'&&testWindow.browserUi.hint_family==='buttons';})()").toBool());
        QTRY_VERIFY_WITH_TIMEOUT(page(readyViewport).toBool(), 5000);
        window->requestActivate(); QTRY_VERIFY(window->isActive()); evaluate("testWindow.activeWebView().forceActiveFocus()");
        if (foreign) { QVERIFY(page("window.rootFixture.focus()").toBool()); QTRY_VERIFY_WITH_TIMEOUT(page("window.rootFixture.ready && window.rootFixture.focused").toBool(), 5000); }
        const auto facts = QJsonObject::fromVariantMap(page("window.rootFixture.snapshot()").toMap());
        QCOMPARE(facts["targetCount"].toInt(), count); QCOMPARE(facts["freshControl"].toBool(), smallControl);
        QCOMPARE(facts["physicalButtons"].toInt(), count + (shape == "clipping" ? 2 : 0));
        QCOMPARE(facts["frameHops"].toInt(), frames ? 2 : 0); QCOMPARE(facts["shadowRoots"].toInt(), depth * count);
        QCOMPARE(facts["slots"].toInt(), shape == "slots" ? count : 0);
        QCOMPARE(facts["shadowDepthMin"].toInt(), depth); QCOMPARE(facts["shadowDepthMax"].toInt(), depth);
        const auto snapshot = evaluate("testWindow.hintSelectorPolicySnapshot('buttons')").toString();
        QVERIFY(!snapshot.isEmpty());
        const auto target = evaluate("testWindow.browserUi.navigation_target_for_index(testWindow.activeWebView().tabIndex)").toMap();
        QVERIFY(!target["tabId"].toString().isEmpty() && !target["documentId"].toString().isEmpty());
        const auto layoutGeneration = facts["layoutGeneration"].toDouble(-1);
        QVERIFY(std::isfinite(layoutGeneration) && layoutGeneration >= 0 && std::floor(layoutGeneration) == layoutGeneration);
        const auto stableLayout = QString("window.rootFixture.ready && window.rootFixture.readyGeneration===%1 && window.rootFixture.layoutGeneration===%1 && window.rootFixture.targetCount===%2")
            .arg(QString::number(layoutGeneration, 'g', 17)).arg(count);
        const auto unchanged = [&] {
            return ui->property("mode").toString() == "normal" && ui->property("hint_family").toString() == "buttons"
                && evaluate("testWindow.browserUi.navigation_target_for_index(testWindow.activeWebView().tabIndex)").toMap() == target
                && evaluate("testWindow.hintSelectorPolicySnapshot('buttons')").toString() == snapshot
                && page(stableLayout).toBool();
        };
        QVERIFY(unchanged());
        bool cleaned = false;
        const auto cleanupGuard = qScopeGuard([&] { if (!cleaned) cleanup(); });
        freshAvailability = measureFresh ? "not-attempted-no-valid-collection" : "not-requested-collection-row";
        stage = index < warmups ? "warmup-collection" : "measured-collection";
        const auto collected = measure([&] {
            evaluate("(function(){var s=" + quoted(snapshot) + ";testWindow.hintFrameProvider.collect(testWindow.activeWebView(),"
                "testWindow.hintCollectorScript('buttons',s,'',false),{family:'buttons',proxy:testWindow.browserUi.hint_proxy_targets_enabled,selectors:s,scope:'',pointer:false},"
                "function(v){testProbe.frameOperationComplete(v)})})()");
        });
        const auto raw = QJsonObject::fromVariantMap(object(collected.value).toVariant().toMap());
        const auto candidates = raw["candidates"].toArray();
        qInfo().noquote() << "FERRIC_SITE_POLICY_SAMPLE_JSON " + QJsonDocument(QJsonObject{
            {"journey", measureFresh ? "rooted-provider-fresh-collection-setup" : "rooted-provider-collection"}, {"metric", metric}, {"shape", shape}, {"count", count}, {"zoom", zoom},
            {"independent_small_control", smallControl}, {"iteration", index}, {"warmup", index < warmups},
            {"elapsed_ms", collected.elapsed}, {"smoke", smoke}, {"candidate_count", candidates.size()},
            {"callback_count", collected.callbacks},
            {"inspected_nodes", raw["inspected_nodes"]}, {"truncated", raw["truncated"]},
            {"error", raw["error"].toString().left(256)}, {"fixture_root_facts", facts}}).toJson(QJsonDocument::Compact);
        if (!collected.completed || raw.contains("error") || candidates.size() != count)
            failedEndpoint = QJsonObject{{"operation", "collect"}, {"completed", collected.completed},
                {"error", raw["error"].toString().left(256)}, {"candidate_count", candidates.size()},
                {"inspected_nodes", raw["inspected_nodes"]}, {"truncated", raw["truncated"]}, {"fixture_root_facts", facts}};
        QVERIFY2(collected.completed && !raw.contains("error") && candidates.size() == count, "Raw rooted collection unavailable; no fresh candidate or successful latency sample");
        const auto inspected = raw["inspected_nodes"].toDouble(-1);
        QVERIFY(std::isfinite(inspected) && inspected >= count && inspected <= 50000 && std::floor(inspected) == inspected);
        QVERIFY(raw["truncated"].isBool());
        QCOMPARE(raw["selector_revision"], QJsonDocument::fromJson(snapshot.toUtf8()).object()["selector_revision"]);
        QSet<QString> names, identities;
        const auto sources = object(provider->property("sources"));
        const auto rootOwner = provider->property("rootOwner").toString();
        for (const auto value : candidates) {
            const auto candidate = value.toObject(); const auto name = candidate["text"].toString();
            const auto path = candidate["frame_path"].toString(), owner = candidate["frame_owner"].toString();
            const auto identity = owner + "|" + path + "|" + candidate["element_id"].toVariant().toString();
            QVERIFY(candidate["kind"].toString() == "button" && expectedNames.contains(name) && !names.contains(name) && !identities.contains(identity));
            names.insert(name); identities.insert(identity);
            const auto geometry = candidate["geometry"].toObject(), viewport = facts["viewport"].toObject();
            for (const auto key : {"x", "y", "width", "height"})
                QVERIFY(geometry[key].isDouble() && std::isfinite(geometry[key].toDouble()));
            QVERIFY(geometry["x"].toDouble() >= -.01 && geometry["y"].toDouble() >= -.01
                && geometry["width"].toDouble() > 0 && geometry["height"].toDouble() > 0
                && geometry["x"].toDouble() + geometry["width"].toDouble() <= viewport["width"].toDouble() + .01
                && geometry["y"].toDouble() + geometry["height"].toDouble() <= viewport["height"].toDouble() + .01);
            if (shape == "same-origin-2") QCOMPARE(path, QString("0.0.0"));
            if (foreign) {
                QString current = owner; QSet<QString> seen; int hops = 0;
                while (current != rootOwner && hops < 8) { QVERIFY(!current.isEmpty() && !seen.contains(current)); seen.insert(current); const auto source = sources.property(current); QVERIFY(source.isObject()); current = source.property("parent").toString(); ++hops; }
                QVERIFY(!owner.isEmpty() && !rootOwner.isEmpty() && current == rootOwner && hops == 2);
                QCOMPARE(provider->property("leaseCount").toInt(), 3);
            } else QVERIFY(owner.isEmpty());
        }
        QVERIFY2(unchanged(), "Collection changed its Core target, selector policy or fixture layout subject");
        // Keep each validated collection even if a later fresh/cleanup step
        // fails. Independent collect rows never attempt the fresh operation.
        if (index >= warmups) { collections.append(collected.elapsed); inspections.append(inspected); truncations.append(raw["truncated"].toBool()); factsLog.append(facts); }
        if (measureFresh) {
            const auto selected = candidates.first().toObject();
            const auto selectedJson = QString::fromUtf8(QJsonDocument(selected).toJson(QJsonDocument::Compact));
            freshAvailability = "attempted"; stage = index < warmups ? "warmup-fresh" : "measured-fresh";
            const auto fresh = measure([&] { evaluate("(function(){var c=JSON.parse(" + quoted(selectedJson)
                + ");testWindow.hintFrameProvider.fresh(c,testWindow.hintFreshScript(c),function(v){testProbe.frameOperationComplete(v)})})()"); });
            const auto checked = QJsonObject::fromVariantMap(object(fresh.value).toVariant().toMap());
            qInfo().noquote() << "FERRIC_SITE_POLICY_SAMPLE_JSON " + QJsonDocument(QJsonObject{
                {"journey", "rooted-provider-fresh"}, {"shape", shape}, {"count", count}, {"zoom", zoom},
                {"independent_small_control", smallControl}, {"iteration", index}, {"warmup", index < warmups},
                {"elapsed_ms", fresh.elapsed}, {"smoke", smoke}, {"visible", checked["visible"]},
                {"callback_count", fresh.callbacks},
                {"error", checked["error"].toString().left(256)}}).toJson(QJsonDocument::Compact);
            if (!fresh.completed || !checked["visible"].toBool()) failedEndpoint = QJsonObject{
                {"operation", "fresh"}, {"completed", fresh.completed}, {"visible", checked["visible"]},
                {"error", checked["error"].toString().left(256)}, {"fixture_root_facts", facts}};
            QVERIFY2(fresh.completed && checked["visible"].toBool(), "The exact retained root candidate did not pass production fresh proof");
            QCOMPARE(checked["text"], selected["text"]); QCOMPARE(checked["kind"], selected["kind"]);
            QCOMPARE(checked["href"], selected["href"]); QCOMPARE(checked["element_id"], selected["element_id"]);
            QCOMPARE(checked["related_element_id"], selected["related_element_id"]);
            QCOMPARE(checked["frame_owner"], selected["frame_owner"]);
            QCOMPARE(checked["frame_path"], selected["frame_path"]);
            for (const auto key : {"x", "y", "width", "height"}) {
                const auto expected = selected["geometry"].toObject()[key].toDouble(std::numeric_limits<double>::quiet_NaN());
                const auto actual = checked["geometry"].toObject()[key].toDouble(std::numeric_limits<double>::quiet_NaN());
                QVERIFY(std::isfinite(expected) && std::isfinite(actual) && std::abs(expected - actual) <= .75);
            }
            QVERIFY2(unchanged(), "Fresh proof changed its Core target, selector policy or fixture layout subject");
            freshAvailability = "observed-valid";
            if (index >= warmups) { freshSamples.append(fresh.elapsed); geometries.append(checked["geometry"]); }
        }
        stage = "untimed-cleanup"; const bool cleanupOk = cleanup(); cleaned = true;
        QVERIFY2(cleanupOk, "Every reached root must acknowledge tracking teardown before provider release");
        QCOMPARE(provider->property("leaseCount").toInt(), 0);
    }
    if (smoke) { failures = 0; stage = "smoke-complete"; return; }
    const auto summarize = [](const QJsonArray &values) { std::vector<double> ordered; for (const auto value : values) ordered.push_back(value.toDouble()); std::sort(ordered.begin(), ordered.end()); return std::pair{ordered[std::size_t(std::ceil(ordered.size() * .95)) - 1], ordered.back()}; };
    if (measureFresh) std::tie(freshP95, freshMax) = summarize(freshSamples);
    else std::tie(collectionP95, collectionMax) = summarize(collections);
    failures = 0; failedIndex = -1; stage = "complete";
}

// P00 measurement-only candidate; not built or run. No production cleanup is
// added. Native frame handles below are read-only observations, not authority.
#include <QDir>
#include <QMap>
#include <QtWebEngineCore/QWebEngineFrame>
#include <optional>

namespace {
QString p00RenderedButtonId(QQmlApplicationEngine *engine,QObject *ui,QQuickItem *repeater,
                           const QString &name,const QString &expectedElementId,bool foreign) {
    if (!engine || !ui || !repeater) return {};
    const auto hints=QJsonDocument::fromJson(ui->property("hint_values").toString().toUtf8()).array();
    QJsonObject target;
    for (const auto &value:hints) {
        const auto row=value.toObject();
        if (row["text"].toString()!=name || row["kind"].toString()!="button") continue;
        if (!target.isEmpty()) return {};
        target=row;
    }
    const auto id=target["element_id"].toVariant().toString(), label=target["label"].toString();
    if (id.isEmpty() || label.isEmpty() || target["frame_owner"].toString().isEmpty()==foreign) return {};
    if (!expectedElementId.isEmpty() && id!=expectedElementId) return {};
    auto *overlay=repeater->parentItem();
    if (!overlay) return {};
    const auto rawDisplayed=overlay->property("displayedMarkers");
    const auto displayed=rawDisplayed.metaType()==QMetaType::fromType<QJSValue>() ? rawDisplayed.value<QJSValue>() : engine->toScriptValue(rawDisplayed);
    for (int index=0;index<displayed.property("length").toInt();++index) {
        const auto row=displayed.property(quint32(index));
        if (row.property("label").toString()!=label) continue;
        QQuickItem *marker=nullptr;
        if (!QMetaObject::invokeMethod(repeater,"itemAt",Qt::DirectConnection,
            Q_RETURN_ARG(QQuickItem *,marker),Q_ARG(int,index)) || !marker || !marker->isVisible()
            || !std::isfinite(marker->opacity()) || marker->opacity()<=0
            || !std::isfinite(marker->width()) || marker->width()<=0
            || !std::isfinite(marker->height()) || marker->height()<=0
            || !std::isfinite(marker->x()) || !std::isfinite(marker->y())) return {};
        const auto x=row.property("marker_x").toNumber(),y=row.property("marker_y").toNumber();
        if (!std::isfinite(x) || !std::isfinite(y) || std::abs(marker->x()-x)>.01 || std::abs(marker->y()-y)>.01) return {};
        bool textMatches=false;
        for (auto *child:marker->childItems()) if (child->isVisible()
            && std::isfinite(child->width()) && child->width()>0 && std::isfinite(child->height()) && child->height()>0
            && child->property("text").toString()==label.toHtmlEscaped()) {textMatches=true;break;}
        if (!textMatches) return {};
        return id;
    }
    return {};
}
QJsonObject p00ProcessResources() {
    const auto number = [](const QByteArray &bytes, const QByteArray &key) -> std::optional<qint64> {
        for (const auto &line : bytes.split('\n')) {
            if (!line.startsWith(key)) continue;
            bool ok = false;
            const auto value = line.mid(key.size()).simplified().split(' ').value(0).toLongLong(&ok);
            if (ok && value >= 0) return value;
        }
        return std::nullopt;
    };
    // Same PPID closure as pinned xtask::process_tree_pids; children may be
    // created by any thread. This is an observed, non-atomic /proc snapshot.
    QMap<qint64,QPair<qint64,QByteArray>> parents;
    QDir proc("/proc");
    for (const auto &name:proc.entryList(QDir::Dirs|QDir::NoDotAndDotDot)) {
        bool numeric=false; const auto pid=name.toLongLong(&numeric);
        if (!numeric || pid<=0) continue;
        QFile file("/proc/"+name+"/stat");
        if (!file.open(QIODevice::ReadOnly)) continue;
        const auto stat=file.readAll(); const auto end=stat.lastIndexOf(')');
        if (end<0) continue;
        const auto fields=stat.mid(end+2).simplified().split(' ');
        bool valid=false; const auto parent=fields.value(1).toLongLong(&valid);
        if (valid && fields.size()>19) parents.insert(pid,{parent,fields[19]});
    }
    QSet<qint64> tree{QCoreApplication::applicationPid()};
    for (;;) {
        const auto before=tree.size();
        for (auto entry=parents.cbegin();entry!=parents.cend();++entry)
            if (tree.contains(entry.value().first)) tree.insert(entry.key());
        if (tree.size()==before) break;
    }
    QList<qint64> pending=tree.values();
    std::sort(pending.begin(),pending.end());
    QSet<qint64> visited;
    QJsonArray observations;
    qint64 rss = 0, pss = 0, threads = 0, descriptors = 0;
    bool complete = true, pssComplete = true;
    for (int index = 0; index < pending.size() && index < 256; ++index) {
        const auto pid = pending[index];
        if (visited.contains(pid)) continue;
        visited.insert(pid);
        const auto directory = QString("/proc/%1/").arg(pid);
        const auto read = [&](const QString &name) {
            QFile file(directory + name);
            return file.open(QIODevice::ReadOnly) ? file.readAll() : QByteArray{};
        };
        const auto stat = read("stat");
        const auto end = stat.lastIndexOf(')');
        const auto fields = end >= 0 ? stat.mid(end + 2).simplified().split(' ') : QList<QByteArray>{};
        bool identityOk = false;
        const auto birth = fields.value(19).toULongLong(&identityOk);
        const auto status = read("status");
        const auto resident = number(status, "VmRSS:");
        const auto threadCount = number(status, "Threads:");
        const auto proportional = number(read("smaps_rollup"), "Pss:");
        QDir fdDirectory(directory + "fd");
        const bool fdAvailable = fdDirectory.exists() && fdDirectory.isReadable();
        const int fdCount = fdAvailable ? fdDirectory.entryList(QDir::AllEntries | QDir::System | QDir::NoDotAndDotDot).size() : -1;
        const auto current = read("stat");
        const auto currentEnd = current.lastIndexOf(')');
        const auto currentFields = currentEnd >= 0 ? current.mid(currentEnd + 2).simplified().split(' ') : QList<QByteArray>{};
        const bool identityStable = identityOk && parents.contains(pid)
            && parents.value(pid).second==QByteArray::number(birth)
            && currentFields.value(19) == QByteArray::number(birth);
        const auto command = read("cmdline");
        const bool renderer = command.contains("--type=renderer");
        const bool available = identityStable && resident && threadCount && fdAvailable;
        complete = complete && available;
        pssComplete = pssComplete && available && proportional.has_value();
        if (available) { rss += *resident; threads += *threadCount; descriptors += fdCount; }
        if (available && proportional) pss += *proportional;
        observations.append(QJsonObject{{"pid",pid},{"start_ticks",QString::number(birth)},
            {"identity_stable",identityStable},{"renderer",renderer},{"available",available},
            {"rss_kib",resident ? QJsonValue(*resident) : QJsonValue(QJsonValue::Null)},
            {"pss_kib",proportional ? QJsonValue(*proportional) : QJsonValue(QJsonValue::Null)},
            {"threads",threadCount ? QJsonValue(*threadCount) : QJsonValue(QJsonValue::Null)},
            {"fd_count",fdAvailable ? QJsonValue(fdCount) : QJsonValue(QJsonValue::Null)}});
    }
    if (visited.size() < pending.size()) complete = pssComplete = false;
    return {{"snapshot","non-atomic observed PPID descendant closure; exited/inaccessible/capped subjects unavailable"},{"process_cap",256},{"complete",complete},{"pss_complete",pssComplete},{"processes",observations},
        {"process_count",observations.size()},
        {"rss_kib",complete ? QJsonValue(rss) : QJsonValue(QJsonValue::Null)},
        {"pss_kib",pssComplete ? QJsonValue(pss) : QJsonValue(QJsonValue::Null)},
        {"threads",complete ? QJsonValue(threads) : QJsonValue(QJsonValue::Null)},
        {"fd_count",complete ? QJsonValue(descriptors) : QJsonValue(QJsonValue::Null)}};
}
}

void ApplicationTests::sitePolicyAuthoringLifetime() {
    const bool smoke=qEnvironmentVariableIsSet("FERRIC_SITE_POLICY_PERF_SMOKE");
    if(!smoke&&!qEnvironmentVariableIsSet("FERRIC_P00_LIFETIME"))QSKIP("Opt-in native authoring lifetime qualification");
    const int warmups=smoke?1:10,cycles=smoke?2:1000;
    int completed=0,completedWarmups=0,failedCycle=-1,failures=1;
    QString stage="setup";QJsonArray snapshots;
    const auto report=qScopeGuard([&]{
        if(failures&&stage=="capture")qInfo().noquote()<<"FERRIC_P15_AUTHORING_FAILURE_STATE "+evaluate(
            "JSON.stringify((()=>{const w=testWindow.runtime_policyWorkspace,r=testWindow.sitePolicyPreviewWorkspace;return {"
            "status:testWindow.browserUi.status_text,mode:testWindow.browserUi.mode,workspace_notice:w.notice,"
            "capture_notice:p15Capture.notice,capture_open:p15Capture.opened,input_available:p15Capture.inputAvailable,"
            "window_active:testWindow.active,chrome_input:testWindow.browserChromeInputActive,"
            "request:r.requestId,capture_family:r.captureFamily,capture_ready:r.captureReady,"
            "native_request:!!r.nativeRequest,leases:r.hintFrameProvider.leaseCount,pending:Object.keys(w.pending).length,"
            "draft:w.selected&&w.selected.view.id,builder_generation:w.guidedBuilder.generation,capture_generation:w.captureGeneration}})())").toString();
        qInfo().noquote()<<"FERRIC_P00_LIFETIME_JSON "+QJsonDocument(QJsonObject{
        {"journey","authoring-capture-preview-lifetime"},{"run_kind",smoke?"functional-smoke-not-baseline":"lifecycle-baseline"},
        {"warmups",warmups},{"requested_cycles",cycles},{"completed_cycles",completed},{"completed_warmups",completedWarmups},
        {"failures",failures},{"failed_cycle",failedCycle},{"stage",stage},{"snapshots",snapshots},
        {"dpr",window->devicePixelRatio()},{"qpa",QGuiApplication::platformName()},{"runtime",runtimeFingerprint()},
        {"observer","Fixed test-local helper compiled once; fresh reads and data arguments; page scripts execute in WebEngine"},
        {"scope","Real native capture selection, all-family preview receipt and draft discard per cycle; runtime/DOM cleanup checked before explicit editor reset; no apply or forced GC"}
    }).toJson(QJsonDocument::Compact);});
    QVERIFY(page("document.body.innerHTML='<a id=authoring-target href=/never>Authoring target</a>';window.authoringEvents=0;for(const type of ['click','submit','keydown','keyup'])document.addEventListener(type,()=>authoringEvents++);true").toBool());
    QVERIFY(command("site-policy"));
    auto *capture=window->findChild<QObject *>("sitePolicyCapture");QVERIFY(capture);
    auto *ui=evaluate("testWindow.browserUi").value<QObject *>();QVERIFY(ui);
    engine->rootContext()->setContextProperty("p15Capture",capture);
    const auto tab=evaluate("testWindow.activeWebView().stableTabId").toString();
    const auto pattern=QString("http://127.0.0.1:%1/*").arg(server.serverPort());
    // Compile the fixed test operations once. Recompiling an observer for
    // every poll adds QML compiler retention to the product being observed.
    // Arguments remain data; no result or product state is cached here.
    const auto helperValue=evaluate(R"JS((()=>{
        const w=testWindow.runtime_policyWorkspace,r=testWindow.sitePolicyPreviewWorkspace,b=w.guidedBuilder;
        return {
            open:function(pattern){w.openRule('temporary','authoring-lifetime',true,pattern)},
            selected:function(){return w.selected!==null},
            draft:function(){return w.selected.view.id},
            validate:function(){w.selected.proposals[0].interaction={enabled:true,labels:{auto_follow:'never',fallback_prefix:'z'},anchors:[{id:'target',families:['all'],code:'m',target_selector:'#authoring-target'}]};w.selected.editEpoch++;w.submit(w.selected,{action:'validate'})},
            validated:function(){return !w.selected.pending&&w.selected.storedEpoch===w.selected.editEpoch},
            capture:function(){w.startCapture('all')},
            captureReady:function(){return r.captureReady},
            captureId:function(){return r.requestId},
            label:function(){return (p15Capture.rows.find(function(row){return row.candidate.text==='Authoring target'})||{}).label||''},
            factsCurrent:function(id){return !!b.capturedFacts&&b.capturedFacts.capture_id===id},
            preview:function(tab){w.submit(w.selected,{action:'preview',tab:tab})},
            discard:function(id){w.request({action:'discard',draft:id},{action:'discard',session:w.selected})},
            sessionCount:function(){return w.sessions.length},
            leases:function(){return r.hintFrameProvider.leaseCount},
            runtimeClean:function(){return !r.activeView&&!r.nativeRequest&&!r.requestId&&!r.hintFrameProvider.view&&Object.keys(r.hintFrameProvider.sources).length===0},
            reset:function(){b.reset()},
            page:function(script,world){testWindow.activeWebView().runJavaScript(script,world,function(value){testProbe.accept(value)})},
            owned:function(){const p=r.hintFrameProvider;return {leases:p.leaseCount,sources:Object.keys(p.sources).length,view:!!r.activeView,request:!!r.nativeRequest,pending:!!r.requestId,poll:p.pollPending,sessions:w.sessions.length,pending_requests:Object.keys(w.pending).length,facts:!!b.capturedFacts,samples:b.keptSamples.length}}
        };
    })())JS");
    auto helper=helperValue.value<QJSValue>();
    if(!helper.isObject())helper=engine->toScriptValue(helperValue);
    QVERIFY(helper.isObject());
    const auto invoke=[&](const char *name,const QJSValueList &arguments=QJSValueList{}) {
        auto function=helper.property(QString::fromLatin1(name));
        if(!function.isCallable())qFatal("Missing authoring helper %s",name);
        auto value=function.call(arguments);
        if(value.isError())qFatal("Authoring helper %s: %s",name,qPrintable(value.toString()));
        return value;
    };
    const auto observePage=[&](const QString &script,int world=0) {
        scriptDone=false;
        invoke("page",{QJSValue(script),QJSValue(world)});
        QElapsedTimer deadline;deadline.start();
        while(!scriptDone&&deadline.elapsed()<5000)QTest::qWait(10);
        if(!scriptDone)qFatal("Authoring page observation timed out");
        return scriptResult;
    };
    for(int index=-warmups;index<cycles;++index) {
        failedCycle=index;stage="draft";
        invoke("open",{QJSValue(pattern)});
        QTRY_VERIFY_WITH_TIMEOUT(invoke("selected").toBool(),10000);
        const auto draft=invoke("draft").toString();QVERIFY(!draft.isEmpty());
        invoke("validate");
        QTRY_VERIFY_WITH_TIMEOUT(invoke("validated").toBool(),10000);
        stage="capture";invoke("capture");
        QTRY_VERIFY_WITH_TIMEOUT(invoke("captureReady").toBool(),20000);
        const auto captureId=invoke("captureId").toString();
        const auto label=invoke("label").toString();
        QVERIFY(!label.isEmpty());typeText(label);key(Qt::Key_Return);
        QTRY_VERIFY_WITH_TIMEOUT(invoke("factsCurrent",{QJSValue(captureId)}).toBool(),10000);
        stage="preview";invoke("preview",{QJSValue(tab)});
        const auto receipt=[&]{
            // Poll the same metadata method without compiling a new QML
            // expression containing this draft UUID on every observation.
            QString snapshot;
            if(!QMetaObject::invokeMethod(ui,"site_policy_draft_snapshot",Qt::DirectConnection,
                    Q_RETURN_ARG(QString,snapshot),Q_ARG(QString,draft)))
                qFatal("Could not read the authoring draft snapshot");
            return QJsonDocument::fromJson(snapshot.toUtf8()).object().value("preview").toObject();
        };
        QTRY_VERIFY_WITH_TIMEOUT(receipt().value("observations").toObject().value("complete").toBool(),20000);
        QCOMPARE(receipt().value("kind").toString(),QString("live"));
        QCOMPARE(receipt().value("observations").toObject().value("invocations").toArray()[0].toObject().value("result").toObject().value("reservations").toArray()[0].toObject().value("state").toString(),QString("Live"));
        stage="discard";
        invoke("discard",{QJSValue(draft)});
        QTRY_COMPARE_WITH_TIMEOUT(invoke("sessionCount").toInt(),0,10000);
        QTRY_COMPARE(invoke("leases").toInt(),0);
        QTRY_VERIFY(invoke("runtimeClean").toBool());
        QTRY_VERIFY(observePage("(()=>{const s=window.__ferric_browserHintState;return !s||(!s.tracking&&s.elements.size===0&&s.observers.length===0&&s.windows.length===0&&!s.scopeLease&&!s.pendingInteraction&&!s.interactionProof)})()",1).toBool());
        invoke("reset");
        QCOMPARE(observePage("authoringEvents").toInt(),0);
        if(index<0)++completedWarmups;else completed=index+1;
        if(index==-1 || index>=0&&((index+1)%100==0||index+1==cycles)) {
            QCoreApplication::sendPostedEvents(nullptr,QEvent::DeferredDelete);
            auto *router=window->findChild<FerricBrowserKeyRouter *>();QVERIFY(router);
            QCOMPARE(router->feasibilitySourceCount(),0);QCOMPARE(router->feasibilityOutputCount(),0);
            const auto owned=invoke("owned").toVariant().toMap();
            QVERIFY(owned.value("leases").toInt()==0&&owned.value("sources").toInt()==0&&owned.value("sessions").toInt()==0&&owned.value("pending_requests").toInt()==0&&owned.value("samples").toInt()==0&&!owned.value("facts").toBool());
            QVERIFY(!owned.value("view").toBool()&&!owned.value("request").toBool()&&!owned.value("pending").toBool()&&!owned.value("poll").toBool());
            snapshots.append(QJsonObject{{"cycle",index<0?0:completed},{"cleanup_clean",true},{"owned",QJsonObject::fromVariantMap(owned)},
                {"qt_descendants",window->findChildren<QObject *>().size()},{"qt_windows",QGuiApplication::allWindows().size()},{"process_resources",p00ProcessResources()}});
            qInfo().noquote()<<"FERRIC_P00_LIFETIME_CHECKPOINT_JSON "+QJsonDocument(snapshots.last().toObject()).toJson(QJsonDocument::Compact);
        }
    }
    stage="complete";failedCycle=-1;failures=0;
}

void ApplicationTests::sitePolicyOrdinaryLifetime_data() {
    QTest::addColumn<bool>("foreign");
    QTest::newRow("ordinary-established") << false;
    QTest::newRow("foreign-established") << true;
}
void ApplicationTests::sitePolicyOrdinaryLifetime() {
    const bool smoke = qEnvironmentVariableIsSet("FERRIC_SITE_POLICY_PERF_SMOKE");
    if (!smoke && !qEnvironmentVariableIsSet("FERRIC_P00_LIFETIME"))
        QSKIP("Opt-in original ordinary lifecycle resource observations");
    QFETCH(bool, foreign);
    const int warmups = smoke ? 1 : 10, cycles = smoke ? 2 : 1000;
    QJsonArray snapshots;
    int completed = 0, completedWarmups=0, failedCycle=-1, failures = 1;
    QString stage = "setup";
    const auto report = qScopeGuard([&] {
        qInfo().noquote() << "FERRIC_P00_LIFETIME_JSON " + QJsonDocument(QJsonObject{
            {"journey",foreign ? "foreign-established-lifetime" : "ordinary-established-lifetime"},
            {"run_kind",smoke ? "functional-smoke-not-baseline" : "lifecycle-baseline"},
            {"warmups",warmups},{"requested_cycles",cycles},{"completed_cycles",completed},
            {"failures",failures},{"failed_cycle",failures?failedCycle:-1},{"completed_warmups",completedWarmups},{"stage",stage},{"snapshots",snapshots},
            {"qpa",QGuiApplication::platformName()},{"dpr",window->devicePixelRatio()},
            {"runtime",runtimeFingerprint()},
            {"scope","Established ordinary Hint cycles; native F/Escape, actual current session/render. Read-only native-frame observations after production close; no injected stop/collect/GC before cleanup observations. No callback-retirement or detached-DOM collection claim."}
        }).toJson(QJsonDocument::Compact);
    });
    const auto port = server.serverPort();
    const QString childUrl = QString("http://localhost:%1/p00-lifetime-child").arg(port);
    const QString rootUrl = QString("http://127.0.0.1:%1/p00-lifetime-root").arg(port);
    const QByteArray marker = "<script>document.documentElement.dataset.p00LifetimeDocument=crypto.randomUUID()</script>";
    const QByteArray control = "<!doctype html><meta charset='utf-8'>" + marker + "<button id='p00-life' style='width:180px;height:35px'>Lifetime control</button>";
    frameFixtures["/p00-lifetime-child"] = control;
    fixtureHtml = foreign ? QString("<!doctype html><iframe id='p00-frame' style='width:400px;height:250px' src='%1'></iframe>").arg(childUrl).toUtf8() + marker : control;
    QVERIFY(command("open " + rootUrl));
    QTRY_COMPARE(page("location.href").toString(),rootUrl);
    if (foreign) QTRY_VERIFY(page("!!document.getElementById('p00-frame')").toBool());
    QVERIFY(command("set --temp hints.auto_follow=never"));
    QTRY_COMPARE(evaluate("JSON.parse(testWindow.browserUi.config_json).hints.auto_follow").toString(),QString("never"));
    QVERIFY(command("bind f hint all"));
    auto *workspace = evaluate("testWindow.hintWorkspace").value<QObject *>();
    auto *ui = evaluate("testWindow.browserUi").value<QObject *>();
    auto *view = evaluate("testWindow.activeWebView()").value<QObject *>();
    auto *repeater = window->findChild<QQuickItem *>("hintMarkerRepeater");
    auto *provider = evaluate("testWindow.hintFrameProvider").value<QObject *>();
    QVERIFY(workspace && ui && view && repeater && provider);
    QString previousSession, expectedElementId;
    int previousEpoch = -1;
    RenderedStateObserver rendered(window,[&] {
        if (!ui->property("hint_visible").toBool() || ui->property("mode").toString() != "hint"
            || workspace->property("hintRefreshPending").toBool()
            || workspace->property("hintTrackingView").value<QObject *>() != view
            || workspace->property("hintCollectionEpoch").toInt() <= previousEpoch
            || workspace->property("hintPresentationSessionId").toString().isEmpty()
            || workspace->property("hintPresentationSessionId").toString() == previousSession
            || repeater->property("count").toInt() < 1 || !repeater->parentItem() || !repeater->parentItem()->isVisible()) return false;
        const auto id=p00RenderedButtonId(engine.get(),ui,repeater,"Lifetime control",expectedElementId,foreign);
        if (id.isEmpty()) return false;
        expectedElementId=id;
        return true;
    });
    const QString stateScript=R"JS((()=>{const s=window.__ferric_browserHintState,map=!!s&&s.elements instanceof Map,observers=!!s&&Array.isArray(s.observers),windows=!!s&&Array.isArray(s.windows);const valid=map&&observers&&windows&&typeof s.tracking==='boolean'&&(s.scopeLease===null||typeof s.scopeLease==='object');return {url:location.href,document:document.documentElement.dataset.p00LifetimeDocument,present:!!s,shape_valid:!!valid,elements:map?s.elements.size:null,observers:observers?s.observers.length:null,windows:windows?s.windows.length:null,scope:valid?!!s.scopeLease:null,tracking:valid?s.tracking:null,target_ids:map?Array.from(s.elements).filter(([id,r])=>r&&r.element&&r.element.id==='p00-life').map(([id])=>String(id)):null,heap_available:!!performance.memory,heap:performance.memory?{used:performance.memory.usedJSHeapSize,total:performance.memory.totalJSHeapSize,limit:performance.memory.jsHeapSizeLimit}:null};})())JS";
    struct ObservedFrame { QWebEngineFrame frame; QString url, document; QVariantMap before; };
    QList<ObservedFrame> frames;
    const auto readFrame = [](QWebEngineFrame frame,const QString &script) -> QVariantMap {
        const auto reply=std::make_shared<std::optional<QVariant>>();
        frame.runJavaScript(script,1,[reply](const QVariant &value){*reply=value;});
        if (!QTest::qWaitFor([reply]{return reply->has_value();},5000)) return {};
        return reply->value().toMap();
    };
    const auto rememberFrames = [&] {
        frames.clear();
        QList<QWebEngineFrame> reached;
        std::function<void(QWebEngineFrame,int)> visit = [&](QWebEngineFrame frame,int depth) {
            if (!frame.isValid() || depth > 2) return;
            reached.append(frame);
            for (const auto &child : frame.children()) visit(child,depth+1);
        };
        visit(view->property("mainFrame").value<QWebEngineFrame>(),0);
        if (reached.size() != (foreign ? 2 : 1)) return false;
        QSet<QString> documents;
        for (auto frame : reached) {
            const auto identity=readFrame(frame,stateScript);
            const auto url=identity.value("url").toString(), document=identity.value("document").toString();
            if ((url!=rootUrl && (!foreign || url!=childUrl)) || document.isEmpty() || documents.contains(document)) return false;
            if (!identity.value("present").toBool() || !identity.value("shape_valid").toBool()
                || !identity.value("tracking").toBool() || identity.value("observers").toInt()<1 || identity.value("windows").toInt()<1) return false;
            const auto ids=identity.value("target_ids").toStringList();
            const bool targetRealm=!foreign || url==childUrl;
            if (targetRealm ? ids!=QStringList{expectedElementId} : !ids.isEmpty()) return false;
            documents.insert(document);
            frames.append({frame,url,document,identity});
        }
        return true;
    };

    const auto observe = [&](int cycle) {
        QJsonArray realms;
        bool clean = true;
        for (auto observed : frames) {
            auto frame=observed.frame;
            if (!frame.isValid()) { realms.append(QJsonObject{{"available",false}}); clean=false; continue; }
            const auto state=readFrame(frame,stateScript);
            const bool current=state.value("url").toString()==observed.url
                && state.value("document").toString()==observed.document;
            const bool empty = current && state.value("present").toBool() && state.value("shape_valid").toBool() && state.contains("elements") && state.value("elements").toInt()==0
                && state.value("observers").toInt()==0 && state.value("windows").toInt()==0
                && !state.value("scope").toBool() && !state.value("tracking").toBool();
            auto row = QJsonObject::fromVariantMap(state);
            row.insert("available",current && state.value("shape_valid").toBool()); row.insert("clean",empty);
            row.insert("before_close",QJsonObject::fromVariantMap(observed.before));
            realms.append(row); clean = clean && empty;
        }
        const auto owned = evaluate("(()=>{const w=testWindow.hintWorkspace,p=w.hintFrameProvider;return {leases:p.leaseCount,sources:Object.keys(p.sources).length,poll_pending:p.pollPending,provider_view:!!p.view,root_owner:!!p.rootOwner,scoped_owner:!!p.scopedOwner,tracking_view:!!w.hintTrackingView,pending_inputs:w.pendingHintInputs.length,pending_view:!!w.pendingHintInputView,refresh_pending:w.hintRefreshPending,hint_visible:testWindow.browserUi.hint_visible,mode:testWindow.browserUi.mode};})()").toMap();
        const bool released = owned.contains("leases") && owned.value("leases").toInt()==0
            && owned.value("sources").toInt()==0 && owned.value("pending_inputs").toInt()==0
            && !owned.value("poll_pending").toBool() && !owned.value("provider_view").toBool()
            && !owned.value("root_owner").toBool() && !owned.value("scoped_owner").toBool()
            && !owned.value("tracking_view").toBool() && !owned.value("pending_view").toBool()
            && !owned.value("refresh_pending").toBool() && !owned.value("hint_visible").toBool()
            && owned.value("mode").toString()=="normal";
        snapshots.append(QJsonObject{{"cycle",cycle},{"monotonic_ns",QString::number(monotonicNsecs())},{"realms",realms},{"owned",QJsonObject::fromVariantMap(owned)},
            {"qt_descendants",window->findChildren<QObject *>().size()},
            {"qt_windows",QGuiApplication::allWindows().size()},{"process_resources",p00ProcessResources()},
            {"cleanup_clean",clean && released}});
        qInfo().noquote()<<"FERRIC_P00_LIFETIME_CHECKPOINT_JSON "+QJsonDocument(snapshots.last().toObject()).toJson(QJsonDocument::Compact);
        frames.clear(); // Do not retain frame observation handles across cycles.
        return clean && released;
    };
    window->requestActivate(); QTRY_VERIFY(window->isActive());
    evaluate("testWindow.activeWebView().forceActiveFocus()");
    for (int index=-warmups; index<cycles; ++index) {
        failedCycle=index; stage="establish";
        if (foreign) {
            const auto reply = std::make_shared<std::optional<QVariant>>();
            // Same main-world focus operation, through the native frame API;
            // no per-cycle QML expression compilation in the observer.
            view->property("mainFrame").value<QWebEngineFrame>().runJavaScript(
                "document.getElementById('p00-frame').focus();true", 0,
                [reply](const QVariant &value) { *reply = value; });
            QElapsedTimer deadline; deadline.start();
            while (!reply->has_value() && deadline.elapsed() < 5000) QTest::qWait(10);
            QVERIFY(reply->has_value());
            QVERIFY(reply->value().toBool());
        }
        previousEpoch=workspace->property("hintCollectionEpoch").toInt();
        const auto elapsed=rendered.measure([&]{gesture(window,Qt::Key_F,"f");},5000);
        QVERIFY2(elapsed>=0,"An established current Hint session/render is required; cancellation-only cycles are insufficient");
        previousSession=workspace->property("hintPresentationSessionId").toString();
        const bool checkpoint=index==-1 || index>=0 && ((index+1)%100==0 || index+1==cycles);
        if (checkpoint) QVERIFY(rememberFrames());
        stage="production-close";
        key(Qt::Key_Escape);
        QTRY_COMPARE(provider->property("leaseCount").toInt(),0);
        QCoreApplication::sendPostedEvents(nullptr,QEvent::DeferredDelete);
        if (index>=0) completed=index+1; else ++completedWarmups;
        if (checkpoint) {
            stage="read-only-post-close-observation";
            QVERIFY2(observe(index<0?0:index+1),"Production close retained renderer or browser Hint state; raw checkpoint retained");
        }
    }
    stage="complete"; failures=0;
}

// Additional P00 measurement-only candidate; not compiled/run/integrated.
#include <QQmlContext>
// UI close decision follows the pinned tst_page_windows.cpp:33-54 helper.
namespace {
QQuickItem *p00VisibleItem(QQuickItem *parent,const QString &name) {
    if (!parent) return nullptr;
    if (parent->isVisible() && parent->objectName()==name) return parent;
    for (auto *child:parent->childItems()) if (auto *found=p00VisibleItem(child,name)) return found;
    return nullptr;
}
bool p00ProceedClose(QQuickWindow *host, bool requireClick = false) {
    QPointer<QQuickWindow> guarded(host);
    if (!guarded || !guarded->isVisible()) return !requireClick;
    QSignalSpy frame(host,&QQuickWindow::frameSwapped);
    host->update();
    if (!QTest::qWaitFor([&]{return !guarded || !guarded->isVisible() || !frame.isEmpty();},5000)) return false;
    if (!guarded || !guarded->isVisible()) return !requireClick;
    QPointer<QQuickItem> button;
    if (!QTest::qWaitFor([&]{
        if (!guarded || !guarded->isVisible()) return true;
        button=p00VisibleItem(guarded->contentItem(),"shutdownProceedButton");
        return !button.isNull();
    },5000)) return false;
    if (!guarded || !guarded->isVisible()) return !requireClick;
    if (!button || !button->isEnabled()) return false;
    QTest::mouseClick(guarded,Qt::LeftButton,Qt::NoModifier,button->mapToScene(button->boundingRect().center()).toPoint());
    return true;
}
}
void ApplicationTests::sitePolicyWindowLifetime_data() {
    QTest::addColumn<bool>("privateWindow");
    QTest::newRow("shared-profile-window") << false;
    QTest::newRow("private-profile-window") << true;
}
void ApplicationTests::sitePolicyWindowLifetime() {
    const bool smoke=qEnvironmentVariableIsSet("FERRIC_SITE_POLICY_PERF_SMOKE");
    if (!smoke && !qEnvironmentVariableIsSet("FERRIC_P00_LIFETIME"))
        QSKIP("Opt-in original shared/private profile window/view teardown observations");
    QFETCH(bool,privateWindow);
    const int warmups=smoke?1:10, cycles=smoke?2:100;
    int completed=0,completedWarmups=0,failedCycle=-1,failures=1;
    QJsonArray snapshots,outcomes;
    QString stage="setup";
    const auto report=qScopeGuard([&]{
        qInfo().noquote()<<"FERRIC_P00_LIFETIME_JSON "+QJsonDocument(QJsonObject{
            {"journey",privateWindow?"private-profile-window-lifetime":"shared-profile-window-lifetime"},
            {"run_kind",smoke?"functional-smoke-not-baseline":"lifecycle-baseline"},
            {"warmups",warmups},{"requested_cycles",cycles},{"completed_cycles",completed},
            {"failures",failures},{"failed_cycle",failures?failedCycle:-1},{"completed_warmups",completedWarmups},{"stage",stage},{"snapshots",snapshots},{"outcomes",outcomes},
            {"runtime",runtimeFingerprint()},{"qpa",QGuiApplication::platformName()},{"dpr",window->devicePixelRatio()},
            {"scope","Real secondary host/view and shared or distinct private profile; public open, established Hint, native window close and rendered prompt acceptance. Released/hidden hosts are distinct from destroyed hosts. No named durable profile switching or capture/draft claim."}
        }).toJson(QJsonDocument::Compact);
    });
    fixtureHtml="<!doctype html><button style='width:180px;height:35px'>Window lifetime control</button>";
    const auto url=QString("http://127.0.0.1:%1/p00-window-lifetime").arg(server.serverPort());
    QVERIFY(command("open "+url));
    QTRY_COMPARE(page("location.href").toString(),url);
    QVERIFY(command("set --temp hints.auto_follow=never"));
    QTRY_COMPARE(evaluate("JSON.parse(testWindow.browserUi.config_json).hints.auto_follow").toString(),QString("never"));
    // Compile fixed observers once; each call resolves the current child.
    // No child QObject, result, or window entry is retained by this helper.
    const auto helperValue = evaluate(R"JS(({
        command:function(text){return testWindow.browserUi.execute_command(text)},
        complete:function(){testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)},
        liveCount:function(){return testWindow.liveWindowEntries().length},
        host:function(){return testWindow.liveWindowEntries().find(e=>e.host!==testWindow).host},
        ready:function(){return p00LifetimeHost.profileBootstrapComplete && !!p00LifetimeHost.activeView},
        view:function(){return p00LifetimeHost.activeView},
        group:function(){return testWindow.profileResourceRegistry.forProfile(p00LifetimeHost.activeView.profile)},
        shared:function(){return p00LifetimeHost.activeView.profile===testWindow.browserProfile},
        childCommand:function(text){return p00LifetimeHost.windowUi.execute_command(text)},
        ui:function(){return p00LifetimeHost.windowUi},
        workspace:function(){return p00LifetimeHost.hintWorkspace},
        confirmation:function(){return p00LifetimeHost.windowCloseConfirmationVisible},
        pagePrompt:function(){return p00LifetimeHost.windowShutdownPagePromptVisible},
        ledger:function(){return testWindow.profileResourceRegistry.groups.map(g=>({transient:g.transientProfile,views:g.lifetime.viewCount,windows:g.lifetime.windowCount,popup_leases:g.lifetime.popupOpenerLeaseCount()}))},
        page:function(script,world){p00LifetimeHost.activeView.runJavaScript(script,world,function(value){testProbe.accept(value)})}
    }))JS");
    auto helper = helperValue.value<QJSValue>();
    if (!helper.isObject()) helper = engine->toScriptValue(helperValue);
    QVERIFY(helper.isObject());
    const auto invoke = [&](const char *name, const QJSValueList &arguments = QJSValueList{}) {
        auto function = helper.property(QString::fromLatin1(name));
        if (!function.isCallable()) qFatal("Missing window lifetime helper %s", name);
        auto value = function.call(arguments);
        if (value.isError()) qFatal("Window lifetime helper %s: %s", name, qPrintable(value.toString()));
        return value;
    };
    const auto observePage = [&](const QString &script) {
        scriptDone = false;
        invoke("page", {QJSValue(script), QJSValue(0)});
        QElapsedTimer deadline; deadline.start();
        while (!scriptDone && deadline.elapsed() < 5000) QTest::qWait(10);
        if (!scriptDone) qFatal("Window lifetime page observation timed out");
        return scriptResult;
    };
    QTRY_COMPARE(invoke("liveCount").toInt(),1);
    const auto baselineLedger=invoke("ledger").toVariant().toList();
    QVERIFY(!baselineLedger.isEmpty());
    const auto capture=[&](int cycle, const QElapsedTimer &releaseDeadline){
        auto resources = p00ProcessResources();
        int resourceAttempts = 1;
        const auto complete = [&] { return resources.value("complete").toBool() && resources.value("pss_complete").toBool(); };
        // Renderer exit/reaping is asynchronous. Observe it within the same
        // five-second release budget; never omit an inaccessible process.
        while (!complete()) {
            const auto remaining = 5000 - releaseDeadline.elapsed();
            if (remaining <= 0) break;
            QTest::qWait(int(std::min<qint64>(10, remaining)));
            if (releaseDeadline.elapsed() >= 5000) break;
            auto nextResources = p00ProcessResources();
            ++resourceAttempts;
            if (releaseDeadline.elapsed() > 5000) break;
            resources = std::move(nextResources);
        }
        const auto observationMs = releaseDeadline.elapsed();
        const bool settled = complete() && observationMs <= 5000;
        snapshots.append(QJsonObject{{"cycle",cycle},{"monotonic_ns",QString::number(monotonicNsecs())},{"ledger",QJsonArray::fromVariantList(invoke("ledger").toVariant().toList())},
            {"live_windows",invoke("liveCount").toInt()},
            {"qt_windows",QGuiApplication::allWindows().size()},
            {"qt_descendants",window->findChildren<QObject *>().size()},
            {"release_observation_ms",int(observationMs)},{"resource_snapshot_attempts",resourceAttempts},
            {"resources_settled_within_release_budget",settled},
            {"process_resources",resources}});
        qInfo().noquote()<<"FERRIC_P00_LIFETIME_CHECKPOINT_JSON "+QJsonDocument(snapshots.last().toObject()).toJson(QJsonDocument::Compact);
        return settled;
    };
    const auto clearContext=qScopeGuard([&]{
        pageReceiver="testWindow.activeWebView()";
        engine->rootContext()->setContextProperty("p00LifetimeHost",static_cast<QObject *>(nullptr));
    });
    for (int index=-warmups;index<cycles;++index) {
        failedCycle=index; stage="public-open";
        QVERIFY(invoke("command", {QJSValue(QString("open --target %1 %2").arg(privateWindow?"private-window":"window",url))}).toBool());
        invoke("complete");
        QTRY_COMPARE(invoke("liveCount").toInt(),2);
        QPointer<QQuickWindow> host(qobject_cast<QQuickWindow *>(invoke("host").toQObject()));
        QVERIFY(host);
        engine->rootContext()->setContextProperty("p00LifetimeHost",host.data());
        QTRY_VERIFY(invoke("ready").toBool());
        QPointer<QObject> view(invoke("view").toQObject());
        QPointer<QObject> group(invoke("group").toQObject());
        QVERIFY(view && group);
        QCOMPARE(invoke("shared").toBool(),!privateWindow);
        pageReceiver="p00LifetimeHost.activeView";
        QTRY_COMPARE(observePage("location.href").toString(),url);
        QTRY_VERIFY(host->isVisible());
        QVERIFY(invoke("childCommand", {QJSValue(QStringLiteral("set --temp hints.auto_follow=never"))}).toBool());
        stage="child-established-hint";
        {
            auto *ui=invoke("ui").toQObject();
            auto *workspace=invoke("workspace").toQObject();
            auto *repeater=workspace ? workspace->findChild<QQuickItem *>("hintMarkerRepeater") : nullptr;
            QVERIFY(ui && workspace && repeater);
            QTRY_COMPARE(QJsonDocument::fromJson(ui->property("config_json").toString().toUtf8()).object()
                .value("hints").toObject().value("auto_follow").toString(),QString("never"));
            RenderedStateObserver rendered(host,[&]{
                return host && view && ui->property("mode").toString()=="hint" && ui->property("hint_visible").toBool()
                    && !workspace->property("hintRefreshPending").toBool()
                    && workspace->property("hintTrackingView").value<QObject *>()==view.data()
                    && !workspace->property("hintPresentationSessionId").toString().isEmpty()
                    && repeater->parentItem() && repeater->parentItem()->isVisible()
                    && repeater->property("count").toInt()==1
                    && !p00RenderedButtonId(engine.get(),ui,repeater,"Window lifetime control",{},false).isEmpty();
            });
            bool accepted=false;
            const auto elapsed=rendered.measure([&]{accepted=invoke("childCommand", {QJSValue(QStringLiteral("hint all"))}).toBool();},5000);
            if (!accepted || elapsed < 0) qInfo().noquote() << "FERRIC_P15_WINDOW_FAILURE_JSON " + QJsonDocument(QJsonObject{
                {"accepted",accepted},{"elapsed",elapsed},{"mode",ui->property("mode").toString()},
                {"status",ui->property("status_text").toString()},{"visible",ui->property("hint_visible").toBool()},
                {"markers",repeater->property("count").toInt()},{"hints",ui->property("hint_values").toString()},
                {"refresh",workspace->property("hintRefreshPending").toBool()},
                {"tracking_view_matches",workspace->property("hintTrackingView").value<QObject *>()==view.data()},
                {"session",workspace->property("hintPresentationSessionId").toString()}}).toJson(QJsonDocument::Compact);
            QVERIFY(accepted && elapsed>=0);
        } // Observer no longer captures child objects when retirement begins.
        stage="native-window-close";
        // The native close event enters the regular secondary-window shutdown
        // while the Hint session is live. tab-close requires Normal/Command.
        QVERIFY(host);
        host->close();
        QTRY_VERIFY(host && host->isVisible() && invoke("confirmation").toBool());
        stage="initial-rendered-close-confirmation";
        QVERIFY(p00ProceedClose(host,true));
        QTRY_VERIFY(!host || !host->isVisible() || !invoke("confirmation").toBool());
        stage="secondary-close-decision";
        QTRY_VERIFY(!host || !host->isVisible() || invoke("pagePrompt").toBool());
        if (host && host->isVisible()) QVERIFY(p00ProceedClose(host));
        QElapsedTimer releaseDeadline; releaseDeadline.start();
        const bool released=QTest::qWaitFor([&]{
            return host.isNull() && view.isNull()
                && invoke("liveCount").toInt()==1
                && invoke("ledger").toVariant().toList()==baselineLedger;
        },5000);
        const auto observedLedger=invoke("ledger").toVariant().toList();
        const QJsonObject outcome{{"index",index},{"warmup",index<0},
            {"initial_rendered_close_confirmed",true},
            {"view_destroyed",view.isNull()},{"host_destroyed",host.isNull()},
            {"host_hidden",host && !host->isVisible()},{"group_destroyed",group.isNull()},
            {"live_windows",invoke("liveCount").toInt()},
            {"observed_ledger",QJsonArray::fromVariantList(observedLedger)},
            {"ledger_returned_to_baseline",observedLedger==baselineLedger},{"released",released}};
        outcomes.append(outcome);
        qInfo().noquote()<<"FERRIC_P00_LIFETIME_WINDOW_CYCLE_JSON "+QJsonDocument(outcome).toJson(QJsonDocument::Compact);
        if (!released) capture(index<0?0:index+1, releaseDeadline);
        QVERIFY2(released,"Window/view/native profile ledger did not return to warmed baseline; observed state retained");
        if (index>=0) completed=index+1; else ++completedWarmups;
        // Qt/QML references are cleared after measuring release, never by
        // manually deleting a surviving host or its private profile.
        engine->rootContext()->setContextProperty("p00LifetimeHost",static_cast<QObject *>(nullptr));
        pageReceiver="testWindow.activeWebView()";
        QCoreApplication::sendPostedEvents(nullptr,QEvent::DeferredDelete);
        if (index==-1 || index>=0 && ((index+1)%10==0 || index+1==cycles))
            QVERIFY2(capture(index<0?0:index+1, releaseDeadline), "Process resources did not settle within the window release budget");
    }
    stage="complete";failures=0;
}


void ApplicationTests::hintCollectionOptimisationPerformance() {
    if(!qEnvironmentVariableIsSet("FERRIC_OPT_PERF"))QSKIP("Opt-in current-source optimisation benchmark");
    const int cards=qEnvironmentVariableIntValue("FERRIC_OPT_CARDS");
    const int scroll=qEnvironmentVariableIntValue("FERRIC_OPT_SCROLL");
    const int warmups=qEnvironmentVariableIsSet("FERRIC_OPT_SMOKE")?1:5;
    const int samples=qEnvironmentVariableIsSet("FERRIC_OPT_SMOKE")?2:qEnvironmentVariableIsSet("FERRIC_OPT_SAMPLES")?qEnvironmentVariableIntValue("FERRIC_OPT_SAMPLES"):20;
    QVERIFY(samples>0&&samples<=100);
    QVERIFY(cards==8||cards==30||cards==100||cards==300);
    window->resize(640,480);
    fixtureHtml="<!doctype html><meta charset=utf-8><style>body{margin:16px;font:16px sans-serif}a{display:block;height:32px}.card{height:96px;overflow:hidden}.card span{font-size:12px}</style><nav><button id=home>Home</button><button>Menu</button><button>Search</button></nav><main id=feed>";
    for(int i=0;i<cards;++i) {
        fixtureHtml+=QString("<article class=card><a class=title href='#article-%1'>Article %1: keyboard browsing</a>").arg(i).toUtf8();
        for(int j=0;j<20;++j)fixtureHtml+=QString("<span> detail %1</span>").arg(j).toUtf8();
        fixtureHtml+="</article>";
    }
    fixtureHtml+="</main><script>document.body.dataset.ready='true'</script>";
    if(cards==8){
        fixtureHtml="<!doctype html><style>body{margin:16px;font:16px sans-serif}a{display:block;height:32px}</style>";
        for(int i=0;i<8;++i)fixtureHtml+=QString("<a href='#article-%1'>Article %1</a>").arg(i).toUtf8();
        fixtureHtml+="<script>document.body.dataset.ready='true'</script>";
    }
    const auto url=QString("http://127.0.0.1:%1/optimisation-feed").arg(server.serverPort());
    QVERIFY(command("open "+url));
    QTRY_VERIFY_WITH_TIMEOUT(page("location.href==="+quoted(url)+"&&document.body.dataset.ready==='true'").toBool(),15000);
    QVERIFY(page(QString("scrollTo(0,%1);true").arg(scroll)).toBool());
    QTest::qWait(250);
    window->requestActivate();QTRY_VERIFY(window->isActive());
    evaluate("testWindow.activeWebView().forceActiveFocus()");
    auto *ui=evaluate("testWindow.browserUi").value<QObject *>();QVERIFY(ui);
    auto *repeater=window->findChild<QQuickItem *>("hintMarkerRepeater");QVERIFY(repeater);
    auto *overlay=repeater->parentItem();QVERIFY(overlay);
    RenderedStateObserver rendered(window,[&]{
        if(ui->property("mode").toString()!="hint"||!overlay->isVisible())return false;
        const int count=repeater->property("count").toInt();if(count<1)return false;
        for(int i=0;i<count;++i){QQuickItem *item=nullptr;
            if(!QMetaObject::invokeMethod(repeater,"itemAt",Qt::DirectConnection,Q_RETURN_ARG(QQuickItem *,item),Q_ARG(int,i))
                ||!item||!item->isVisible()||item->opacity()<=0||item->width()<=0||item->height()<=0)return false;
            bool label=false;
            for(auto *child:item->childItems())if(child->isVisible()&&child->width()>0&&child->height()>0&&!child->property("text").toString().isEmpty()){label=true;break;}
            if(!label)return false;
        }
        return !evaluate("testWindow.hintWorkspace.hintRefreshPending").toBool();
    });
    RenderedStateObserver normal(window,[&]{return ui->property("mode").toString()=="normal"&&!overlay->isVisible();});
    for(int i=0;i<warmups+samples;++i){
        const auto gate=qEnvironmentVariable("FERRIC_OPT_GATE");
        if(!gate.isEmpty())QVERIFY2(QTest::qWaitFor([&]{return QFile::exists(gate+"/go-"+QString::number(i));},120000),"Benchmark coordinator did not grant sample");
        QVERIFY(normal.measure([&]{gesture(window,Qt::Key_Escape);window->update();})>=0);
        const double ms=rendered.measure([&]{gesture(window,Qt::Key_F,"f");},5000);
        QVERIFY2(ms>=0,qPrintable(ui->property("status_text").toString()));
        QJsonArray projection;
        for(const auto &v:QJsonDocument::fromJson(ui->property("hint_values").toString().toUtf8()).array()){
            const auto h=v.toObject();projection.append(QJsonArray{QUrl(h["href"].toString()).fragment(),h["text"],h["kind"],h["label"],h["frame_path"],h["x"],h["y"],h["width"],h["height"]});
        }
        qInfo().noquote()<<"FERRIC_OPT_SAMPLE "+QJsonDocument(QJsonObject{
            {"case",qEnvironmentVariable("FERRIC_OPT_CASE","full")},{"cards",cards},{"scroll",scroll},{"iteration",i},{"warmup",i<warmups},
            {"ms",ms},{"projection",projection},{"observer_ms",rendered.predicateMillis()},
            {"viewport",QJsonObject::fromVariantMap(page("({width:innerWidth,height:innerHeight,scrollY})").toMap())}
        }).toJson(QJsonDocument::Compact);
    }
    QVERIFY(normal.measure([&]{gesture(window,Qt::Key_Escape);})>=0);
}
