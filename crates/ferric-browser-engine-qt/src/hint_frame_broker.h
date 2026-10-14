#ifndef FERRIC_HINT_FRAME_BROKER_H
#define FERRIC_HINT_FRAME_BROKER_H

#include <QtCore/QHash>
#include <QtCore/QObject>
#include <QtQml/QJSValue>
#include <QtQml/qqmlregistration.h>
#include <QtWebEngineCore/QWebEngineFrame>

// Handles and document leases stay in the browser process. No page payload can
// select a native frame by its URL, name, position or enumeration index.
class FerricHintFrameBroker : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(FerricHintFrameBroker)
public:
    explicit FerricHintFrameBroker(QObject *parent = nullptr);
    Q_INVOKABLE void attach(const QWebEngineFrame &frame, const QJSValue &callback);
    Q_INVOKABLE void focusedChild(const QString &parent, const QJSValue &callback);
    Q_INVOKABLE void run(const QString &owner, const QString &script, const QJSValue &callback);
    Q_INVOKABLE void dispatch(const QString &owner, const QString &script, const QJSValue &callback);
    Q_INVOKABLE void clear();
    Q_PROPERTY(int leaseCount READ leaseCount NOTIFY leasesChanged)
    int leaseCount() const { return frames_.size(); }
signals:
    void leasesChanged();
private:
    struct Lease { QWebEngineFrame frame; QString epoch; QString parent; };
    using Completion = std::function<void(const QVariant &)>;
    void evaluate(QWebEngineFrame frame, const QString &script, Completion completion);
    void attachFrame(const QWebEngineFrame &frame, const QString &parent, Completion completion);
    void deliver(const QJSValue &callback, const QVariant &value);
    bool reachable(const QString &owner) const;
    void execute(const QString &owner, const QString &script, const QJSValue &callback, bool action);
    QHash<QString, Lease> frames_;
    QList<QWebEngineFrame> inspected_;
    quint64 generation_ = 0;
    int inFlight_ = 0;
};
#endif
