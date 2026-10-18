#include "portal_file_request.h"
#include <QCoreApplication>
#include <QDBusArgument>
#include <QDBusMessage>
#include <QDBusMetaType>
#include <QDBusObjectPath>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QDBusServiceWatcher>
#include <QDBusVariant>
#include <QFile>
#include <QFileInfo>
#include <QPointer>
#include <QRegularExpression>
#include <QUuid>

namespace FerricPortalProtocol {
const QString service = QStringLiteral("org.freedesktop.portal.Desktop");
const QString portalPath = QStringLiteral("/org/freedesktop/portal/desktop");
const QString chooserInterface =
    QStringLiteral("org.freedesktop.portal.FileChooser");
const QString requestInterface =
    QStringLiteral("org.freedesktop.portal.Request");
constexpr int ioTimeout = 5000;
struct Pattern {
  uint type;
  QString value;
};
using Patterns = QList<Pattern>;
struct Filter {
  QString label;
  Patterns patterns;
};
using Filters = QList<Filter>;
QDBusArgument &operator<<(QDBusArgument &a, const Pattern &p) {
  a.beginStructure();
  a << p.type << p.value;
  a.endStructure();
  return a;
}
const QDBusArgument &operator>>(const QDBusArgument &a, Pattern &p) {
  a.beginStructure();
  a >> p.type >> p.value;
  a.endStructure();
  return a;
}
QDBusArgument &operator<<(QDBusArgument &a, const Filter &f) {
  a.beginStructure();
  a << f.label << f.patterns;
  a.endStructure();
  return a;
}
const QDBusArgument &operator>>(const QDBusArgument &a, Filter &f) {
  a.beginStructure();
  a >> f.label >> f.patterns;
  a.endStructure();
  return a;
}
Filters serializeFilters(const QStringList &names) {
  Filters filters;
  static const QRegularExpression format(
      QStringLiteral("^(.*)\\(([^()]*)\\)\\s*$"));
  for (const auto &name : names) {
    auto match = format.match(name);
    Filter filter{match.hasMatch() ? match.captured(1).trimmed() : name, {}};
    const auto patterns = match.hasMatch()
                              ? match.captured(2).split(' ', Qt::SkipEmptyParts)
                              : QStringList{QStringLiteral("*")};
    for (const auto &pattern : patterns)
      filter.patterns.append({0, pattern});
    filters.append(filter);
  }
  return filters;
}
} // namespace FerricPortalProtocol
Q_DECLARE_METATYPE(FerricPortalProtocol::Pattern)
Q_DECLARE_METATYPE(FerricPortalProtocol::Patterns)
Q_DECLARE_METATYPE(FerricPortalProtocol::Filter)
Q_DECLARE_METATYPE(FerricPortalProtocol::Filters)
using namespace FerricPortalProtocol;

FerricPortalFileRequest::FerricPortalFileRequest(Options options)
    : QObject(QCoreApplication::instance()), options_(std::move(options)),
      connectionName_(QStringLiteral("ferric-file-") +
                      QUuid::createUuid().toString(QUuid::Id128)),
      bus_(connectionName_) {
  retirementDeadline_.setSingleShot(true);
  connect(&retirementDeadline_, &QTimer::timeout, this, &QObject::deleteLater);
  qDBusRegisterMetaType<Pattern>();
  qDBusRegisterMetaType<Patterns>();
  qDBusRegisterMetaType<Filter>();
  qDBusRegisterMetaType<Filters>();
}
FerricPortalFileRequest::~FerricPortalFileRequest() {
  retirementDeadline_.stop();
  for (auto child : children())
    disconnect(child, nullptr, this, nullptr);
  if (!requestPath_.isEmpty())
    bus_.disconnect(serviceOwner_, requestPath_, requestInterface,
                    QStringLiteral("Response"), this,
                    SLOT(receiveResponse(uint, QVariantMap, QDBusMessage)));
  // This is solely this actor's connection. The browser's shared session bus
  // and other windows' requests remain connected.
  QDBusConnection::disconnectFromBus(connectionName_);
}
void FerricPortalFileRequest::start() {
  if (started_ || resolved_)
    return;
  started_ = true;
  // A cancelled actor can briefly outlive its window to close a late
  // request. Bound that population before allocating another bus socket.
  if (QCoreApplication::instance()
          ->findChildren<FerricPortalFileRequest *>()
          .size() > 64) {
    finish(Failed, {}, QStringLiteral("file-chooser-busy"));
    return;
  }
  bus_ = QDBusConnection::connectToBus(QDBusConnection::SessionBus,
                                       connectionName_);
  if (!bus_.isConnected()) {
    finish(Unavailable, {}, QStringLiteral("bus-unavailable"));
    return;
  }
  auto ownerWatch = new QDBusServiceWatcher(
      service, bus_, QDBusServiceWatcher::WatchForOwnerChange, this);
  connect(ownerWatch, &QDBusServiceWatcher::serviceOwnerChanged, this,
          [this](const QString &, const QString &, const QString &nextOwner) {
            observedOwner_ = nextOwner;
            ownerObserved_ = true;
            if (resolved_ || serviceOwner_.isEmpty() ||
                nextOwner == serviceOwner_)
              return;
            abandoned_ = true;
            if (openIssued_)
              closeRequest(requestPath_);
            finish(openIssued_ ? Failed : Unavailable, {},
                   QStringLiteral("service-owner-changed"));
          });
  resolveOwner(true);
}
void FerricPortalFileRequest::resolveOwner(bool allowActivation) {
  auto call = QDBusMessage::createMethodCall(
      QStringLiteral("org.freedesktop.DBus"),
      QStringLiteral("/org/freedesktop/DBus"),
      QStringLiteral("org.freedesktop.DBus"), QStringLiteral("GetNameOwner"));
  call << service;
  auto watcher =
      new QDBusPendingCallWatcher(bus_.asyncCall(call, ioTimeout), this);
  connect(
      watcher, &QDBusPendingCallWatcher::finished, this,
      [this, watcher, allowActivation] {
        QDBusPendingReply<QString> reply = *watcher;
        watcher->deleteLater();
        if (resolved_)
          return;
        if (reply.isError() && allowActivation &&
            reply.error().name() ==
                QStringLiteral("org.freedesktop.DBus.Error.NameHasNoOwner")) {
          // Installed portals are commonly activated on demand. Resolve and
          // pin the resulting unique owner before passing it any intent.
          auto activation = QDBusMessage::createMethodCall(
              QStringLiteral("org.freedesktop.DBus"),
              QStringLiteral("/org/freedesktop/DBus"),
              QStringLiteral("org.freedesktop.DBus"),
              QStringLiteral("StartServiceByName"));
          activation << service << uint(0);
          auto pending = new QDBusPendingCallWatcher(
              bus_.asyncCall(activation, ioTimeout), this);
          connect(pending, &QDBusPendingCallWatcher::finished, this,
                  [this, pending] {
                    QDBusPendingReply<uint> activated = *pending;
                    pending->deleteLater();
                    if (resolved_)
                      return;
                    if (activated.isError()) {
                      finish(Unavailable, {},
                             QStringLiteral("service-unavailable"));
                      return;
                    }
                    resolveOwner(false);
                  });
          return;
        }
        if (reply.isError() || !reply.value().startsWith(':')) {
          finish(Unavailable, {}, QStringLiteral("service-unavailable"));
          return;
        }
        if (ownerObserved_ && observedOwner_ != reply.value()) {
          finish(Unavailable, {}, QStringLiteral("service-owner-changed"));
          return;
        }
        serviceOwner_ = reply.value();
        probeVersion();
      });
}
void FerricPortalFileRequest::probeVersion() {
  auto call = QDBusMessage::createMethodCall(
      serviceOwner_, portalPath,
      QStringLiteral("org.freedesktop.DBus.Properties"), QStringLiteral("Get"));
  call << chooserInterface << QStringLiteral("version");
  auto watcher =
      new QDBusPendingCallWatcher(bus_.asyncCall(call, ioTimeout), this);
  connect(watcher, &QDBusPendingCallWatcher::finished, this, [this, watcher] {
    QDBusPendingReply<QDBusVariant> reply = *watcher;
    watcher->deleteLater();
    if (resolved_)
      return;
    if (reply.isError() ||
        reply.value().variant().metaType().id() != QMetaType::UInt ||
        reply.value().variant().toUInt() <
            (options_.mode == Folder ? 3U : 1U)) {
      finish(Unavailable, {}, QStringLiteral("chooser-capability-unavailable"));
      return;
    }
    openPortal();
  });
}
bool FerricPortalFileRequest::subscribe(const QString &path) {
  return bus_.connect(serviceOwner_, path, requestInterface,
                      QStringLiteral("Response"), this,
                      SLOT(receiveResponse(uint, QVariantMap, QDBusMessage)));
}
void FerricPortalFileRequest::openPortal() {
  const auto token =
      QStringLiteral("ferric_") + QUuid::createUuid().toString(QUuid::Id128);
  auto sender = bus_.baseService();
  sender.remove(0, 1);
  sender.replace('.', '_');
  requestPath_ =
      portalPath + QStringLiteral("/request/") + sender + '/' + token;
  // Subscribe before OpenFile. A fast portal can respond before its method
  // reply; defer that response until the returned handle has been checked.
  if (!subscribe(requestPath_)) {
    finish(Failed, {}, QStringLiteral("response-subscription-failed"));
    return;
  }
  QVariantMap options{{QStringLiteral("handle_token"), token},
                      {QStringLiteral("modal"), true},
                      {QStringLiteral("multiple"), options_.mode == OpenFiles},
                      {QStringLiteral("directory"), options_.mode == Folder}};
  const auto filters = serializeFilters(options_.nameFilters);
  if (!filters.isEmpty())
    options.insert(QStringLiteral("filters"), QVariant::fromValue(filters));
  if (options_.currentFolder.isLocalFile())
    options.insert(
        QStringLiteral("current_folder"),
        QFile::encodeName(options_.currentFolder.toLocalFile()).append('\0'));
  if (options_.mode == SaveFile && options_.selectedFile.isLocalFile()) {
    QFileInfo file(options_.selectedFile.toLocalFile());
    options.insert(QStringLiteral("current_name"), file.fileName());
    if (file.exists())
      options.insert(QStringLiteral("current_file"),
                     QFile::encodeName(file.absoluteFilePath()).append('\0'));
  }
  auto call = QDBusMessage::createMethodCall(
      serviceOwner_, portalPath, chooserInterface,
      options_.mode == SaveFile ? QStringLiteral("SaveFile")
                                : QStringLiteral("OpenFile"));
  call << options_.parentHandle << options_.title << options;
  openIssued_ = true;
  openReplyPending_ = true;
  auto watcher =
      new QDBusPendingCallWatcher(bus_.asyncCall(call, ioTimeout), this);
  connect(watcher, &QDBusPendingCallWatcher::finished, this, [this, watcher] {
    QDBusPendingReply<QDBusObjectPath> reply = *watcher;
    watcher->deleteLater();
    openReplyPending_ = false;
    if (reply.isError()) {
      if (!resolved_) {
        closeRequest(requestPath_);
        finish(Failed, {}, QStringLiteral("open-request-failed"));
      }
      retireIfReady();
      return;
    }
    const auto actual = reply.value().path();
    if (!actual.startsWith(portalPath + QStringLiteral("/request/"))) {
      if (!resolved_) {
        closeRequest(requestPath_);
        finish(Failed, {}, QStringLiteral("invalid-request-handle"));
      }
      retireIfReady();
      return;
    }
    if (actual != requestPath_) {
      bus_.disconnect(serviceOwner_, requestPath_, requestInterface,
                      QStringLiteral("Response"), this,
                      SLOT(receiveResponse(uint, QVariantMap, QDBusMessage)));
      requestPath_ = actual;
      earlyResponse_.reset();
      if (!resolved_ && !subscribe(requestPath_)) {
        closeRequest(requestPath_);
        finish(Failed, {}, QStringLiteral("response-subscription-failed"));
      }
    }
    if (abandoned_)
      closeRequest(requestPath_);
    else if (!resolved_) {
      QPointer<FerricPortalFileRequest> guard(this);
      emit opened();
      if (!guard)
        return;
      if (!resolved_ && earlyResponse_) {
        const auto response = std::move(*earlyResponse_);
        earlyResponse_.reset();
        processResponse(response);
      }
    }
    retireIfReady();
  });
}
void FerricPortalFileRequest::receiveResponse(uint response,
                                              const QVariantMap &results,
                                              const QDBusMessage &message) {
  if (resolved_ || message.type() != QDBusMessage::SignalMessage ||
      message.service() != serviceOwner_ || message.path() != requestPath_)
    return;
  if (openReplyPending_) {
    if (!earlyResponse_)
      earlyResponse_ = Response{response, results};
    return;
  }
  processResponse({response, results});
}
void FerricPortalFileRequest::processResponse(const Response &response) {
  if (response.code == 1) {
    finish(Cancelled, {}, QStringLiteral("user-cancelled"));
    return;
  }
  if (response.code != 0) {
    finish(Failed, {}, QStringLiteral("portal-rejected"));
    return;
  }
  const auto uris =
      qdbus_cast<QStringList>(response.results.value(QStringLiteral("uris")));
  if (uris.isEmpty() || (options_.mode != OpenFiles && uris.size() != 1)) {
    finish(Failed, {}, QStringLiteral("invalid-file-selection"));
    return;
  }
  finish(Accepted, uris, {});
}
void FerricPortalFileRequest::cancel() {
  if (resolved_)
    return;
  abandoned_ = true;
  if (openIssued_)
    closeRequest(requestPath_);
  finish(Cancelled, {}, QStringLiteral("caller-cancelled"));
}
void FerricPortalFileRequest::closeRequest(const QString &path) {
  if (path.isEmpty() || serviceOwner_.isEmpty() || !bus_.isConnected())
    return;
  ++pendingCloses_;
  auto call = QDBusMessage::createMethodCall(
      serviceOwner_, path, requestInterface, QStringLiteral("Close"));
  auto watcher = new QDBusPendingCallWatcher(bus_.asyncCall(call, 2000), this);
  connect(watcher, &QDBusPendingCallWatcher::finished, this, [this, watcher] {
    watcher->deleteLater();
    --pendingCloses_;
    retireIfReady();
  });
}
void FerricPortalFileRequest::finish(Outcome outcome, const QStringList &uris,
                                     const QString &diagnostic) {
  if (resolved_)
    return;
  resolved_ = true;
  // No timer limits how long the user may choose files. This deadline only
  // bounds cleanup after a terminal decision or cancelled transport call.
  retirementDeadline_.start(8000);
  QTimer::singleShot(0, this, [this, outcome, uris, diagnostic] {
    completionEmitted_ = true;
    QPointer<FerricPortalFileRequest> guard(this);
    emit finished(outcome, uris, diagnostic);
    if (guard)
      retireIfReady();
  });
}
void FerricPortalFileRequest::retireIfReady() {
  if (resolved_ && completionEmitted_ && !openReplyPending_ &&
      pendingCloses_ == 0)
    deleteLater();
}
