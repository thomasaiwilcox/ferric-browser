#include "../../src/versioning_cookie_adapter.h"

#include <QtCore/QDateTime>
#include <QtCore/QElapsedTimer>
#include <QtCore/QThread>
#include <QtCore/QUrl>
#include <QtGui/QGuiApplication>
#include <QtNetwork/QHostAddress>
#include <QtNetwork/QNetworkCookie>
#include <QtNetwork/QTcpServer>
#include <QtNetwork/QTcpSocket>
#include <QtQml/QQmlApplicationEngine>
#include <QtWebEngineQuick/QQuickWebEngineProfile>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>

#include <algorithm>
#include <functional>
#include <iostream>

namespace {
bool check(bool condition, const char *message)
{
    if (!condition)
        std::cerr << "FAIL: " << message << '\n';
    return condition;
}

bool waitUntil(QGuiApplication &app, const std::function<bool()> &predicate, int timeoutMs)
{
    QElapsedTimer elapsed;
    elapsed.start();
    while (!predicate() && elapsed.elapsed() < timeoutMs) {
        app.processEvents();
        QThread::msleep(5);
    }
    return predicate();
}

struct ObservedFacts {
    bool seed = false;
    bool concurrent = false;
    bool javascript = false;
    bool shortLived = false;
    bool sequenceValid = true;
    quint64 lastSequence = 0;
    std::size_t count = 0;

    void drain(FerricVersioningCookieAdapter &adapter)
    {
        for (const auto &item : adapter.takeObservations()) {
            sequenceValid &= item.epoch == adapter.epoch()
                             && item.sequence == lastSequence + 1;
            lastSequence = item.sequence;
            ++count;
            if (item.kind != FerricVersioningCookieAdapter::Kind::ObservedAdded)
                continue;
            seed |= item.name == "vb_boundary_seed" && item.value == "synthetic-seed";
            concurrent |= item.name == "vb_boundary_live" && item.value == "synthetic-live";
            javascript |= item.name == "vb_boundary_js" && item.value == "synthetic-js";
            shortLived |= item.name == "vb_boundary_short"
                          && item.value == "synthetic-short";
        }
    }
};

struct RequestFacts {
    QByteArray path;
    bool seed = false;
    bool javascript = false;
    bool shortLived = false;
};

auto requestAt(const QList<RequestFacts> &requests, const char *path)
{
    return std::find_if(requests.cbegin(), requests.cend(), [path](const RequestFacts &request) {
        return request.path.contains(path);
    });
}
}

int main(int argc, char **argv)
{
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
    QGuiApplication app(argc, argv);
    QtWebEngineQuick::initialize();
    if (!check(argc == 3, "provide QML fixture and seed/observe stage"))
        return 2;
    const QByteArray stage(argv[2]);
    if (!check(stage == "seed" || stage == "observe", "known probe stage"))
        return 2;
    QQmlApplicationEngine engine;
    engine.load(QUrl::fromLocalFile(QString::fromLocal8Bit(argv[1])));
    if (!check(engine.rootObjects().size() == 1, "load disposable persistent Quick fixture"))
        return 2;
    auto *root = engine.rootObjects().first();
    auto *profile = qobject_cast<QQuickWebEngineProfile *>(
        root->property("probeProfile").value<QObject *>());
    if (!check(profile && !profile->isOffTheRecord(), "profile uses disposable persistent store"))
        return 1;
    FerricVersioningCookieAdapter adapter;
    if (!check(adapter.attach(profile), "attach before first fixture navigation"))
        return 1;
    auto *store = profile->cookieStore();
    const QUrl origin(QStringLiteral("http://localhost/"));
    ObservedFacts facts;

    if (stage == "seed") {
        QNetworkCookie cookie("vb_boundary_seed", "synthetic-seed");
        cookie.setPath(QStringLiteral("/"));
        cookie.setExpirationDate(QDateTime::currentDateTimeUtc().addDays(1));
        store->setCookie(cookie, origin);
        if (!check(waitUntil(app, [&] {
                       facts.drain(adapter);
                       return facts.seed;
                   }, 10000), "persistent synthetic seed reached native observer")
            || !check(facts.sequenceValid && !adapter.invalidated(),
                      "seed observation sequence remains valid"))
            return 1;
        // Allow the documented disk-backed profile to settle before the
        // process exits. This delay is not a snapshot or durability proof.
        waitUntil(app, [] { return false; }, 2500);
        std::cout << "PASS: disposable persistent fixture seeded\n";
        return 0;
    }

    // Trigger enumeration and a live write before servicing callbacks. The
    // public stream does not label which added event belongs to which source.
    if (!check(adapter.requestEnumeration(), "dispatch initial persistent enumeration"))
        return 1;
    QNetworkCookie live("vb_boundary_live", "synthetic-live");
    live.setPath(QStringLiteral("/"));
    store->setCookie(live, origin);
    const bool observedBoth = waitUntil(app, [&] {
        facts.drain(adapter);
        return facts.seed && facts.concurrent;
    }, 3000);
    if (!observedBoth) {
        if (!facts.concurrent)
            std::cerr << "FAIL: concurrent cookie absent from observer\n";
        if (!facts.concurrent)
            return 1;
    }
    if (!check(facts.sequenceValid && !adapter.invalidated(),
               "enumeration/write sequence remained bounded"))
        return 1;
    const bool seedObservedDuringInitialWindow = facts.seed;

    // A local page mutates document.cookie while the same profile is being
    // observed. Only synthetic presence bits, never Cookie headers, survive.
    QTcpServer server;
    if (!check(server.listen(QHostAddress::LocalHost, 0), "listen on loopback-only fixture"))
        return 1;
    QList<RequestFacts> requests;
    QObject::connect(&server, &QTcpServer::newConnection, &app, [&] {
        while (auto *socket = server.nextPendingConnection()) {
            QObject::connect(socket, &QTcpSocket::readyRead, &app,
                             [&, socket, buffer = QByteArray{}, handled = false]() mutable {
                                 if (handled)
                                     return;
                                 buffer.append(socket->readAll());
                                 if (buffer.size() > 64 * 1024) {
                                     handled = true;
                                     socket->disconnectFromHost();
                                     return;
                                 }
                                 const auto headersEnd = buffer.indexOf("\r\n\r\n");
                                 if (headersEnd < 0)
                                     return;
                                 handled = true;
                                 const auto requestLine = buffer.left(buffer.indexOf("\r\n"));
                                 const auto headers = buffer.left(headersEnd).toLower();
                                 requests.append({
                                     requestLine,
                                     headers.contains("vb_boundary_seed=synthetic-seed"),
                                     headers.contains("vb_boundary_js=synthetic-js"),
                                     headers.contains("vb_boundary_short=synthetic-short")});
                                 const auto body = requestLine.contains("/set-js")
                                     ? QByteArray("<!doctype html><script>document.cookie='vb_boundary_js=synthetic-js; Path=/'</script>")
                                     : QByteArray("<!doctype html><title>boundary fixture</title>");
                                 const auto response = QByteArray("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: ")
                                     + QByteArray::number(body.size())
                                     + "\r\nConnection: close\r\n\r\n" + body;
                                 socket->write(response);
                                 socket->disconnectFromHost();
                             });
            QObject::connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
        }
    });
    QUrl setJs(QStringLiteral("http://localhost"));
    setJs.setPort(server.serverPort());
    setJs.setPath(QStringLiteral("/set-js"));
    root->setProperty("probeUrl", setJs);
    if (!check(waitUntil(app, [&] {
                   facts.drain(adapter);
                   return facts.javascript && requestAt(requests, "/set-js") != requests.cend();
               }, 15000), "JavaScript mutation reached native observer"))
        return 1;
    const auto beforeJs = requestAt(requests, "/set-js");
    if (!check(beforeJs->seed, "server truth confirms persistent seed was in Chromium jar"))
        return 1;

    QNetworkCookie shortLived("vb_boundary_short", "synthetic-short");
    shortLived.setPath(QStringLiteral("/"));
    shortLived.setExpirationDate(QDateTime::currentDateTimeUtc().addSecs(3));
    store->setCookie(shortLived, origin);
    if (!check(waitUntil(app, [&] {
                   facts.drain(adapter);
                   return facts.shortLived;
               }, 10000), "short-lived cookie reached observer"))
        return 1;
    waitUntil(app, [] { return false; }, 4000);
    QUrl afterExpiry = setJs;
    afterExpiry.setPath(QStringLiteral("/after-expiry"));
    root->setProperty("probeUrl", afterExpiry);
    if (!check(waitUntil(app, [&] {
                   facts.drain(adapter);
                   return requestAt(requests, "/after-expiry") != requests.cend();
               }, 10000), "post-expiration browser request reached local server"))
        return 1;
    const auto after = requestAt(requests, "/after-expiry");
    if (!check(after->seed && after->javascript && !after->shortLived,
               "server truth shows retained, JS and expired-cookie states")
        || !check(facts.sequenceValid && !adapter.invalidated(),
                  "observation sequence remains valid across mutations"))
        return 1;

    const auto beforeRescan = facts.count;
    if (!check(adapter.requestEnumeration(), "dispatch diagnostic rescan"))
        return 1;
    waitUntil(app, [&] {
        facts.drain(adapter);
        return false;
    }, 250);
    if (!check(facts.sequenceValid && !adapter.invalidated(),
               "diagnostic rescan sequence remains valid"))
        return 1;
    std::cout << "PASS: persistent source, concurrent write, JS mutation and expiration observed; "
              << "initial seed callback=" << (seedObservedDuringInitialWindow ? "yes" : "no")
              << "; "
              << "rescan events=" << facts.count - beforeRescan
              << " (no completion or exact T0 inferred)\n";
    return 0;
}
