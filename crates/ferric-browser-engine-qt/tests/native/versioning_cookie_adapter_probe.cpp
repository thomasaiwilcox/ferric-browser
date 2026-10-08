#include "../../src/versioning_cookie_adapter.h"

#include <QtCore/QElapsedTimer>
#include <QtCore/QThread>
#include <QtCore/QUrl>
#include <QtGui/QGuiApplication>
#include <QtNetwork/QNetworkCookie>
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
}

int main(int argc, char **argv)
{
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
    QGuiApplication app(argc, argv);
    QtWebEngineQuick::initialize();
    if (!check(argc == 2, "provide disposable QML profile fixture"))
        return 2;
    QQmlApplicationEngine engine;
    engine.load(QUrl::fromLocalFile(QString::fromLocal8Bit(argv[1])));
    if (!check(engine.rootObjects().size() == 1, "load disposable Quick view"))
        return 2;
    auto *root = engine.rootObjects().first();
    auto *profile = qobject_cast<QQuickWebEngineProfile *>(
        root->property("probeProfile").value<QObject *>());
    if (!check(profile && profile->isOffTheRecord(), "fixture profile is off the record"))
        return 1;
    FerricVersioningCookieAdapter adapter;
    if (!check(adapter.attach(profile), "attach disposable Quick profile")
        || !check(adapter.requestEnumeration(), "dispatch public enumeration request"))
        return 1;
    auto *store = profile->cookieStore();
    const QUrl origin(QStringLiteral("https://example.test/"));
    QNetworkCookie first("synthetic-name", "synthetic-secret-value");
    first.setPath(QStringLiteral("/"));
    store->setCookie(first, origin);
    if (!check(waitUntil(app, [&] { return adapter.queuedCount() > 0; }, 10000),
               "synthetic cookie observation reached bounded queue"))
        return 1;
    const auto firstEpoch = adapter.epoch();
    const auto observed = adapter.takeObservations();
    bool found = false;
    quint64 lastSequence = 0;
    for (const auto &item : observed) {
        lastSequence = std::max(lastSequence, item.sequence);
        if (item.name == "synthetic-name" && item.value == "synthetic-secret-value") {
            found = item.kind == FerricVersioningCookieAdapter::Kind::ObservedAdded
                    && item.epoch == firstEpoch && item.sequence > 0;
        }
    }
    if (!check(found, "raw bytes remain in C++-only observation result")
        || !check(!adapter.overflowed(), "first observation did not overflow"))
        return 1;

    store->deleteCookie(first, origin);
    bool removed = false;
    const bool removalArrived = waitUntil(app, [&] {
        for (const auto &item : adapter.takeObservations()) {
            if (item.kind == FerricVersioningCookieAdapter::Kind::ObservedRemoved
                && item.name == "synthetic-name"
                && item.value == "synthetic-secret-value"
                && item.epoch == firstEpoch && item.sequence > lastSequence)
                removed = true;
        }
        return removed;
    }, 10000);
    if (!check(removalArrived, "removal observation retains epoch and monotonic sequence"))
        return 1;

    for (size_t index = 0; index < FerricVersioningCookieAdapter::maxQueued() + 8; ++index) {
        const QByteArray name = "overflow-" + QByteArray::number(index);
        QNetworkCookie cookie(name, "synthetic-secret-overflow");
        cookie.setPath(QStringLiteral("/"));
        store->setCookie(cookie, origin);
    }
    if (!check(waitUntil(app, [&] { return adapter.overflowed(); }, 15000),
               "overflow invalidates observation state")
        || !check(adapter.queuedCount() == 0, "overflow discards partial queue")
        || !check(!adapter.requestEnumeration(), "invalidated epoch refuses rescan"))
        return 1;

    if (!check(adapter.attach(profile), "rebind explicit same profile with fresh epoch")
        || !check(adapter.epoch() > firstEpoch, "rebind changes observation epoch"))
        return 1;
    QNetworkCookie queued("queued-before-destroy", "synthetic-secret-queued");
    queued.setPath(QStringLiteral("/"));
    store->setCookie(queued, origin);
    if (!check(waitUntil(app, [&] { return adapter.queuedCount() > 0; }, 10000),
               "observation queued before profile destruction"))
        return 1;
    const auto boundEpoch = adapter.epoch();
    delete root;
    app.processEvents();
    if (!check(!adapter.attached(), "destroyed profile detaches adapter")
        || !check(adapter.queuedCount() == 0, "destruction drops queued secrets")
        || !check(adapter.epoch() > boundEpoch, "destruction invalidates old epoch")
        || !check(!adapter.requestEnumeration(), "destroyed profile cannot enumerate"))
        return 1;

    std::cout << "PASS: bounded Quick cookie observation, overflow invalidation and profile teardown\n";
    return 0;
}
