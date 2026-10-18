#include "local_file_url.h"
#include <QtCore/QDir>

// Receive QUrl directly; converting through display text can decode a literal
// percent sign and change its identity when parsed a second time.
QString ferric_browser_local_file_path(const QUrl &value) {
    auto url = value;
    if (!url.isValid() || !url.isLocalFile() || url.hasQuery() || url.hasFragment()
        || !url.userInfo().isEmpty() || url.port() != -1
        || (!url.host().isEmpty() && url.host().compare(QStringLiteral("localhost"), Qt::CaseInsensitive) != 0)) {
        return {};
    }
    // Qt represents localhost as a UNC path; this Linux browser treats it as
    // the same local absolute path as an empty file-URL host.
    url.setHost({});
    const auto path = url.toLocalFile();
    return QDir::isAbsolutePath(path) && !path.contains(QChar::Null) ? path : QString{};
}
