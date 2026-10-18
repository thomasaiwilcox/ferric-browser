#ifndef FERRIC_NATIVE_FILE_CHOOSER_H
#define FERRIC_NATIVE_FILE_CHOOSER_H

#include "portal_file_request.h"
#include <QFileDialog>
#include <QFutureWatcher>
#include <QPointer>
#include <QTimer>
#include <QUrl>
#include <QWindow>
#include <QtQml/qqmlregistration.h>
#include <memory>
#include <vector>

// Owns the complete native/widget chooser, including its fallback. No Qt
// private API or QML-engine-owned fallback window crosses this boundary.
class FerricNativeFileChooser : public QObject {
  Q_OBJECT
  QML_NAMED_ELEMENT(NativeFileChooser)
  Q_PROPERTY(QWindow *parentWindow READ parentWindow WRITE setParentWindow
                 NOTIFY parentWindowChanged)
  Q_PROPERTY(
      QString title READ title WRITE setTitle NOTIFY configurationChanged)
  Q_PROPERTY(
      int fileMode READ fileMode WRITE setFileMode NOTIFY configurationChanged)
  Q_PROPERTY(QStringList nameFilters READ nameFilters WRITE setNameFilters
                 NOTIFY configurationChanged)
  Q_PROPERTY(
      int options READ options WRITE setOptions NOTIFY configurationChanged)
  Q_PROPERTY(QUrl currentFolder READ currentFolder WRITE setCurrentFolder NOTIFY
                 currentFolderChanged)
  Q_PROPERTY(QUrl currentFile READ selectedFile WRITE setSelectedFile NOTIFY
                 selectionChanged)
  Q_PROPERTY(QUrl selectedFile READ selectedFile WRITE setSelectedFile NOTIFY
                 selectionChanged)
  Q_PROPERTY(
      QList<QUrl> selectedFiles READ selectedFiles NOTIFY selectionChanged)
  Q_PROPERTY(QUrl selectedFolder READ selectedFile WRITE setSelectedFile NOTIFY
                 selectionChanged)
  Q_PROPERTY(bool visible READ visible NOTIFY visibleChanged)
  Q_PROPERTY(bool portalRequired READ portalRequired WRITE setPortalRequired
                 NOTIFY configurationChanged)
  Q_PROPERTY(QString backend READ backend NOTIFY backendChanged)
public:
  enum FileMode { OpenFile, OpenFiles, SaveFile, Folder };
  Q_ENUM(FileMode)
  explicit FerricNativeFileChooser(QObject *parent = nullptr);
  ~FerricNativeFileChooser() override;
  QWindow *parentWindow() const { return owner_; }
  void setParentWindow(QWindow *owner);
  QString title() const { return title_; }
  void setTitle(const QString &title);
  int fileMode() const { return mode_; }
  void setFileMode(int mode);
  QStringList nameFilters() const { return filters_; }
  void setNameFilters(const QStringList &filters);
  int options() const { return options_; }
  void setOptions(int options);
  QUrl currentFolder() const;
  void setCurrentFolder(const QUrl &folder);
  QUrl selectedFile() const;
  QList<QUrl> selectedFiles() const;
  void setSelectedFile(const QUrl &file);
  bool visible() const { return active_; }
  bool portalRequired() const { return portalRequired_; }
  void setPortalRequired(bool required);
  QString backend() const { return backend_; }
  Q_INVOKABLE void open();
  Q_INVOKABLE void close();
  Q_INVOKABLE void accept();
  Q_INVOKABLE void reject();
signals:
  void parentWindowChanged();
  void configurationChanged();
  void currentFolderChanged();
  void selectionChanged();
  void visibleChanged();
  void accepted();
  void rejected();
  void failed(const QString &message);
  void backendChanged();

private:
  void finish(QFileDialog *dialog, bool accepted);
  void openWidget();
  void preparePortal();
  void openPortal(const FerricPortalFileRequest::Options &intent);
  void complete(int outcome, const QList<QUrl> &urls = {},
                const QString &message = {});
  void setBackend(const QString &backend);
  void retireParentExport();
  QPointer<QWindow> owner_;
  QMetaObject::Connection ownerDestroyed_;
  QString title_;
  QStringList filters_{QStringLiteral("All files (*)")};
  int mode_ = OpenFile;
  int options_ = 0;
  QUrl folder_;
  QList<QUrl> selection_;
  bool active_ = false;
  bool portalRequired_ = false;
  bool requestRequired_ = false;
  quint64 generation_ = 0;
  QString backend_;
  QTimer parentDeadline_;
  QPointer<QFutureWatcher<QString>> parentExport_;
  QPointer<FerricPortalFileRequest> portal_;
  FerricPortalFileRequest::Options intent_;
  int requestOptions_ = 0;
  std::unique_ptr<QFileDialog> dialog_;
  // Finished dialogs stay owned until their signal stack has unwound.
  // The destructor also owns this cleanup if the event loop has stopped.
  std::vector<std::unique_ptr<QFileDialog>> retired_;
};

#endif
