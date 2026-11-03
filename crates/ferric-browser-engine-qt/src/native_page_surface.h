#pragma once

#include "public_page_owner.h"

#include <QJSValue>
#include <QPointer>
#include <QQuickItem>
#include <QWebEngineFrame>
#include <QWebEngineFindTextResult>
#include <QWebEngineHistory>
#include <QWebEngineLoadingInfo>
#include <QWebEngineScript>
#include <QtQml/qqmlregistration.h>
#include <array>

class QWebEngineView;
class FerricNativePageSurface;
class FerricScriptRunner;

// The script collection deliberately exposes only the browser's ordinary
// MainWorld/ApplicationWorld scripts. Reserved credential worlds are never
// searchable, removable or writable through the shared QML service API.
class FerricNativePageScripts final : public QObject {
    Q_OBJECT
    QML_ANONYMOUS
public:
    explicit FerricNativePageScripts(FerricNativePageSurface *surface);
    Q_INVOKABLE QVariantList find(const QString &name) const;
    Q_INVOKABLE bool insert(const QJSValue &descriptor);
    Q_INVOKABLE bool remove(const QJSValue &descriptor);
    Q_INVOKABLE void clear();
private:
    bool decode(const QJSValue &descriptor, QWebEngineScript *script) const;
    FerricNativePageSurface *surface_;
};

class FerricNativePageSettings final : public QObject {
    Q_OBJECT
    QML_ANONYMOUS
    Q_PROPERTY(bool javascriptEnabled READ javascriptEnabled WRITE setJavascriptEnabled NOTIFY changed)
    Q_PROPERTY(bool autoLoadImages READ autoLoadImages WRITE setAutoLoadImages NOTIFY changed)
    Q_PROPERTY(bool forceDarkMode READ forceDarkMode WRITE setForceDarkMode NOTIFY changed)
    Q_PROPERTY(bool playbackRequiresUserGesture READ playbackRequiresUserGesture WRITE setPlaybackRequiresUserGesture NOTIFY changed)
    Q_PROPERTY(bool javascriptCanOpenWindows READ javascriptCanOpenWindows WRITE setJavascriptCanOpenWindows NOTIFY changed)
    Q_PROPERTY(bool focusOnNavigationEnabled READ focusOnNavigationEnabled WRITE setFocusOnNavigationEnabled NOTIFY changed)
public:
    explicit FerricNativePageSettings(QObject *parent);
    void bind(QWebEnginePage *page);
    bool javascriptEnabled() const { return get(0); }
    void setJavascriptEnabled(bool value) { set(0, value); }
    bool autoLoadImages() const { return get(1); }
    void setAutoLoadImages(bool value) { set(1, value); }
    bool forceDarkMode() const { return get(2); }
    void setForceDarkMode(bool value) { set(2, value); }
    bool playbackRequiresUserGesture() const { return get(3); }
    void setPlaybackRequiresUserGesture(bool value) { set(3, value); }
    bool javascriptCanOpenWindows() const { return get(4); }
    void setJavascriptCanOpenWindows(bool value) { set(4, value); }
    bool focusOnNavigationEnabled() const { return get(5); }
    void setFocusOnNavigationEnabled(bool value) { set(5, value); }
signals:
    void changed();
private:
    bool get(std::size_t index) const;
    void set(std::size_t index, bool value);
    QPointer<QWebEnginePage> page_;
    std::array<bool, 6> values_{true, true, false, false, true, true};
    std::array<bool, 6> assigned_{};
};

// A visual placeholder and public-page service facade for the shared QML
// workspace. The actual web content remains a QWebEngineView in Widgets; this
// item never embeds, reparents or introspects a Quick WebEngine delegate.
// Binding is native-only and admits a real owner/view pair exactly once.
class FerricNativePageSurface : public QQuickItem {
    Q_OBJECT
    QML_ANONYMOUS
    Q_PROPERTY(QUrl url READ url WRITE setUrl NOTIFY urlChanged)
    Q_PROPERTY(QString title READ title NOTIFY titleChanged)
    Q_PROPERTY(bool loading READ loading NOTIFY pageStateChanged)
    Q_PROPERTY(bool audioMuted READ audioMuted WRITE setAudioMuted NOTIFY audioMutedChanged)
    Q_PROPERTY(bool recentlyAudible READ recentlyAudible NOTIFY recentlyAudibleChanged)
    Q_PROPERTY(qreal zoomFactor READ zoomFactor WRITE setZoomFactor NOTIFY zoomFactorChanged)
    Q_PROPERTY(int lifecycleState READ lifecycleState WRITE setLifecycleState NOTIFY lifecycleStateChanged)
    Q_PROPERTY(qint64 renderProcessPid READ renderProcessPid NOTIFY renderProcessPidChanged)
    Q_PROPERTY(int recommendedState READ recommendedState NOTIFY pageStateChanged)
    Q_PROPERTY(QWebEngineFrame mainFrame READ mainFrame NOTIFY pageStateChanged)
    Q_PROPERTY(QWebEngineHistory *history READ history NOTIFY pageStateChanged)
    Q_PROPERTY(FerricNativePageSettings *settings READ settings CONSTANT)
    Q_PROPERTY(FerricNativePageScripts *userScripts READ userScripts CONSTANT)
public:
    explicit FerricNativePageSurface(QQuickItem *parent = nullptr);
    bool bind(FerricPublicPageOwner *owner, QWebEngineView *view);
    QWebEnginePage *page() const;
    FerricPublicPageOwner *pageOwner() const { return owner_; }
    QWebEngineView *pageView() const;
    QUrl url() const;
    void setUrl(const QUrl &url);
    QString title() const;
    bool loading() const;
    bool audioMuted() const;
    void setAudioMuted(bool muted);
    bool recentlyAudible() const;
    qreal zoomFactor() const;
    void setZoomFactor(qreal zoom);
    int lifecycleState() const;
    void setLifecycleState(int state);
    qint64 renderProcessPid() const;
    int recommendedState() const;
    QWebEngineFrame mainFrame() const;
    QWebEngineHistory *history() const;
    FerricNativePageSettings *settings() { return &settings_; }
    FerricNativePageScripts *userScripts() { return &scripts_; }
    Q_INVOKABLE void runJavaScript(const QString &script, int world = 0,
                                   const QJSValue &callback = {});
    Q_INVOKABLE void triggerWebAction(int action);
    Q_INVOKABLE void goBack();
    Q_INVOKABLE void goForward();
    Q_INVOKABLE void goBackOrForward(int offset);
    Q_INVOKABLE void reload();
    Q_INVOKABLE void stop();
    Q_INVOKABLE void findText(const QString &query, int flags = 0,
                              const QJSValue &callback = {});
signals:
    void pageStateChanged();
    void urlChanged();
    void titleChanged();
    void audioMutedChanged();
    void recentlyAudibleChanged();
    void zoomFactorChanged();
    void lifecycleStateChanged();
    void renderProcessPidChanged();
    void loadingChanged(const QWebEngineLoadingInfo &request);
    void findTextFinished(const QWebEngineFindTextResult &result);
    void authorityInvalidated();
protected:
    void focusInEvent(QFocusEvent *event) override;
private:
    void deliver(const FerricPublicPageOwner::Operation &operation,
                 const QJSValue &callback, const QVariant &value);
    QPointer<FerricPublicPageOwner> owner_;
    QPointer<QWebEngineView> view_;
    bool bound_ = false;
    FerricNativePageSettings settings_;
    FerricNativePageScripts scripts_;
    FerricScriptRunner *runner_;
    QJSValue findCallback_;
    quint64 findSerial_ = 0;
};
