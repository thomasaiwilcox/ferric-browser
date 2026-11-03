#pragma once
#include <QHash>
#include <QObject>
#include <QPointer>
#include <QQuickItem>
#include <QSet>
#include <QString>
#include <QUrl>
#include <QWebEngineCertificateError>
#include <QWebEngineLoadingInfo>

// Always-on native observation of one actual Qt profile, independent of the
// phone setting. No QML properties/invokables or page-supplied security facts.
class FerricProfileTls final : public QObject {
    Q_OBJECT
public:
    static FerricProfileTls *establish(QObject *profile);
    bool observeView(QQuickItem *view);
    bool eligible(QQuickItem *view, const QUrl &origin) const;
    QString identity(QQuickItem *view) const;
signals:
    void changed();
private slots:
    void certificateError(const QWebEngineCertificateError &error);
    void loadingChanged(const QWebEngineLoadingInfo &info);
    void rendererChanged();
    void lifecycleChanged();
    void revokeView();
private:
    friend class FerricQuickProfileFactory;
    static FerricProfileTls *createFresh(QObject *profile);
    explicit FerricProfileTls(QObject *profile, bool fresh);
    struct Document {
        QPointer<QQuickItem> view;
        quint64 epoch = 0;
        qint64 renderer = 0;
        bool started = false;
        bool eligible = false;
        QUrl finalOrigin;
    };
    void advance(Document &document);
    void denyProfile();
    QPointer<QObject> profile_;
    QHash<QQuickItem *, Document> documents_;
    QSet<QString> taintedHosts_;
    quint64 profileEpoch_ = 1;
    quint64 nextEpoch_ = 1;
    bool unknownProfile_ = false;
};
