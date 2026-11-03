#ifndef FERRIC_POPUP_OWNER_BRIDGE_H
#define FERRIC_POPUP_OWNER_BRIDGE_H
#include <QObject>
#include <QString>
#include <QPointer>
#include <memory>
#include <QtQml/qqmlregistration.h>

// Converts QML QObject references into checked, non-aliasing native owners.
// Runtime/profile admission and transfer remain at the typed Rust boundary.
class FerricPopupOwnerBridge : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(FerricPopupOwnerBridge)
  public:
    using QObject::QObject;
    // Both references are read-only; comparing an owner with itself is safe.
    Q_INVOKABLE bool sharesPermissionSession(QObject *left, QObject *right) const;
    Q_INVOKABLE bool initialize(QObject *source, QObject *destination,
                                const QString &token);
    Q_INVOKABLE bool inheritProfile(QObject *source, QObject *destination,
                                   const QString &profileName);
    Q_INVOKABLE bool preparePageWindow(QObject *source, QObject *destination,
                                      const QString &tabId);
    Q_INVOKABLE bool commitPageWindow();
    Q_INVOKABLE QString activatePreparedPage();
    Q_INVOKABLE QString createdPagePresentationIntent() const;
    Q_INVOKABLE bool presentCreatedPage();
    Q_INVOKABLE bool prepareLivePageWindow(QObject *source, QObject *destination, const QString &nonce);
    Q_INVOKABLE bool commitLivePageWindow();
    Q_INVOKABLE bool livePageCoreCommitted() const;
  private:
    struct LivePreparation;
    std::shared_ptr<LivePreparation> live_;
    struct PreparedWindow;
    static void failPreparedPublication(const std::shared_ptr<PreparedWindow> &state);
    std::shared_ptr<PreparedWindow> prepared_;
    bool preparedCommitted_ = false;
};

class BrowserUi;
void ferric_queue_page_creation_publication(const BrowserUi &ui);

#endif
