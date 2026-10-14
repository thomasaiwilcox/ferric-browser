#ifndef FERRIC_HINT_FRAME_BROKER_H
#define FERRIC_HINT_FRAME_BROKER_H

#include <QtCore/QHash>
#include <QtCore/QObject>
#include <QtQml/QJSValue>
#include <QtQml/qqmlregistration.h>
#include <QtWebEngineCore/QWebEngineFrame>

// Handles and document leases stay in the browser process. No page payload can
// select a native frame by its URL, name, position or enumeration index.
class FerricFrameBroker : public QObject {
    Q_OBJECT
    QML_ANONYMOUS
public:
    explicit FerricFrameBroker(QObject *parent, const QString &documentProperty);
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
    QString documentProperty_;
    QHash<QString, Lease> frames_;
    QList<QWebEngineFrame> inspected_;
    quint64 generation_ = 0;
    int inFlight_ = 0;
};
// Independent lease namespaces and lifetimes over the same native authority.
class FerricHintFrameBroker : public FerricFrameBroker {
    Q_OBJECT
    QML_NAMED_ELEMENT(FerricHintFrameBroker)
public:
    explicit FerricHintFrameBroker(QObject *parent = nullptr)
        : FerricFrameBroker(parent, QStringLiteral("__ferric_hintFrameDocument")) {}
};
class FerricSelectionFrameBroker : public FerricFrameBroker {
    Q_OBJECT
    QML_NAMED_ELEMENT(FerricSelectionFrameBroker)
public:
    explicit FerricSelectionFrameBroker(QObject *parent = nullptr)
        : FerricFrameBroker(parent, QStringLiteral("__ferric_selectionFrameDocument")) {}
};
#endif
