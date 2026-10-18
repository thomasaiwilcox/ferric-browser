#include "download_source.h"
#include <QtWebEngineQuick/QQuickWebEngineDownloadRequest>
#include <QtWebEngineCore/private/qwebenginedownloadrequest_p.h>
#include <QtWebEngineCore/qtwebenginecoreglobal.h>
#include <QtWebEngineCore/qtwebenginecoreversion.h>
#include <QThread>
#include <QVariant>
#include <cstring>

namespace {
// A declared pointer-to-member accesses the protected scoped pointer without
// downcasting an existing request to a probe subclass or guessing offsets.
class DownloadRequestAccess : public QWebEngineDownloadRequest {
  public:
    static bool hasSource(const QWebEngineDownloadRequest &request) {
        const auto member = &DownloadRequestAccess::d_ptr;
        const auto &data = request.*member;
        return data && data->adapterClient;
    }
};
}

bool FerricDownloadSource::supported() const {
    // Private declarations are used only with the exact SDK library version.
    // A system Qt update must not turn a missing-source check into an ABI read.
    return std::strcmp(qWebEngineVersion(), QTWEBENGINECORE_VERSION_STR) == 0;
}

QQuickItem *FerricDownloadSource::viewFor(QObject *download) const {
    if (!supported() || !download || download->thread() != QThread::currentThread())
        return nullptr;
    auto request = qobject_cast<QQuickWebEngineDownloadRequest *>(download);
    if (!request || !DownloadRequestAccess::hasSource(*request))
        return nullptr;
    // Qt's public QML getter unconditionally dereferences adapterClient in
    // 6.11.2. The guard above implements its documented nullable contract.
    return qobject_cast<QQuickItem *>(request->property("view").value<QObject *>());
}
