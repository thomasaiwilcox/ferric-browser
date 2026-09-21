#include "mpris_controller.h"

#include <QtCore/QCoreApplication>
#include <QtCore/QUrl>
#include <QtCore/QUrlQuery>
#include <QtDBus/QDBusConnection>
#include <QtDBus/QDBusConnectionInterface>

namespace {
constexpr auto objectPath = "/org/mpris/MediaPlayer2";
constexpr auto playerInterface = "org.mpris.MediaPlayer2.Player";

bool sensitiveQueryKey(const QString &key)
{
    const auto lower = key.trimmed().toLower();
    return lower == QStringLiteral("pass") || lower == QStringLiteral("password")
        || lower == QStringLiteral("token") || lower == QStringLiteral("secret")
        || lower == QStringLiteral("auth") || lower == QStringLiteral("session")
        || lower == QStringLiteral("signature") || lower == QStringLiteral("sig")
        || lower == QStringLiteral("code") || lower == QStringLiteral("api_key")
        || lower == QStringLiteral("api-key") || lower == QStringLiteral("access_token")
        || lower == QStringLiteral("access-token") || lower == QStringLiteral("refresh_token")
        || lower == QStringLiteral("refresh-token") || lower == QStringLiteral("client_secret")
        || lower == QStringLiteral("client-secret") || lower == QStringLiteral("private_key")
        || lower == QStringLiteral("private-key") || lower == QStringLiteral("credential")
        || lower == QStringLiteral("credentials") || lower == QStringLiteral("jwt")
        || lower == QStringLiteral("bearer") || lower == QStringLiteral("nonce");
}

QString safeMetadataUrl(const QUrl &parsed)
{
    if (!parsed.isValid() || parsed.host().isEmpty()
        || (parsed.scheme() != QStringLiteral("http")
            && parsed.scheme() != QStringLiteral("https"))
        || !parsed.userInfo().isEmpty())
        return {};

    QUrl safe = parsed;
    safe.setFragment({});
    QUrlQuery query(parsed);
    QUrlQuery filtered;
    for (const auto &item : query.queryItems(QUrl::FullyDecoded)) {
        if (!sensitiveQueryKey(item.first))
            filtered.addQueryItem(item.first, item.second);
    }
    safe.setQuery(filtered);
    const auto encoded = safe.toString(QUrl::FullyEncoded);
    return encoded.size() <= 4096 ? encoded : QString{};
}
}

FerricBrowserMprisRootAdaptor::FerricBrowserMprisRootAdaptor(FerricBrowserMprisController *parent)
    : QDBusAbstractAdaptor(parent)
{
}

QString FerricBrowserMprisRootAdaptor::identity() const
{
    return QStringLiteral("FerricBrowser");
}

QString FerricBrowserMprisRootAdaptor::desktopEntry() const
{
    return QStringLiteral("io.github.ferricbrowser.FerricBrowser");
}

bool FerricBrowserMprisRootAdaptor::canQuit() const
{
    return false;
}

bool FerricBrowserMprisRootAdaptor::canRaise() const
{
    return true;
}

bool FerricBrowserMprisRootAdaptor::hasTrackList() const
{
    return false;
}

void FerricBrowserMprisRootAdaptor::Raise()
{
    if (auto *controller = qobject_cast<FerricBrowserMprisController *>(parent()))
        emit controller->raiseRequested();
}

void FerricBrowserMprisRootAdaptor::Quit()
{
    // The browser never permits a desktop media client to close it.
}

FerricBrowserMprisPlayerAdaptor::FerricBrowserMprisPlayerAdaptor(FerricBrowserMprisController *parent)
    : QDBusAbstractAdaptor(parent)
{
}

QString FerricBrowserMprisPlayerAdaptor::playbackStatus() const
{
    const auto *controller = qobject_cast<const FerricBrowserMprisController *>(parent());
    return controller ? controller->playbackStatus() : QStringLiteral("Stopped");
}

QString FerricBrowserMprisPlayerAdaptor::loopStatus() const
{
    return QStringLiteral("None");
}

double FerricBrowserMprisPlayerAdaptor::rate() const
{
    return 1.0;
}

bool FerricBrowserMprisPlayerAdaptor::shuffle() const
{
    return false;
}

double FerricBrowserMprisPlayerAdaptor::volume() const
{
    return 1.0;
}

qlonglong FerricBrowserMprisPlayerAdaptor::position() const
{
    return 0;
}

double FerricBrowserMprisPlayerAdaptor::minimumRate() const
{
    return 1.0;
}

double FerricBrowserMprisPlayerAdaptor::maximumRate() const
{
    return 1.0;
}

QVariantMap FerricBrowserMprisPlayerAdaptor::metadata() const
{
    const auto *controller = qobject_cast<const FerricBrowserMprisController *>(parent());
    return controller ? controller->metadata() : QVariantMap{};
}

bool FerricBrowserMprisPlayerAdaptor::canGoNext() const
{
    return false;
}

bool FerricBrowserMprisPlayerAdaptor::canGoPrevious() const
{
    return false;
}

bool FerricBrowserMprisPlayerAdaptor::canPlay() const
{
    const auto *controller = qobject_cast<const FerricBrowserMprisController *>(parent());
    return controller && controller->canPlay();
}

bool FerricBrowserMprisPlayerAdaptor::canPause() const
{
    return canPlay();
}

bool FerricBrowserMprisPlayerAdaptor::canSeek() const
{
    return false;
}

bool FerricBrowserMprisPlayerAdaptor::canControl() const
{
    return true;
}

void FerricBrowserMprisPlayerAdaptor::PlayPause()
{
    if (auto *controller = qobject_cast<FerricBrowserMprisController *>(parent()))
        emit controller->mediaToggleRequested();
}

FerricBrowserMprisController::FerricBrowserMprisController(QObject *parent)
    : QObject(parent)
{
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected())
        return;

    const auto suffix = QString::number(QCoreApplication::applicationPid());
    serviceName_ = QStringLiteral("org.mpris.MediaPlayer2.ferric-browser.instance") + suffix;
    if (!bus.registerService(serviceName_)) {
        serviceName_.clear();
        return;
    }

    rootAdaptor_ = new FerricBrowserMprisRootAdaptor(this);
    playerAdaptor_ = new FerricBrowserMprisPlayerAdaptor(this);
    if (!bus.registerObject(QString::fromLatin1(objectPath), this,
                            QDBusConnection::ExportAdaptors)) {
        delete playerAdaptor_;
        playerAdaptor_ = nullptr;
        delete rootAdaptor_;
        rootAdaptor_ = nullptr;
        bus.unregisterService(serviceName_);
        serviceName_.clear();
        return;
    }
    available_ = true;
    emit availableChanged();
}

FerricBrowserMprisController::~FerricBrowserMprisController()
{
    auto bus = QDBusConnection::sessionBus();
    if (!serviceName_.isEmpty()) {
        bus.unregisterObject(QString::fromLatin1(objectPath));
        bus.unregisterService(serviceName_);
    }
}

bool FerricBrowserMprisController::available() const
{
    return available_;
}

QString FerricBrowserMprisController::serviceName() const
{
    return serviceName_;
}

void FerricBrowserMprisController::update(const QString &title, const QString &url,
                                        const bool audible, const bool muted,
                                        const bool privateProfile)
{
    QVariantMap nextMetadata;
    const QUrl parsed(url);
    const auto safeUrl = safeMetadataUrl(parsed);
    if (!privateProfile && !safeUrl.isEmpty()) {
        nextMetadata.insert(QStringLiteral("mpris:trackid"),
                            QVariant::fromValue(QDBusObjectPath(
                                QStringLiteral("/org/mpris/MediaPlayer2/Track/Current"))));
        nextMetadata.insert(QStringLiteral("xesam:url"), safeUrl);
        if (!title.trimmed().isEmpty())
            nextMetadata.insert(QStringLiteral("xesam:title"), title.left(512));
    }
    const auto nextStatus = audible && !muted && !nextMetadata.isEmpty()
        ? QStringLiteral("Playing")
        : QStringLiteral("Stopped");
    // URL metadata is useful for a media client, but it does not by itself
    // prove that the current page exposes controllable media.  Keep the
    // advertised control conservative until Qt reports recent page audio;
    // the QML toggle path applies the same guard before invoking the engine.
    const bool nextCanPlay = audible && !muted && !nextMetadata.isEmpty();
    if (metadata_ == nextMetadata && playbackStatus_ == nextStatus && canPlay_ == nextCanPlay)
        return;
    metadata_ = nextMetadata;
    playbackStatus_ = nextStatus;
    canPlay_ = nextCanPlay;
    publishPlayerProperties();
}

void FerricBrowserMprisController::clear()
{
    update({}, {}, false, false, true);
}

QString FerricBrowserMprisController::playbackStatus() const
{
    return playbackStatus_;
}

QVariantMap FerricBrowserMprisController::metadata() const
{
    return metadata_;
}

bool FerricBrowserMprisController::canPlay() const
{
    return canPlay_;
}

void FerricBrowserMprisController::publishPlayerProperties(const QStringList &invalidated)
{
    if (!playerAdaptor_)
        return;
    QVariantMap changed;
    changed.insert(QStringLiteral("PlaybackStatus"), playbackStatus_);
    changed.insert(QStringLiteral("Metadata"), metadata_);
    changed.insert(QStringLiteral("CanPlay"), canPlay_);
    changed.insert(QStringLiteral("CanPause"), canPlay_);
    emit playerAdaptor_->PropertiesChanged(QString::fromLatin1(playerInterface), changed,
                                           invalidated);
}
