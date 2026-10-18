#include "native_file_chooser.h"
#include "gui_application.h"
#include "local_file_url.h"
#include <KWaylandExtras>
#include <QApplication>
#include <QDir>
#include <QFileSystemModel>
#include <QFont>
#include <QFutureWatcher>
#include <QHeaderView>
#include <QPainter>
#include <QPalette>
#include <QQmlProperty>
#include <QTimer>
#include <QToolButton>
#include <QTreeView>

namespace {
void adaptMonochromeIcons(QWidget *dialog) {
  // Some desktop styles bake black navigation arrows into their standard
  // icons before the dialog receives its window-specific palette. Retint
  // only transparent, dark monochrome icons; preserve colored/file icons.
  for (auto button : dialog->findChildren<QToolButton *>()) {
    const auto source =
        button->icon().pixmap(button->iconSize(), button->devicePixelRatioF());
    const auto pixels = source.toImage().convertToFormat(QImage::Format_ARGB32);
    bool monochrome = !pixels.isNull(), transparent = false, ink = false;
    for (int y = 0; y < pixels.height() && monochrome; ++y) {
      const auto row = reinterpret_cast<const QRgb *>(pixels.constScanLine(y));
      for (int x = 0; x < pixels.width(); ++x) {
        const auto pixel = row[x];
        if (!qAlpha(pixel)) {
          transparent = true;
          continue;
        }
        ink = true;
        const int high = qMax(qRed(pixel), qMax(qGreen(pixel), qBlue(pixel)));
        const int low = qMin(qRed(pixel), qMin(qGreen(pixel), qBlue(pixel)));
        if (high > 128 || high - low > 8) {
          monochrome = false;
          break;
        }
      }
    }
    if (!monochrome || !transparent || !ink)
      continue;
    QIcon icon;
    for (const auto mode : {QIcon::Normal, QIcon::Disabled}) {
      auto tinted = source;
      QPainter painter(&tinted);
      painter.setCompositionMode(QPainter::CompositionMode_SourceIn);
      auto color = button->palette().color(
          mode == QIcon::Disabled ? QPalette::Disabled : QPalette::Active,
          QPalette::ButtonText);
      if (mode == QIcon::Disabled)
        color.setAlphaF(color.alphaF() * 0.45);
      painter.fillRect(tinted.rect(), color);
      painter.end();
      icon.addPixmap(tinted, mode);
    }
    button->setIcon(icon);
  }
}
class SessionFileDialog final : public QFileDialog {
public:
  bool returnOwnerActivation = false;
  ~SessionFileDialog() override {
    // QFileDialog saves its UI state in its base destructor. Forget browsing
    // locations first, even when the owner is destroyed while still open.
    blockSignals(true);
    setHistory({});
    setSidebarUrls({QUrl::fromLocalFile(QDir::homePath()),
                    QUrl::fromLocalFile(QStringLiteral("/"))});
    setDirectory(QDir::homePath());
  }

protected:
  void showEvent(QShowEvent *event) override {
    QFileDialog::showEvent(event);
    // The widget fallback restores pixel-sized columns even when the
    // requesting window uses a larger font. Keep metadata readable and
    // give names a useful initial width; the view can scroll horizontally
    // and users can still resize the name column.
    for (auto tree : findChildren<QTreeView *>()) {
      if (!qobject_cast<QFileSystemModel *>(tree->model()))
        continue;
      auto header = tree->header();
      for (int column = 1; column < header->count(); ++column)
        header->setSectionResizeMode(column, QHeaderView::ResizeToContents);
      header->setSectionResizeMode(0, QHeaderView::Interactive);
      header->resizeSection(0,
                            qMax(tree->viewport()->width() / 2,
                                 fontMetrics().horizontalAdvance(
                                     QStringLiteral("example filename.txt"))));
    }
    adaptMonochromeIcons(this);
  }
  void done(int result) override {
    // QWidget restores other QWidget owners. Our owner is a QQuickWindow,
    // so retain activation intent before QFileDialog hides its window.
    returnOwnerActivation = windowHandle() && windowHandle()->isActive();
    QFileDialog::done(result);
  }
};
bool localAbsoluteUrl(const QUrl &url) {
  return !ferric_browser_local_file_path(url).isEmpty();
}
QPalette windowPalette(QWindow *owner) {
  const auto value = owner->property("palette");
  if (value.canConvert<QPalette>())
    return value.value<QPalette>();
  auto palette = QApplication::palette();
  // Qt Quick exposes a Palette object, not a QPalette value. Read its
  // documented color groups through the public QML property interface.
  const struct {
    const char *name;
    QPalette::ColorRole role;
  } roles[] = {{"window", QPalette::Window},
               {"windowText", QPalette::WindowText},
               {"base", QPalette::Base},
               {"alternateBase", QPalette::AlternateBase},
               {"text", QPalette::Text},
               {"button", QPalette::Button},
               {"buttonText", QPalette::ButtonText},
               {"brightText", QPalette::BrightText},
               {"highlight", QPalette::Highlight},
               {"highlightedText", QPalette::HighlightedText},
               {"placeholderText", QPalette::PlaceholderText},
               {"accent", QPalette::Accent},
               {"toolTipBase", QPalette::ToolTipBase},
               {"toolTipText", QPalette::ToolTipText},
               {"link", QPalette::Link},
               {"linkVisited", QPalette::LinkVisited},
               {"light", QPalette::Light},
               {"midlight", QPalette::Midlight},
               {"mid", QPalette::Mid},
               {"dark", QPalette::Dark},
               {"shadow", QPalette::Shadow}};
  const struct {
    const char *name;
    QPalette::ColorGroup group;
  } groups[] = {{"active", QPalette::Active},
                {"inactive", QPalette::Inactive},
                {"disabled", QPalette::Disabled}};
  for (const auto &group : groups) {
    for (const auto &role : roles) {
      const auto color = QQmlProperty::read(
          owner, QStringLiteral("palette.%1.%2")
                     .arg(QLatin1String(group.name), QLatin1String(role.name)));
      if (color.canConvert<QColor>())
        palette.setColor(group.group, role.role, color.value<QColor>());
    }
  }
  return palette;
}
} // namespace

FerricNativeFileChooser::FerricNativeFileChooser(QObject *parent)
    : QObject(parent), folder_(QUrl::fromLocalFile(QDir::homePath())) {
  parentDeadline_.setSingleShot(true);
  connect(&parentDeadline_, &QTimer::timeout, this, [this] {
    if (!active_)
      return;
    if (requestRequired_)
      complete(FerricPortalFileRequest::Failed, {},
               tr("The system file chooser could not attach to this window. "
                  "Try again."));
    else
      openWidget();
  });
}
FerricNativeFileChooser::~FerricNativeFileChooser() {
  disconnect(ownerDestroyed_);
  if (dialog_)
    disconnect(dialog_.get(), nullptr, this, nullptr);
  if (portal_) {
    disconnect(portal_, nullptr, this, nullptr);
    portal_->cancel();
  }
}

void FerricNativeFileChooser::setParentWindow(QWindow *owner) {
  if (owner_ == owner)
    return;
  close();
  disconnect(ownerDestroyed_);
  owner_ = owner;
  if (owner)
    ownerDestroyed_ = connect(owner, &QObject::destroyed, this, [this] {
      owner_ = nullptr;
      close();
      emit parentWindowChanged();
    });
  emit parentWindowChanged();
}
void FerricNativeFileChooser::setTitle(const QString &title) {
  if (title_ == title)
    return;
  title_ = title;
  if (dialog_)
    dialog_->setWindowTitle(title);
  emit configurationChanged();
}
void FerricNativeFileChooser::setFileMode(int mode) {
  if (mode_ == mode)
    return;
  mode_ = mode;
  emit configurationChanged();
}
void FerricNativeFileChooser::setNameFilters(const QStringList &filters) {
  if (filters_ == filters)
    return;
  filters_ = filters;
  emit configurationChanged();
}
void FerricNativeFileChooser::setOptions(int options) {
  if (options_ == options)
    return;
  options_ = options;
  emit configurationChanged();
}
void FerricNativeFileChooser::setPortalRequired(bool required) {
  if (portalRequired_ == required)
    return;
  portalRequired_ = required;
  emit configurationChanged();
}
void FerricNativeFileChooser::setBackend(const QString &backend) {
  if (backend_ == backend)
    return;
  backend_ = backend;
  emit backendChanged();
}
QUrl FerricNativeFileChooser::currentFolder() const {
  return dialog_ ? dialog_->directoryUrl() : folder_;
}
void FerricNativeFileChooser::setCurrentFolder(const QUrl &folder) {
  if (!localAbsoluteUrl(folder))
    return;
  auto local = folder;
  local.setHost({});
  if (currentFolder() == local)
    return;
  folder_ = local;
  if (dialog_)
    dialog_->setDirectoryUrl(local);
  emit currentFolderChanged();
}
QList<QUrl> FerricNativeFileChooser::selectedFiles() const {
  return dialog_ ? dialog_->selectedUrls() : selection_;
}
QUrl FerricNativeFileChooser::selectedFile() const {
  return selectedFiles().value(0);
}
void FerricNativeFileChooser::setSelectedFile(const QUrl &file) {
  if (!file.isEmpty() && !localAbsoluteUrl(file))
    return;
  auto local = file;
  local.setHost({});
  selection_ = file.isEmpty() ? QList<QUrl>{} : QList<QUrl>{local};
  if (dialog_ && !file.isEmpty())
    dialog_->selectUrl(local);
  emit selectionChanged();
}
void FerricNativeFileChooser::open() {
  if (active_)
    return;
  const auto generation = ++generation_;
  active_ = true;
  requestRequired_ = portalRequired_;
  requestOptions_ = options_;
  if (!ferric_browser_gui_preferences_ready() || !owner_ || mode_ < OpenFile ||
      mode_ > Folder || retired_.size() >= 32) {
    complete(FerricPortalFileRequest::Failed, {},
             tr("The file chooser could not open. Try again."));
    return;
  }
  intent_ = {static_cast<FerricPortalFileRequest::Mode>(mode_),
             {},
             title_,
             filters_,
             folder_,
             selection_.value(0)};
  QPointer<FerricNativeFileChooser> guard(this);
  setBackend({});
  if (!guard || !active_ || generation != generation_)
    return;
  emit visibleChanged();
  if (!guard || !active_ || generation != generation_)
    return;
  const auto platform = QGuiApplication::platformName();
  if (platform.startsWith(QStringLiteral("wayland")) &&
      (requestRequired_ ||
       !(requestOptions_ & QFileDialog::DontUseNativeDialog))) {
    preparePortal();
  } else if (requestRequired_) {
    if (platform == QStringLiteral("xcb")) {
      auto intent = intent_;
      intent.parentHandle =
          QStringLiteral("x11:") + QString::number(owner_->winId(), 16);
      openPortal(intent);
    } else
      complete(FerricPortalFileRequest::Unavailable, {},
               tr("The system file chooser is unavailable in this session. "
                  "Check your desktop portal settings and try again."));
  } else
    openWidget();
}
void FerricNativeFileChooser::preparePortal() {
  const auto generation = generation_;
  const QPointer<QWindow> owner = owner_;
  auto intent = intent_;
  auto watcher = new QFutureWatcher<QString>(this);
  parentExport_ = watcher;
  connect(
      watcher, &QFutureWatcher<QString>::finished, this,
      [this, watcher, generation, owner, intent]() mutable {
        const auto future = watcher->future();
        if (parentExport_ == watcher)
          parentExport_ = nullptr;
        watcher->deleteLater();
        if (!active_ || generation != generation_ ||
            !parentDeadline_.isActive() || !owner || owner_ != owner)
          return;
        parentDeadline_.stop();
        const auto handle = !future.isCanceled() && future.resultCount() == 1
                                ? future.result()
                                : QString{};
        if (handle.isEmpty()) {
          if (requestRequired_)
            complete(
                FerricPortalFileRequest::Unavailable, {},
                tr("The system file chooser could not attach to this window. "
                   "Check your desktop portal settings and try again."));
          else
            openWidget();
          return;
        }
        intent.parentHandle = QStringLiteral("wayland:") + handle;
        openPortal(intent);
      });
  parentDeadline_.start(5000);
  // KWindowSystem caches one export on the native owner. Keep that export
  // until the window dies: unexporting it per request would invalidate
  // another simultaneous chooser attached to the same window.
  watcher->setFuture(KWaylandExtras::exportToplevel(owner));
}
void FerricNativeFileChooser::retireParentExport() {
  parentDeadline_.stop();
  if (!parentExport_)
    return;
  disconnect(parentExport_, nullptr, this, nullptr);
  parentExport_->deleteLater();
  parentExport_ = nullptr;
}
void FerricNativeFileChooser::openPortal(
    const FerricPortalFileRequest::Options &intent) {
  if (!active_ || !owner_)
    return;
  auto request = new FerricPortalFileRequest(intent);
  portal_ = request;
  connect(request, &FerricPortalFileRequest::opened, this, [this, request] {
    if (portal_ == request && active_)
      setBackend(QStringLiteral("system"));
  });
  connect(
      request, &FerricPortalFileRequest::finished, this,
      [this, request](int outcome, const QStringList &uris, const QString &) {
        if (portal_ != request || !active_)
          return;
        portal_ = nullptr;
        // Unavailable is only returned before OpenFile/SaveFile was sent.
        // An uncertain response after that point must never open a second
        // dialog; the first may still be visible on the desktop.
        if (outcome == FerricPortalFileRequest::Unavailable &&
            !requestRequired_) {
          openWidget();
          return;
        }
        QList<QUrl> urls;
        for (const auto &uri : uris)
          urls.append(QUrl::fromEncoded(uri.toUtf8(), QUrl::StrictMode));
        complete(outcome, urls,
                 outcome == FerricPortalFileRequest::Unavailable
                     ? tr("The system file chooser is unavailable. Check your "
                          "desktop portal settings and try again.")
                     : tr("The system file chooser could not complete the "
                          "request. Try again."));
      });
  request->start();
}
void FerricNativeFileChooser::openWidget() {
  retireParentExport();
  if (!active_ || !owner_ || dialog_)
    return;
  auto dialog = std::make_unique<SessionFileDialog>();
  dialog->setHistory({});
  dialog->setSidebarUrls({QUrl::fromLocalFile(QDir::homePath()),
                          QUrl::fromLocalFile(QStringLiteral("/"))});
  dialog->setObjectName(objectName());
  dialog->setWindowTitle(intent_.title);
  // These flags share the documented QFileDialog option values. Only
  // options exposed by our own presentation boundary are accepted.
  dialog->setOptions(QFileDialog::Options::fromInt(requestOptions_ & 0x3e));
  // The fallback is explicitly browser-owned. Letting Qt select another
  // platform dialog here would re-enter the uncancellable toolkit portal.
  dialog->setOption(QFileDialog::DontUseNativeDialog);
  dialog->setAcceptMode(intent_.mode == FerricPortalFileRequest::SaveFile
                            ? QFileDialog::AcceptSave
                            : QFileDialog::AcceptOpen);
  dialog->setFileMode(intent_.mode == FerricPortalFileRequest::OpenFiles
                          ? QFileDialog::ExistingFiles
                      : intent_.mode == FerricPortalFileRequest::SaveFile
                          ? QFileDialog::AnyFile
                      : intent_.mode == FerricPortalFileRequest::Folder
                          ? QFileDialog::Directory
                          : QFileDialog::ExistingFile);
  if (intent_.mode == FerricPortalFileRequest::Folder)
    dialog->setOption(QFileDialog::ShowDirsOnly);
  dialog->setSupportedSchemes({QStringLiteral("file")});
  dialog->setNameFilters(intent_.nameFilters);
  if (localAbsoluteUrl(intent_.currentFolder))
    dialog->setDirectoryUrl(intent_.currentFolder);
  if (!intent_.selectedFile.isEmpty())
    dialog->selectUrl(intent_.selectedFile);
  const auto font = owner_->property("font");
  if (font.canConvert<QFont>())
    dialog->setFont(font.value<QFont>());
  dialog->setPalette(windowPalette(owner_));
  dialog->setWindowModality(Qt::WindowModal);
  dialog->winId();
  dialog->windowHandle()->setTransientParent(owner_);
  auto raw = dialog.get();
  connect(raw, &QDialog::finished, this, [this, raw](int result) {
    finish(raw, result == QDialog::Accepted);
  });
  connect(raw, &QFileDialog::currentChanged, this,
          &FerricNativeFileChooser::selectionChanged);
  connect(raw, &QFileDialog::directoryEntered, this,
          &FerricNativeFileChooser::currentFolderChanged);
  dialog_ = std::move(dialog);
  dialog_->open();
  setBackend(QStringLiteral("browser"));
}
void FerricNativeFileChooser::finish(QFileDialog *dialog, bool wasAccepted) {
  if (dialog_.get() != dialog)
    return;
  const QPointer<QWindow> returnOwner =
      static_cast<SessionFileDialog *>(dialog)->returnOwnerActivation ? owner_
                                                                      : nullptr;
  folder_ = dialog->directoryUrl();
  const auto urls = wasAccepted ? dialog->selectedUrls() : QList<QUrl>{};
  const auto generation = generation_;
  disconnect(dialog, nullptr, this, nullptr);
  dialog->hide();
  retired_.push_back(std::move(dialog_));
  QTimer::singleShot(0, this, [this, dialog, returnOwner, generation] {
    for (auto it = retired_.begin(); it != retired_.end(); ++it)
      if (it->get() == dialog) {
        retired_.erase(it);
        break;
      }
    // Do not steal activation from a different window that took focus
    // while the chooser was open, or reactivate a hidden/retired owner.
    if (generation == generation_ && !active_ && returnOwner &&
        returnOwner == owner_ && returnOwner->isVisible() &&
        (!QGuiApplication::focusWindow() ||
         QGuiApplication::focusWindow() == returnOwner))
      returnOwner->requestActivate();
  });
  complete(wasAccepted ? FerricPortalFileRequest::Accepted
                       : FerricPortalFileRequest::Cancelled,
           urls);
}
void FerricNativeFileChooser::complete(int outcome, const QList<QUrl> &urls,
                                       const QString &message) {
  if (!active_)
    return;
  active_ = false;
  retireParentExport();
  QString failure = message;
  selection_.clear();
  if (outcome == FerricPortalFileRequest::Accepted) {
    if (urls.isEmpty() || (intent_.mode != FerricPortalFileRequest::OpenFiles &&
                           urls.size() != 1)) {
      outcome = FerricPortalFileRequest::Failed;
      failure = tr("No usable file was returned. Choose a file stored on this "
                   "computer and try again.");
    } else {
      for (auto url : urls) {
        if (!localAbsoluteUrl(url)) {
          outcome = FerricPortalFileRequest::Failed;
          failure = tr("This location is not supported. Save the file on this "
                       "computer, then choose it again.");
          selection_.clear();
          break;
        }
        url.setHost({});
        selection_.append(url);
      }
      if (outcome == FerricPortalFileRequest::Accepted && !selection_.isEmpty())
        folder_ = intent_.mode == FerricPortalFileRequest::Folder
                      ? selection_.first()
                      : selection_.first().adjusted(QUrl::RemoveFilename);
      // Match QFileDialog's directory representation without normalizing
      // dot segments: those can cross symlinks and change path identity.
      if (outcome == FerricPortalFileRequest::Accepted &&
          folder_.path().size() > 1 && folder_.path().endsWith('/'))
        folder_.setPath(folder_.path().chopped(1));
    }
  }
  const auto generation = generation_;
  // Receivers can destroy the controller or reopen it from any notification.
  // A retired request must never publish a result into its replacement.
  QTimer::singleShot(0, this, [this, generation, outcome, failure] {
    if (generation != generation_)
      return;
    QPointer<FerricNativeFileChooser> guard(this);
    emit currentFolderChanged();
    if (!guard || generation != generation_)
      return;
    emit selectionChanged();
    if (!guard || generation != generation_)
      return;
    emit visibleChanged();
    if (!guard || generation != generation_)
      return;
    if (outcome == FerricPortalFileRequest::Accepted)
      emit accepted();
    else if (outcome == FerricPortalFileRequest::Cancelled)
      emit rejected();
    else
      emit failed(failure.isEmpty() ? tr("The file chooser could not complete "
                                         "the request. Try again.")
                                    : failure);
  });
}
void FerricNativeFileChooser::close() {
  if (dialog_)
    dialog_->reject();
  else if (active_) {
    if (portal_) {
      auto request = portal_;
      portal_ = nullptr;
      disconnect(request, nullptr, this, nullptr);
      request->cancel();
    }
    complete(FerricPortalFileRequest::Cancelled);
  }
}
void FerricNativeFileChooser::accept() {
  if (dialog_)
    QMetaObject::invokeMethod(dialog_.get(), "accept");
}
void FerricNativeFileChooser::reject() { close(); }
