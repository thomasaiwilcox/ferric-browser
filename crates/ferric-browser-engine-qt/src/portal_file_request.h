#pragma once
#include <QDBusConnection>
#include <QDBusMessage>
#include <QObject>
#include <QTimer>
#include <QUrl>
#include <QVariantMap>
#include <optional>

// Each actor owns one private bus connection
// and one request; a retired window cannot reuse its authority or callbacks.
class FerricPortalFileRequest final : public QObject {
  Q_OBJECT
public:
  enum Mode { OpenFile, OpenFiles, SaveFile, Folder };
  enum Outcome { Accepted, Cancelled, Unavailable, Failed };
  Q_ENUM(Outcome)
  struct Options {
    Mode mode = OpenFile;
    QString parentHandle;
    QString title;
    QStringList nameFilters;
    QUrl currentFolder;
    QUrl selectedFile;
  };
  explicit FerricPortalFileRequest(Options options);
  ~FerricPortalFileRequest() override;
  void start();
  void cancel();
signals:
  void opened();
  void finished(int outcome, const QStringList &uris,
                const QString &diagnostic);
private slots:
  void receiveResponse(uint response, const QVariantMap &results,
                       const QDBusMessage &message);

private:
  struct Response {
    uint code;
    QVariantMap results;
  };
  bool subscribe(const QString &path);
  void probeVersion();
  void resolveOwner(bool allowActivation);
  void openPortal();
  void processResponse(const Response &response);
  void closeRequest(const QString &path);
  void finish(Outcome outcome, const QStringList &uris,
              const QString &diagnostic);
  void retireIfReady();
  Options options_;
  QString connectionName_;
  QDBusConnection bus_;
  QString serviceOwner_;
  QString observedOwner_;
  bool ownerObserved_ = false;
  QString requestPath_;
  std::optional<Response> earlyResponse_;
  bool started_ = false;
  bool openIssued_ = false;
  bool openReplyPending_ = false;
  bool abandoned_ = false;
  bool resolved_ = false;
  bool completionEmitted_ = false;
  int pendingCloses_ = 0;
  QTimer retirementDeadline_;
};
