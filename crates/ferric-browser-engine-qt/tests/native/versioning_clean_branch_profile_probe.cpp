#include <QtCore/QElapsedTimer>
#include <QtCore/QThread>
#include <QtCore/QUrl>
#include <QtGui/QGuiApplication>
#include <QtNetwork/QNetworkCookie>
#include <QtQml/QQmlApplicationEngine>
#include <QtWebEngineCore/QWebEngineCookieStore>
#include <QtWebEngineQuick/QQuickWebEngineProfile>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>

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
    if (!check(argc == 2, "provide the clean-branch QML fixture"))
        return 2;

    QQmlApplicationEngine engine;
    engine.load(QUrl::fromLocalFile(QString::fromLocal8Bit(argv[1])));
    if (!check(engine.rootObjects().size() == 1, "load clean-branch Quick fixture"))
        return 2;
    auto *root = engine.rootObjects().first();
    auto *parent = qobject_cast<QQuickWebEngineProfile *>(
        root->property("browserProfile").value<QObject *>());
    auto *first = qobject_cast<QQuickWebEngineProfile *>(
        root->property("firstProfileObject").value<QObject *>());
    auto *second = qobject_cast<QQuickWebEngineProfile *>(
        root->property("secondProfileObject").value<QObject *>());
    if (!check(parent && first && second && parent != first && parent != second
                   && first != second,
               "three distinct native Quick profile instances"))
        return 1;
    for (auto *profile : {first, second}) {
        if (!check(profile->isOffTheRecord(), "branch profile is off-record")
            || !check(profile->storageName().isEmpty(), "branch storage name is empty")
            || !check(profile->persistentCookiesPolicy()
                           == QQuickWebEngineProfile::NoPersistentCookies,
                       "branch cookies remain memory-only")
            || !check(profile->httpCacheType() == QQuickWebEngineProfile::MemoryHttpCache,
                       "branch cache remains memory-only")
            || !check(profile->persistentPermissionsPolicy()
                           == QQuickWebEngineProfile::PersistentPermissionsPolicy::AskEveryTime,
                       "branch permission grants are not persisted"))
            return 1;
    }
    QVariant admitted;
    if (!check(QMetaObject::invokeMethod(root, "admitBoth", Q_RETURN_ARG(QVariant, admitted))
                   && admitted.toBool(),
               "admit two exact native branch leases"))
        return 1;

    auto *firstStore = first->cookieStore();
    auto *secondStore = second->cookieStore();
    if (!check(firstStore && secondStore && firstStore != secondStore,
               "independent native cookie stores"))
        return 1;
    bool firstOwn = false;
    bool firstOther = false;
    bool secondOwn = false;
    bool secondOther = false;
    QObject::connect(firstStore, &QWebEngineCookieStore::cookieAdded, &app,
                     [&](const QNetworkCookie &cookie) {
                         firstOwn |= cookie.name() == "vb_clean_first";
                         firstOther |= cookie.name() == "vb_clean_second";
                     });
    QObject::connect(secondStore, &QWebEngineCookieStore::cookieAdded, &app,
                     [&](const QNetworkCookie &cookie) {
                         secondOwn |= cookie.name() == "vb_clean_second";
                         secondOther |= cookie.name() == "vb_clean_first";
                     });
    const QUrl origin(QStringLiteral("http://localhost/"));
    QNetworkCookie firstCookie("vb_clean_first", "one");
    firstCookie.setPath(QStringLiteral("/"));
    firstStore->setCookie(firstCookie, origin);
    if (!check(waitUntil(app, [&] { return firstOwn; }, 5000),
               "first branch accepted only its own cookie"))
        return 1;
    QNetworkCookie secondCookie("vb_clean_second", "two");
    secondCookie.setPath(QStringLiteral("/"));
    secondStore->setCookie(secondCookie, origin);
    if (!check(waitUntil(app, [&] { return secondOwn; }, 5000),
               "second branch accepted only its own cookie"))
        return 1;
    firstStore->loadAllCookies();
    secondStore->loadAllCookies();
    waitUntil(app, [] { return false; }, 500);
    if (!check(!firstOther && !secondOther,
               "neither branch observed the other branch's cookie"))
        return 1;

    QVariant released;
    if (!check(QMetaObject::invokeMethod(root, "releaseBoth", Q_RETURN_ARG(QVariant, released))
                   && released.toBool(),
               "release both exact branch leases"))
        return 1;
    std::cout << "PASS: distinct memory-only Quick branch profiles and cookie stores\n";
    return 0;
}
