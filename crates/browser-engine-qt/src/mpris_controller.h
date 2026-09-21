#ifndef RUSTBROWSER_MPRIS_CONTROLLER_H
#define RUSTBROWSER_MPRIS_CONTROLLER_H

#include <QtCore/QObject>
#include <QtCore/QPointer>
#include <QtCore/QVariantMap>
#include <QtDBus/QDBusAbstractAdaptor>
#include <QtDBus/QDBusObjectPath>
#include <QtQml/qqmlregistration.h>

class RustBrowserMprisController;

class RustBrowserMprisRootAdaptor final : public QDBusAbstractAdaptor {
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "org.mpris.MediaPlayer2")
    Q_PROPERTY(QString Identity READ identity CONSTANT)
    Q_PROPERTY(QString DesktopEntry READ desktopEntry CONSTANT)
    Q_PROPERTY(bool CanQuit READ canQuit CONSTANT)
    Q_PROPERTY(bool CanRaise READ canRaise CONSTANT)
    Q_PROPERTY(bool HasTrackList READ hasTrackList CONSTANT)

public:
    explicit RustBrowserMprisRootAdaptor(RustBrowserMprisController *parent);

    QString identity() const;
    QString desktopEntry() const;
    bool canQuit() const;
    bool canRaise() const;
    bool hasTrackList() const;

public slots:
    void Raise();
    void Quit();
};

class RustBrowserMprisPlayerAdaptor final : public QDBusAbstractAdaptor {
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "org.mpris.MediaPlayer2.Player")
    Q_PROPERTY(QString PlaybackStatus READ playbackStatus)
    Q_PROPERTY(QString LoopStatus READ loopStatus CONSTANT)
    Q_PROPERTY(double Rate READ rate CONSTANT)
    Q_PROPERTY(bool Shuffle READ shuffle CONSTANT)
    Q_PROPERTY(double Volume READ volume CONSTANT)
    Q_PROPERTY(qlonglong Position READ position)
    Q_PROPERTY(double MinimumRate READ minimumRate CONSTANT)
    Q_PROPERTY(double MaximumRate READ maximumRate CONSTANT)
    Q_PROPERTY(QVariantMap Metadata READ metadata)
    Q_PROPERTY(bool CanGoNext READ canGoNext CONSTANT)
    Q_PROPERTY(bool CanGoPrevious READ canGoPrevious CONSTANT)
    Q_PROPERTY(bool CanPlay READ canPlay)
    Q_PROPERTY(bool CanPause READ canPause)
    Q_PROPERTY(bool CanSeek READ canSeek CONSTANT)
    Q_PROPERTY(bool CanControl READ canControl CONSTANT)

public:
    explicit RustBrowserMprisPlayerAdaptor(RustBrowserMprisController *parent);

    QString playbackStatus() const;
    QString loopStatus() const;
    double rate() const;
    bool shuffle() const;
    double volume() const;
    qlonglong position() const;
    double minimumRate() const;
    double maximumRate() const;
    QVariantMap metadata() const;
    bool canGoNext() const;
    bool canGoPrevious() const;
    bool canPlay() const;
    bool canPause() const;
    bool canSeek() const;
    bool canControl() const;

public slots:
    void PlayPause();

signals:
    void PropertiesChanged(const QString &interfaceName, const QVariantMap &changed,
                           const QStringList &invalidated);
};

class RustBrowserMprisController : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(MprisController)
    Q_PROPERTY(bool available READ available NOTIFY availableChanged)
    Q_PROPERTY(QString serviceName READ serviceName NOTIFY availableChanged)

public:
    explicit RustBrowserMprisController(QObject *parent = nullptr);
    ~RustBrowserMprisController() override;

    bool available() const;
    QString serviceName() const;

    Q_INVOKABLE void update(const QString &title, const QString &url, bool audible,
                            bool muted, bool privateProfile);
    Q_INVOKABLE void clear();

    QString playbackStatus() const;
    QVariantMap metadata() const;
    bool canPlay() const;

signals:
    void availableChanged();
    void mediaToggleRequested();
    void raiseRequested();
    void quitRequested();

private:
    void publishPlayerProperties(const QStringList &invalidated = {});

    bool available_ = false;
    QString serviceName_;
    QString playbackStatus_ = QStringLiteral("Stopped");
    QVariantMap metadata_;
    bool canPlay_ = false;
    RustBrowserMprisRootAdaptor *rootAdaptor_ = nullptr;
    RustBrowserMprisPlayerAdaptor *playerAdaptor_ = nullptr;
};

#endif
