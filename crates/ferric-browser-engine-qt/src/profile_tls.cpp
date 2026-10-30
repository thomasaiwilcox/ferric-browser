#include "profile_tls.h"
#include <QCoreApplication>
#include <QMetaMethod>
#include <QQmlProperty>
#include <QThread>
#include <QWebEngineFrame>
#include <QtWebEngineQuick/QQuickWebEngineProfile>
#include <limits>

namespace {
QString host(const QUrl &url) { return QString::fromLatin1(QUrl::toAce(url.host())).toLower(); }
QUrl origin(const QUrl &url)
{
    QUrl result;
    result.setScheme(url.scheme()); result.setHost(url.host());
    result.setPort(url.port(443));
    return result;
}
bool unsupportedTlsFlags()
{
    auto arguments = QCoreApplication::arguments();
    arguments.append(QString::fromLocal8Bit(qgetenv("QTWEBENGINE_CHROMIUM_FLAGS")).split(QChar::Space, Qt::SkipEmptyParts));
    for (const auto &argument : arguments) {
        const auto flag = argument.toLower();
        if (flag.contains("ignore-certificate") || flag.contains("ignore-ssl")
            || flag.contains("allow-insecure") || flag.contains("disable-web-security")
            || flag.contains("unsafely-treat-insecure")) return true;
    }
    return false;
}
}

FerricProfileTls *FerricProfileTls::establish(QObject *profile)
{
    if (!qobject_cast<QQuickWebEngineProfile *>(profile) || QThread::currentThread() != profile->thread()) return nullptr;
    if (auto *existing = profile->findChild<FerricProfileTls *>(QString(), Qt::FindDirectChildrenOnly)) return existing;
    return new FerricProfileTls(profile, false);
}

FerricProfileTls *FerricProfileTls::createFresh(QObject *profile)
{
    if (!qobject_cast<QQuickWebEngineProfile *>(profile)
        || profile->findChild<FerricProfileTls *>(QString(), Qt::FindDirectChildrenOnly)) return nullptr;
    return new FerricProfileTls(profile, true);
}

FerricProfileTls::FerricProfileTls(QObject *profile, bool fresh)
    : QObject(profile), profile_(profile), unknownProfile_(!fresh || unsupportedTlsFlags()) {}

void FerricProfileTls::denyProfile()
{
    unknownProfile_ = true;
    if (profileEpoch_ != std::numeric_limits<quint64>::max()) ++profileEpoch_;
    emit changed();
}

void FerricProfileTls::advance(Document &document)
{
    document.eligible = false;
    document.finalOrigin = QUrl();
    if (nextEpoch_ == std::numeric_limits<quint64>::max()) { unknownProfile_ = true; document.epoch = 0; }
    else document.epoch = nextEpoch_++;
}

bool FerricProfileTls::observeView(QQuickItem *view)
{
    if (!profile_ || !view || view->thread() != thread() || QThread::currentThread() != thread()
        || view->property("profile").value<QObject *>() != profile_) return false;
    if (documents_.contains(view)) return true;
    Document document;
    document.view = view;
    document.renderer = view->property("renderProcessPid").toLongLong();
    advance(document);
    documents_.insert(view, document);
    const bool connected = connect(view, SIGNAL(certificateError(QWebEngineCertificateError)), this, SLOT(certificateError(QWebEngineCertificateError)))
        && connect(view, SIGNAL(loadingChanged(QWebEngineLoadingInfo)), this, SLOT(loadingChanged(QWebEngineLoadingInfo)))
        && QQmlProperty(view, "profile").connectNotifySignal(this, SLOT(revokeView()))
        && QQmlProperty(view, "renderProcessPid").connectNotifySignal(this, SLOT(rendererChanged()))
        && QQmlProperty(view, "lifecycleState").connectNotifySignal(this, SLOT(lifecycleChanged()));
    QMetaMethod terminated;
    for (int index = 0; index < view->metaObject()->methodCount(); ++index) {
        const auto method = view->metaObject()->method(index);
        if (method.methodType() == QMetaMethod::Signal && method.name() == "renderProcessTerminated" && method.parameterCount() == 2) terminated = method;
    }
    const auto slot = metaObject()->method(metaObject()->indexOfSlot("revokeView()"));
    const bool terminationConnected = terminated.isValid() && QObject::connect(view, terminated, this, slot);
    connect(view, &QObject::destroyed, this, [this, view] { documents_.remove(view); });
    const auto url = view->property("url").toUrl();
    // A later navigation cannot erase an exception cached before observation.
    if (!connected || !terminationConnected || view->property("loading").toBool() || (!url.isEmpty() && url != QUrl("about:blank"))) denyProfile();
    return connected;
}

void FerricProfileTls::certificateError(const QWebEngineCertificateError &error)
{
    auto *view = qobject_cast<QQuickItem *>(sender());
    if (!view || !documents_.contains(view) || view->property("profile").value<QObject *>() != profile_) return;
    const auto value = host(error.url());
    if (value.isEmpty() || taintedHosts_.size() >= 1024 || profileEpoch_ == std::numeric_limits<quint64>::max()) { denyProfile(); return; }
    if (taintedHosts_.contains(value)) return;
    taintedHosts_.insert(value); // no eviction; Qt exceptions survive view/port changes
    ++profileEpoch_;
    emit changed();
}

void FerricProfileTls::loadingChanged(const QWebEngineLoadingInfo &info)
{
    auto *view = qobject_cast<QQuickItem *>(sender());
    auto document = documents_.find(view);
    if (document == documents_.end()) return;
    if (info.status() == QWebEngineLoadingInfo::LoadStartedStatus) {
        advance(*document);
        document->started = true;
    } else {
        const bool started = document->started;
        advance(*document);
        document->started = false;
        auto frame = view->property("mainFrame").value<QWebEngineFrame>();
        // Qt reports normal HTTP status codes in errorCode, including a
        // non-error 401/404 document as LoadFailed. HTTP application status
        // does not replace or weaken this profile's native certificate proof.
        const bool httpDocument = info.errorDomain() == QWebEngineLoadingInfo::HttpStatusCodeDomain
            && info.errorCode() >= 200 && info.errorCode() < 600
            && (info.status() == QWebEngineLoadingInfo::LoadSucceededStatus || info.status() == QWebEngineLoadingInfo::LoadFailedStatus);
        if (started && !info.isErrorPage() && !info.isDownload()
            && (httpDocument || (info.status() == QWebEngineLoadingInfo::LoadSucceededStatus
                && info.errorCode() == 0 && info.errorDomain() == QWebEngineLoadingInfo::NoErrorDomain))
            && frame.isValid() && frame.isMainFrame() && frame.url().scheme() == QLatin1String("https")
            && origin(frame.url()) == origin(info.url()) && view->property("profile").value<QObject *>() == profile_) {
            document->finalOrigin = origin(frame.url());
            document->renderer = view->property("renderProcessPid").toLongLong();
            document->eligible = document->renderer > 0;
        }
    }
    emit changed();
}

void FerricProfileTls::rendererChanged()
{
    auto *view = qobject_cast<QQuickItem *>(sender());
    auto document = documents_.find(view);
    if (document == documents_.end()) return;
    if (document->eligible && document->renderer != view->property("renderProcessPid").toLongLong()) revokeView();
}

void FerricProfileTls::revokeView()
{
    auto document = documents_.find(qobject_cast<QQuickItem *>(sender()));
    if (document == documents_.end()) return;
    // Frozen pages may return to Active without a new load, but never gain a
    // new TLS proof merely through that lifecycle notification.
    advance(*document);
    document->started = false;
    emit changed();
}

void FerricProfileTls::lifecycleChanged()
{
    const auto *view = qobject_cast<QQuickItem *>(sender());
    if (view && view->property("lifecycleState").toInt() == 2) revokeView();
}

bool FerricProfileTls::eligible(QQuickItem *view, const QUrl &requestedOrigin) const
{
    if (!profile_ || unknownProfile_ || unsupportedTlsFlags() || !view || QThread::currentThread() != thread()
        || view->property("profile").value<QObject *>() != profile_ || view->property("loading").toBool()) return false;
    const auto document = documents_.constFind(view);
    const auto frame = view->property("mainFrame").value<QWebEngineFrame>();
    return document != documents_.cend() && document->epoch && document->eligible
        && document->renderer == view->property("renderProcessPid").toLongLong()
        && frame.isValid() && frame.isMainFrame() && frame.url().scheme() == QLatin1String("https")
        && document->finalOrigin == origin(frame.url()) && document->finalOrigin == origin(requestedOrigin)
        && !taintedHosts_.contains(host(frame.url()));
}

QString FerricProfileTls::identity(QQuickItem *view) const
{
    const auto document = documents_.constFind(view);
    return document == documents_.cend() ? QString() : QStringLiteral("%1/%2").arg(profileEpoch_).arg(document->epoch);
}
