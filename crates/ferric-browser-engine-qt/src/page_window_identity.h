#ifndef FERRIC_PAGE_WINDOW_IDENTITY_H
#define FERRIC_PAGE_WINDOW_IDENTITY_H

#include <QObject>
#include <QPointer>
#include <QString>
#include <QStringList>
#include <QWindow>
#include <QtQml/qqmlregistration.h>

// A mapping-time correlation marker for an application-created window. The
// compositor adapter must still corroborate exact ownership and stable ID.
class FerricPageWindowIdentity : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(FerricPageWindowIdentity)
    Q_PROPERTY(QWindow *targetWindow READ window WRITE setWindow NOTIFY windowChanged)
    Q_PROPERTY(QString tag READ tag CONSTANT)
    Q_PROPERTY(bool prepared READ prepared NOTIFY preparedChanged)
    Q_PROPERTY(bool tagRequested READ tagRequested NOTIFY preparedChanged)
public:
    explicit FerricPageWindowIdentity(QObject *parent = nullptr);
    QWindow *window() const;
    void setWindow(QWindow *window);
    QString tag() const;
    bool prepared() const;
    bool tagRequested() const;
    Q_INVOKABLE QString supersedePageFocus();
    Q_INVOKABLE QString currentPageFocus() const;
    Q_INVOKABLE bool focusIntentCurrent(const QString &intent) const;
    Q_INVOKABLE QString activatePage(QObject *issuer, QObject *target, const QString &nonce);
    Q_INVOKABLE QString pageFocusStatus(QObject *issuer, QObject *target, const QString &nonce) const;
signals:
    void windowChanged();
    void preparedChanged();
private:
    QPointer<QWindow> window_;
    QString tag_;
    bool prepared_ = false;
    bool tagRequested_ = false;
};

class BrowserUi;
// Private CXX read of the actual native owner and its mapping-time marker.
QStringList ferric_browser_native_page_window_identity(const BrowserUi &ui);
QString ferric_browser_page_window_marker(const BrowserUi &ui);
#endif
