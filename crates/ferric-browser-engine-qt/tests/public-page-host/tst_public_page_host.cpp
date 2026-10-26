#include <QApplication>
#include <QFile>
#include <QFileDialog>
#include <QHostAddress>
#include <QPointer>
#include <QQuickWidget>
#include <QQuickItem>
#include <QSignalSpy>
#include <QTcpServer>
#include <QTcpSocket>
#include <QTest>
#include <QUrl>
#include <QVBoxLayout>
#include <QWebEngineNewWindowRequest>
#include <QWebEnginePage>
#include <QWebEngineProfile>
#include <QWebEngineSettings>
#include <QWebEngineView>
#include <QWidget>
#include <QWindow>
#include <QDir>
#include <QTemporaryDir>
#include <QElapsedTimer>
#include <functional>

namespace {
class Fixture final : public QObject {
    Q_OBJECT
public:
    bool start() {
        connect(&server, &QTcpServer::newConnection, this, [this] {
            while (auto *s = server.nextPendingConnection()) {
                connect(s, &QTcpSocket::readyRead, this, [this, s] {
                    const QByteArray req = s->readAll();
                    const QByteArray path = req.split(' ').value(1).split('?').value(0);
                    if (path == "/sentinel") ++sentinelRequests;
                    ++requests;
                    if (path == "/fail") {
                        s->abort();
                        return;
                    }
                    QByteArray body;
                    int status = 200;
                    if (path == "/popup.html") {
                        QFile f(QStringLiteral(FIXTURE_DIR) + "/popup.html");
                        if (!f.open(QIODevice::ReadOnly)) status = 500;
                        else body = f.readAll();
                    } else if (path == "/dialog.html") {
                        body = "<!doctype html><title>dialog</title><p>dialog child</p>";
                    } else if (path != "/sentinel") {
                        status = 404;
                    }
                    const QByteArray phrase = status == 200 ? "OK" : (status == 500 ? "Failure" : "Not Found");
                    s->write("HTTP/1.1 " + QByteArray::number(status) + " " + phrase + "\r\n");
                    s->write("Content-Type: text/html; charset=utf-8\r\n");
                    s->write("Cache-Control: no-store\r\nConnection: close\r\n");
                    s->write("Content-Length: " + QByteArray::number(body.size()) + "\r\n\r\n" + body);
                    s->disconnectFromHost();
                });
            }
        });
        return server.listen(QHostAddress::LocalHost, 0);
    }
    QUrl url(const QString &path) const {
        return QUrl(QStringLiteral("http://127.0.0.1:%1%2").arg(server.serverPort()).arg(path));
    }
    QTcpServer server;
    int requests = 0;
    int sentinelRequests = 0;
};

static QVariant js(QWebEnginePage *page, const QString &script, int timeoutMs = 6000) {
    QVariant result;
    bool done = false;
    page->runJavaScript(script, [&](const QVariant &v) { result = v; done = true; });
    QElapsedTimer timer;
    timer.start();
    while (!done && timer.elapsed() < timeoutMs) {
        QCoreApplication::processEvents(QEventLoop::AllEvents, 20);
        QTest::qWait(5);
    }
    return done ? result : QVariant();
}

static bool waitUntil(const std::function<bool()> &predicate, int timeoutMs = 8000) {
    QElapsedTimer timer;
    timer.start();
    while (!predicate() && timer.elapsed() < timeoutMs) {
        QCoreApplication::processEvents(QEventLoop::AllEvents, 20);
        QTest::qWait(10);
    }
    return predicate();
}

struct Host {
    QPointer<QWidget> shell;
    QPointer<QQuickWidget> chrome;
    QPointer<QWebEngineView> view;
    QPointer<QWebEnginePage> page;
    QPointer<QWebEngineProfile> profile;
};

class Harness final : public QObject {
    Q_OBJECT
public:
    explicit Harness(Fixture *f) : fixture(f) {}
    QWebEngineProfile *newProfile(bool offTheRecord, const QString &name) {
        auto *p = offTheRecord ? new QWebEngineProfile(this) : new QWebEngineProfile(name, this);
        p->settings()->setAttribute(QWebEngineSettings::JavascriptCanOpenWindows, true);
        if (!offTheRecord) {
            auto *dir = new QTemporaryDir;
            dirs.push_back(dir);
            p->setPersistentStoragePath(dir->path() + "/storage");
            p->setCachePath(dir->path() + "/cache");
            p->setPersistentCookiesPolicy(QWebEngineProfile::ForcePersistentCookies);
        }
        profiles.push_back(p);
        return p;
    }
    Host *makeHost(QWebEngineProfile *profile) {
        auto *h = new Host;
        h->profile = profile;
        // Each page shell is an independent top-level owner. Parenting a popup shell to
        // its opener would destroy the surviving page when the opener closes.
        h->shell = new QWidget;
        h->shell->setWindowTitle("public-page-host-fixture");
        auto *layout = new QVBoxLayout(h->shell);
        h->chrome = new QQuickWidget(h->shell);
        h->chrome->setResizeMode(QQuickWidget::SizeRootObjectToView);
        h->chrome->setSource(QUrl::fromLocalFile(QStringLiteral(CHROME_QML)));
        layout->addWidget(h->chrome);
        h->view = new QWebEngineView(h->shell);
        h->page = new QWebEnginePage(profile, h->view);
        h->view->setPage(h->page);
        layout->addWidget(h->view);
        hosts.push_back(h);
        connect(h->page, &QWebEnginePage::windowCloseRequested, this, [this, h] { closeHost(h); });
        connect(h->page, &QWebEnginePage::newWindowRequested, this,
                [this, h](QWebEngineNewWindowRequest &request) { adopt(h, request); });
        return h;
    }
    void adopt(Host *source, QWebEngineNewWindowRequest &request) {
        ++popupRequests;
        const bool selectedPostOpenFailure = failAfterOpen && request.requestedUrl().path() == "/fail";
        if (failBeforeOpen || (crossProfile && source->profile != alternateProfile)) {
            failBeforeOpen = false;
            ++rejected;
            return;
        }
        Host *dest = makeHost(source->profile);
        dest->page->settings()->setAttribute(QWebEngineSettings::JavascriptCanOpenWindows, true);
        request.openIn(dest->page);
        ++adopted;
        if (selectedPostOpenFailure) {
            failAfterOpen = false;
            connect(dest->page, &QWebEnginePage::loadFinished, this, [this, dest](bool ok) {
                if (!ok) { ++postOpenFailures; closeHost(dest); }
            });
        }
        dest->shell->resize(800, 600);
        dest->shell->show();
    }
    void closeHost(Host *h) {
        if (!h || !h->shell) return;
        if (h->page) h->page->stop();
        h->shell->close();
        h->shell->deleteLater();
        ++closed;
    }
    int alivePages(QWebEngineProfile *p = nullptr) const {
        int n = 0;
        for (const Host *h : hosts) if (h && h->page && (!p || h->profile == p)) ++n;
        return n;
    }
    bool drain(QWebEngineProfile *p) {
        if (alivePages(p) != 0) return false;
        if (!p) return false;
        profiles.removeOne(p);
        p->deleteLater();
        QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        return true;
    }
    Fixture *fixture;
    QList<Host *> hosts;
    QList<QWebEngineProfile *> profiles;
    QList<QTemporaryDir *> dirs;
    QWebEngineProfile *alternateProfile = nullptr;
    bool failBeforeOpen = false;
    bool failAfterOpen = false;
    bool crossProfile = false;
    int popupRequests = 0, adopted = 0, rejected = 0, postOpenFailures = 0, closed = 0;
};

class PublicPageHostTests final : public QObject {
    Q_OBJECT
private slots:
    void initTestCase() {
        QVERIFY(fixture.start());
        QVERIFY(QWebEngineProfile::defaultProfile());
    }
    void lifecycle() {
        Harness h(&fixture);
        auto *profile = h.newProfile(false, "lifecycle");
        Host *top = h.makeHost(profile);
        top->page->settings()->setAttribute(QWebEngineSettings::JavascriptCanOpenWindows, true);
        top->page->load(fixture.url("/popup.html?kind=top"));
        QVERIFY(waitUntil([&] { return js(top->page, "document.body.dataset.kind").toString() == "top"; }));
        const QString topNonce = js(top->page, "fixtureNonce").toString();
        js(top->page, "document.querySelector('#open').click()");
        QVERIFY(waitUntil([&] { return h.alivePages(profile) == 2; }));
        Host *child = h.hosts.last();
        QVERIFY(waitUntil([&] { return js(child->page, "document.body.dataset.kind").toString() == "child"; }));
        const QString childNonce = js(child->page, "fixtureNonce").toString();
        js(child->page, "window.savedOpener=window.opener; document.querySelector('#open').click()");
        QVERIFY(waitUntil([&] { return h.alivePages(profile) == 3; }));
        Host *grand = h.hosts.last();
        QVERIFY(waitUntil([&] { return js(grand->page, "document.body.dataset.kind").toString() == "grandchild"; }));
        const QString grandNonce = js(grand->page, "fixtureNonce").toString();
        QVERIFY(!topNonce.isEmpty() && !childNonce.isEmpty() && !grandNonce.isEmpty());
        QCOMPARE(js(grand->page, "window.opener.fixtureNonce").toString(), childNonce);
        QCOMPARE(js(child->page, "window.opener.fixtureNonce").toString(), topNonce);

        top->page->triggerAction(QWebEnginePage::RequestClose);
        QVERIFY(waitUntil([&] { return top->page.isNull(); }));
        QCOMPARE(js(grand->page, "fixtureNonce").toString(), grandNonce);
        QCOMPARE(js(child->page, "window.opener === null").toBool(), true);
        QCOMPARE(js(child->page, "window.savedOpener.closed").toBool(), true);
        js(child->page, "window.savedOpener.location.replace('/sentinel?token=unique')");
        QTest::qWait(400);
        QCOMPARE(fixture.sentinelRequests, 0);

        const QVariantMap target = js(grand->page,
            "(()=>{const r=document.querySelector('#target').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()").toMap();
        QVERIFY(!target.isEmpty());
        QTest::mouseClick(grand->view, Qt::LeftButton, Qt::NoModifier,
                          QPoint(target.value("x").toInt(), target.value("y").toInt()));
        QCOMPARE(js(grand->page, "pointerCount").toInt(), 1);
        js(grand->page, "document.querySelector('#target').focus()");
        QTest::keyClick(grand->view, Qt::Key_A);
        QVERIFY(waitUntil([&] { return js(grand->page, "keyCount").toInt() > 0; }));

        child->page->triggerAction(QWebEnginePage::RequestClose);
        QVERIFY(waitUntil([&] { return child->page.isNull(); }));
        QCOMPARE(js(grand->page, "fixtureNonce").toString(), grandNonce);
        grand->page->triggerAction(QWebEnginePage::RequestClose);
        QVERIFY(waitUntil([&] { return h.alivePages(profile) == 0; }));
        QVERIFY(h.drain(profile));
    }
    void profiles() {
        Harness h(&fixture);
        auto *persistent = h.newProfile(false, "persistent");
        auto *privateProfile = h.newProfile(true, "private");
        QVERIFY(!persistent->isOffTheRecord());
        QVERIFY(privateProfile->isOffTheRecord());
        Host *a = h.makeHost(persistent);
        Host *b = h.makeHost(privateProfile);
        a->page->load(fixture.url("/popup.html?kind=cookie"));
        b->page->load(fixture.url("/popup.html?kind=cookie"));
        QVERIFY(waitUntil([&] { return js(a->page, "document.cookie.includes('partition_probe=present')").toBool(); }));
        QVERIFY(waitUntil([&] { return js(b->page, "document.cookie.includes('partition_probe=present')").toBool(); }));
        QCOMPARE(a->page->profile(), persistent);
        QCOMPARE(b->page->profile(), privateProfile);
        h.alternateProfile = privateProfile;
        h.crossProfile = true;
        js(a->page, "window.open('/popup.html?kind=child','_blank')");
        QTRY_VERIFY(h.rejected == 1);
        QCOMPARE(h.adopted, 0);
        h.crossProfile = false;
        for (Host *host : {a, b}) h.closeHost(host);
        QVERIFY(waitUntil([&] { return h.alivePages() == 0; }));
        QVERIFY(h.drain(persistent));
        QVERIFY(h.drain(privateProfile));
    }
    void dialogs() {
        Harness h(&fixture);
        auto *profile = h.newProfile(true, "dialogs");
        Host *host = h.makeHost(profile);
        host->shell->resize(500, 400);
        host->shell->show();
        auto *dialog = new QFileDialog(host->shell, "fixture dialog");
        QCOMPARE(dialog->parentWidget(), host->shell.data());
        dialog->setModal(false);
        dialog->show();
        QCoreApplication::processEvents();
        QVERIFY(dialog->parentWidget() == host->shell);
        QVERIFY(waitUntil([&] { return dialog->windowHandle() && host->shell->windowHandle(); }));
        QCOMPARE(dialog->windowHandle()->transientParent(), host->shell->windowHandle());
        dialog->close();
        delete dialog;

        const auto inputs = host->chrome->rootObject()->findChild<QObject *>("chromeInput");
        QVERIFY(inputs);
        auto *input = qobject_cast<QQuickItem *>(inputs);
        QVERIFY(input);
        input->forceActiveFocus();
        QCOMPARE(host->chrome->rootObject()->property("activeFocus").toBool(), false);
        QVERIFY(waitUntil([&] { return input->hasActiveFocus(); }));
        const QString before = input->property("text").toString();
        QTest::keyClick(host->chrome, Qt::Key_X);
        QVERIFY(waitUntil([&] { return input->property("text").toString() != before; }));
        h.closeHost(host);
        QVERIFY(waitUntil([&] { return h.alivePages() == 0; }));
        QVERIFY(h.drain(profile));
    }
    void failureCleanup() {
        Harness h(&fixture);
        auto *profile = h.newProfile(true, "failures");
        Host *source = h.makeHost(profile);
        source->page->settings()->setAttribute(QWebEngineSettings::JavascriptCanOpenWindows, true);
        source->page->load(fixture.url("/popup.html?kind=top"));
        QVERIFY(waitUntil([&] { return js(source->page, "document.body.dataset.kind").toString() == "top"; }));

        h.failBeforeOpen = true;
        js(source->page, "window.open('/popup.html?kind=child','_blank')");
        QVERIFY(waitUntil([&] { return h.rejected == 1; }));
        QCOMPARE(h.alivePages(profile), 1);
        QCOMPARE(h.adopted, 0);

        h.failAfterOpen = true;
        js(source->page, "window.open('/fail','_blank')");
        QVERIFY(waitUntil([&] { return h.postOpenFailures == 1; }));
        QCOMPARE(h.alivePages(profile), 1);
        QCOMPARE(h.adopted, 1);

        js(source->page, "window.open('/popup.html?kind=child','_blank')");
        QVERIFY(waitUntil([&] { return h.alivePages(profile) == 2; }));
        Host *child = h.hosts.last();
        QVERIFY(waitUntil([&] { return js(child->page, "document.body.dataset.kind").toString() == "child"; }));
        const QString sourceNonce = js(source->page, "fixtureNonce").toString();
        child->page->triggerAction(QWebEnginePage::RequestClose);
        QVERIFY(waitUntil([&] { return h.alivePages(profile) == 1; }));
        QCOMPARE(js(source->page, "fixtureNonce").toString(), sourceNonce);
        h.closeHost(source);
        QVERIFY(waitUntil([&] { return h.alivePages(profile) == 0; }));
        QVERIFY(h.drain(profile));
    }
    void nativeInput() {
        if (qEnvironmentVariableIsEmpty("WAYLAND_DISPLAY")) QSKIP("No native Wayland session");
        Harness h(&fixture);
        auto *profile = h.newProfile(true, "native-input");
        Host *top = h.makeHost(profile);
        top->page->settings()->setAttribute(QWebEngineSettings::JavascriptCanOpenWindows, true);
        top->page->load(fixture.url("/popup.html?kind=top"));
        QVERIFY(waitUntil([&] { return js(top->page, "document.body.dataset.kind").toString() == "top"; }));
        js(top->page, "window.open('/popup.html?kind=child','_blank')");
        QVERIFY(waitUntil([&] { return h.alivePages(profile) == 2; }));
        Host *child = h.hosts.last();
        QVERIFY(waitUntil([&] { return js(child->page, "document.body.dataset.kind").toString() == "child"; }));
        child->shell->setWindowTitle("f02e0-owned-native-input");
        child->shell->show();
        QVERIFY(waitUntil([&] { return child->shell->isActiveWindow(); }, 4000));
        child->view->setFocus(Qt::OtherFocusReason);
        QVERIFY(waitUntil([&] { return child->view->hasFocus(); }));
        // This is deliberately labelled QtTest synthetic input; compositor-delivered input
        // is reported separately in the run record and is never inferred from this event.
        QTest::keyClick(child->view, Qt::Key_B);
        QVERIFY(waitUntil([&] { return js(child->page, "keyCount").toInt() > 0; }));
        for (Host *host : {top, child}) h.closeHost(host);
        QVERIFY(waitUntil([&] { return h.alivePages() == 0; }));
        QVERIFY(h.drain(profile));
    }
private:
    Fixture fixture;
};
}

QTEST_MAIN(PublicPageHostTests)
#include "tst_public_page_host.moc"
