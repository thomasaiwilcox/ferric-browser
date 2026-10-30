#include "public_profile_owner.h"

#include "public_page_owner.h"
#include "request_interceptor.h"

#include <QList>
#include <QDir>
#include <QPointer>
#include <QThread>
#include <QtWebEngineCore/QWebEngineProfile>
#include <QtWebEngineCore/QWebEngineProfileBuilder>

#include <atomic>
#include <utility>

namespace {
std::atomic<quint64> nextProfileGeneration{1};
}

FerricPublicProfileOwner::FerricPublicProfileOwner(
    FerricBrowserRequestInterceptor *interceptor, QObject *parent)
    : FerricPublicProfileOwner(interceptor, Configuration{}, parent) {}

FerricPublicProfileOwner::FerricPublicProfileOwner(
    FerricBrowserRequestInterceptor *interceptor,
    const Configuration &configuration, QObject *parent)
    : QObject(parent), interceptor_(interceptor),
      generation_(
          nextProfileGeneration.fetch_add(1, std::memory_order_relaxed)) {
  if (interceptor == nullptr || interceptor->thread() != thread()) {
    return;
  }

  QWebEngineProfile *profile = nullptr;
  if (configuration.storageName.isEmpty()) {
    // An accidental persistent path must not silently turn a private profile
    // into a disk-backed profile or be ignored by a misleading configuration.
    if (!configuration.persistentStoragePath.isEmpty() || !configuration.cachePath.isEmpty()) return;
    profile = QWebEngineProfileBuilder::createOffTheRecordProfile(this);
  } else {
    if (!QDir::isAbsolutePath(configuration.persistentStoragePath)
        || !QDir::isAbsolutePath(configuration.cachePath)) return;
    QWebEngineProfileBuilder builder;
    builder.setPersistentStoragePath(configuration.persistentStoragePath)
        .setCachePath(configuration.cachePath)
        .setPersistentPermissionsPolicy(QWebEngineProfile::PersistentPermissionsPolicy::AskEveryTime);
    profile = builder.createProfile(configuration.storageName, this);
  }
  if (!profile) return;
  profile->setPersistentPermissionsPolicy(QWebEngineProfile::PersistentPermissionsPolicy::AskEveryTime);
  if (!interceptor->attach(profile)) {
    delete profile;
    return;
  }
  profile_ = profile;

  connect(interceptor, &QObject::destroyed, this, [this] { shutdown(); });
  connect(profile, &QObject::destroyed, this, [this] {
    profile_ = nullptr;
    shutdown();
  });
}

FerricPublicProfileOwner::~FerricPublicProfileOwner() { shutdown(); }

bool FerricPublicProfileOwner::isReady() const {
  return profile_ && interceptor_ && !shuttingDown_;
}

FerricPublicPageOwner *FerricPublicProfileOwner::createPage(QObject *host) {
  if (!isReady() || host == nullptr || host->thread() != thread() ||
      QThread::currentThread() != thread()) {
    return nullptr;
  }

  auto *page = new FerricPublicPageOwner(this, profile_, host, host);
  if (page->page() == nullptr) {
    delete page;
    return nullptr;
  }
  pages_.insert(page);
  connect(page, &QObject::destroyed, this,
          [this, page] { pageOwnerDestroyed(page); });
  emit pageCountChanged();
  return page;
}

bool FerricPublicProfileOwner::shutdown() {
  if (shuttingDown_) {
    return pages_.isEmpty() && profile_.isNull();
  }
  shuttingDown_ = true;

  QList<QPointer<FerricPublicPageOwner>> pages;
  pages.reserve(pages_.size());
  for (FerricPublicPageOwner *page : std::as_const(pages_)) {
    pages.append(page);
  }
  for (const auto &page : std::as_const(pages)) {
    if (page) {
      page->close();
    }
  }

  if (!pages_.isEmpty()) {
    return false;
  }
  if (profile_ != nullptr) {
    if (interceptor_ != nullptr) {
      interceptor_->detach(profile_);
    }
    QWebEngineProfile *const profile = profile_.data();
    profile_ = nullptr;
    delete profile;
  }
  if (!drainedEmitted_) {
    drainedEmitted_ = true;
    emit drained();
  }
  return true;
}

void FerricPublicProfileOwner::pageClosed(FerricPublicPageOwner *page) {
  if (pages_.remove(page) > 0) {
    emit pageCountChanged();
  }
}

void FerricPublicProfileOwner::pageOwnerDestroyed(FerricPublicPageOwner *page) {
  if (pages_.remove(page) > 0) {
    emit pageCountChanged();
  }
}
