#include "versioning_archive_interceptor.h"

#include <QtCore/QFile>
#include <QtCore/QFileInfo>
#include <QtCore/QUrl>
#include <QtCore/QWriteLocker>
#include <QtWebEngineCore/QWebEngineUrlRequestInfo>

#include <limits>

#if defined(Q_OS_UNIX)
#include <sys/stat.h>
#include <unistd.h>
#endif

namespace {

constexpr quint64 saturatingIncrement(quint64 value) noexcept
{
    constexpr auto maximum = std::numeric_limits<quint64>::max();
    return value == maximum ? maximum : value + 1;
}

static_assert(saturatingIncrement(0) == 1);
static_assert(saturatingIncrement(std::numeric_limits<quint64>::max())
              == std::numeric_limits<quint64>::max());

} // namespace

FerricVersioningArchiveInterceptor::FerricVersioningArchiveInterceptor(QObject *parent)
    : QWebEngineUrlRequestInterceptor(parent)
{
}

FerricVersioningArchiveInterceptor::~FerricVersioningArchiveInterceptor()
{
    detachProfiles();
}

bool FerricVersioningArchiveInterceptor::attach(QObject *profile)
{
    auto *quickProfile = qobject_cast<QQuickWebEngineProfile *>(profile);
    if (quickProfile == nullptr || !quickProfile->isOffTheRecord()) {
        return false;
    }
    for (auto attached = profiles_.begin(); attached != profiles_.end();) {
        if (attached->isNull()) {
            attached = profiles_.erase(attached);
        } else if (*attached != quickProfile) {
            return false;
        } else {
            ++attached;
        }
    }
    if (!profiles_.contains(quickProfile)) {
        profiles_.append(QPointer<QQuickWebEngineProfile>(quickProfile));
    }
    quickProfile->setUrlRequestInterceptor(this);
    return true;
}

bool FerricVersioningArchiveInterceptor::authorizeArchiveUrl(const QString &url)
{
    const QUrl parsed(url, QUrl::StrictMode);
    if (!parsed.isValid() || !parsed.isLocalFile() || !parsed.userInfo().isEmpty()
        || parsed.hasQuery() || parsed.hasFragment()) {
        clearAuthorization();
        return false;
    }

    const QFileInfo file(parsed.toLocalFile());
    if (!file.isFile() || file.isSymLink() || !file.isReadable()
        || file.suffix().compare(QStringLiteral("mhtml"), Qt::CaseInsensitive) != 0
        || file.size() <= 0
        || file.size() > 64 * 1024 * 1024) {
        clearAuthorization();
        return false;
    }
#if defined(Q_OS_UNIX)
    const auto encodedPath = QFile::encodeName(file.absoluteFilePath());
    struct stat metadata {};
    if (::lstat(encodedPath.constData(), &metadata) != 0 || !S_ISREG(metadata.st_mode)
        || metadata.st_uid != ::geteuid() || (metadata.st_mode & 0777) != 0600
        || metadata.st_nlink != 1 || metadata.st_size <= 0
        || metadata.st_size > 64 * 1024 * 1024 || metadata.st_size != file.size()) {
        clearAuthorization();
        return false;
    }
#else
    clearAuthorization();
    return false;
#endif
    const auto canonicalPath = file.canonicalFilePath();
    if (canonicalPath.isEmpty()) {
        clearAuthorization();
        return false;
    }
    const auto canonicalUrl = QUrl::fromLocalFile(canonicalPath);
    if (canonicalUrl.toString(QUrl::FullyEncoded) != parsed.toString(QUrl::FullyEncoded)) {
        clearAuthorization();
        return false;
    }

    QWriteLocker locker(&authorizationLock_);
    authorizedArchiveUrl_ = canonicalUrl.toString(QUrl::FullyEncoded);
    initialMainFramePending_ = true;
    blankMainFramePending_ = false;
    // Scope diagnostics to this exact one-use archive authorization so a
    // previous file cannot satisfy the next native fixture's assertions.
    blockedMainFrameRequests_ = 0;
    blockedSubFrameRequests_ = 0;
    blockedStylesheetRequests_ = 0;
    blockedImageRequests_ = 0;
    blockedOtherRequests_ = 0;
    return true;
}

void FerricVersioningArchiveInterceptor::clearAuthorization()
{
    QWriteLocker locker(&authorizationLock_);
    authorizedArchiveUrl_.clear();
    initialMainFramePending_ = false;
    blankMainFramePending_ = true;
}

void FerricVersioningArchiveInterceptor::interceptRequest(QWebEngineUrlRequestInfo &info)
{
    bool allowed = false;
    {
        // URL and one-use flags form one authorization generation. Reading
        // the URL under a separate lock could consume a newer generation's
        // permit for a stale earlier archive request.
        QWriteLocker locker(&authorizationLock_);
        if (blankMainFramePending_
            && info.resourceType() == QWebEngineUrlRequestInfo::ResourceTypeMainFrame
            && info.requestMethod() == QByteArrayLiteral("GET")
            && info.requestUrl() == QUrl(QStringLiteral("about:blank"))) {
            blankMainFramePending_ = false;
            allowed = true;
        } else if (initialMainFramePending_ && !authorizedArchiveUrl_.isEmpty()
                   && info.resourceType() == QWebEngineUrlRequestInfo::ResourceTypeMainFrame
                   && info.requestMethod() == QByteArrayLiteral("GET")
                   && info.requestUrl().toString(QUrl::FullyEncoded) == authorizedArchiveUrl_) {
            initialMainFramePending_ = false;
            allowed = true;
        }
        if (!allowed) {
            // Keep only aggregate resource classes so the native probe can
            // distinguish profile interception from renderer-side suppression.
            // Never retain a request URL, referrer, or payload.
            switch (info.resourceType()) {
            case QWebEngineUrlRequestInfo::ResourceTypeMainFrame:
                blockedMainFrameRequests_ = saturatingIncrement(blockedMainFrameRequests_);
                break;
            case QWebEngineUrlRequestInfo::ResourceTypeSubFrame:
                blockedSubFrameRequests_ = saturatingIncrement(blockedSubFrameRequests_);
                break;
            case QWebEngineUrlRequestInfo::ResourceTypeStylesheet:
                blockedStylesheetRequests_ = saturatingIncrement(blockedStylesheetRequests_);
                break;
            case QWebEngineUrlRequestInfo::ResourceTypeImage:
                blockedImageRequests_ = saturatingIncrement(blockedImageRequests_);
                break;
            default:
                blockedOtherRequests_ = saturatingIncrement(blockedOtherRequests_);
                break;
            }
        }
    }
    if (!allowed) {
        info.block(true);
    }
}

quint64 FerricVersioningArchiveInterceptor::blockedRequestCount(
    QWebEngineUrlRequestInfo::ResourceType type) const
{
    QReadLocker locker(&authorizationLock_);
    switch (type) {
    case QWebEngineUrlRequestInfo::ResourceTypeMainFrame:
        return blockedMainFrameRequests_;
    case QWebEngineUrlRequestInfo::ResourceTypeSubFrame:
        return blockedSubFrameRequests_;
    case QWebEngineUrlRequestInfo::ResourceTypeStylesheet:
        return blockedStylesheetRequests_;
    case QWebEngineUrlRequestInfo::ResourceTypeImage:
        return blockedImageRequests_;
    default:
        return blockedOtherRequests_;
    }
}

void FerricVersioningArchiveInterceptor::detachProfiles()
{
    for (const auto &profile : profiles_) {
        if (profile != nullptr) {
            profile->setUrlRequestInterceptor(nullptr);
        }
    }
    profiles_.clear();
}
