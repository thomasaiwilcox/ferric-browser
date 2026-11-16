#include <QtQuickTest/quicktest.h>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <QtWebEngineCore/qtwebenginecoreglobal.h>
#include <QElapsedTimer>
#include <QJsonDocument>
#include <QJsonObject>
#include <QApplication>
#include <QInputMethodEvent>
#include <QObject>
#include <QQmlEngine>
#include <QQmlContext>
#include <QJSEngine>
#include <QStringList>
#include <QThread>
#include <QTimer>
#include <QSet>
#include <cmath>
#include <QtQml/qqml.h>
#include "../../src/script_runner.h"
#include "../../src/browser_key_router.h"
#include "../../src/settings_model.h"
#include "../../src/native_file_chooser.h"
#include "../../src/gui_application.h"
#include "../../src/download_source.h"
#include "../../src/hint_frame_broker.h"

// Test-only observation of the shipping broker's native-only receipt API.
// No JSON result can construct the retained DocumentReceipt value.
class FrameReceiptProbe : public QObject
{
    Q_OBJECT
public:
    Q_INVOKABLE void blockGui(int milliseconds) {
        if (milliseconds > 0 && milliseconds <= 1500) QThread::msleep(static_cast<unsigned long>(milliseconds));
    }
    Q_INVOKABLE void deleteOnLease(QObject *object) {
        auto *broker = qobject_cast<FerricHintFrameBroker *>(object);
        deletionTarget_ = broker;
        if (broker) QObject::connect(broker, &FerricFrameBroker::leasesChanged, broker,
            [broker] { delete broker; }, static_cast<Qt::ConnectionType>(Qt::DirectConnection | Qt::SingleShotConnection));
    }
    Q_INVOKABLE bool watchedBrokerAlive() const { return !deletionTarget_.isNull(); }
    Q_INVOKABLE void watchBroker(QObject *object) { deletionTarget_=qobject_cast<FerricHintFrameBroker *>(object); }
    Q_INVOKABLE int pendingScripts(QObject *object) const {
        const auto *broker = qobject_cast<FerricHintFrameBroker *>(object);
        return broker ? broker->pendingScripts() : -1;
    }
    Q_INVOKABLE void reset() { receipt_.reset(); fragments_.reset(); graphAccounting_.reset(); documentAccounting_.reset(); }
    Q_INVOKABLE void configureGraphAccounting(int refusedPass, const QString &interruption = QString()) {
        graphAccounting_ = std::make_shared<GraphAccounting>();
        graphAccounting_->refusedPass = refusedPass;
        graphAccounting_->interruption = interruption;
    }
    Q_INVOKABLE QVariantList graphEvents() const {
        return graphAccounting_ ? graphAccounting_->events : QVariantList();
    }
    Q_INVOKABLE void configureDocumentAccounting(int refusedDocument, int refusedPass, const QString &interruption = QString()) {
        documentAccounting_=std::make_shared<DocumentAccounting>();
        documentAccounting_->refusedDocument=refusedDocument;
        documentAccounting_->refusedPass=refusedPass;
        documentAccounting_->interruption=interruption;
    }
    Q_INVOKABLE QVariantList documentEvents() const {
        return documentAccounting_?documentAccounting_->events:QVariantList();
    }
    Q_INVOKABLE bool hasFragmentCatalogue() const { return fragments_.has_value(); }
    Q_INVOKABLE bool receiptFrameValid() const {
        return receipt_ && receipt_->documents().last().frame.isValid();
    }
    Q_INVOKABLE bool matchesFrame(const QWebEngineFrame &frame) const
    {
        return receipt_ && receipt_->documents().last().frame == frame;
    }
    Q_INVOKABLE void capture(QObject *object, const QString &owner, const QJSValue &callback)
    {
        receipt_.reset();
        auto *broker = qobject_cast<FerricHintFrameBroker *>(object);
        if (!broker) { deliver(callback, QVariantMap{{"ok", false}}); return; }
        const QPointer<FrameReceiptProbe> guard(this);
        const auto admission=documentAdmission(broker);
        broker->captureDocument(owner, [guard, callback](std::optional<FerricFrameBroker::DocumentReceipt> receipt) {
            if (!guard) return;
            guard->receipt_ = std::move(receipt);
            QVariantList documents;
            if (guard->receipt_) for (const auto &document : guard->receipt_->documents())
                documents.append(QVariantMap{{"owner", document.owner}, {"parent", document.parent},
                    {"epoch", document.epoch}, {"origin", document.origin}, {"focused", document.focused},
                    {"main", document.frame.isMainFrame()}, {"url", document.url.toString()}});
            guard->deliver(callback, QVariantMap{{"ok", guard->receipt_.has_value()}, {"documents", documents}});
        },1000,admission?std::optional<FerricFrameBroker::DocumentWork>({admission,QStringLiteral("bind")}):std::nullopt);
    }
    Q_INVOKABLE void validate(QObject *object, const QJSValue &callback)
    {
        auto *broker = qobject_cast<FerricHintFrameBroker *>(object);
        if (!broker || !receipt_) { deliver(callback, false); return; }
        const QPointer<FrameReceiptProbe> guard(this);
        const auto admission=documentAdmission(broker);
        broker->revalidateDocument(*receipt_, [guard, callback](bool valid) {
            if (guard) guard->deliver(callback, valid);
        },1000,admission?std::optional<FerricFrameBroker::DocumentWork>({admission,QStringLiteral("validation")}):std::nullopt);
    }
    Q_INVOKABLE void captureComposed(QObject *object, const QJSValue &callback)
    {
        auto *broker=qobject_cast<FerricHintFrameBroker *>(object);
        if(!broker||!receipt_){deliver(callback,QVariantMap{{"ok",false}});return;}
        const auto original=*receipt_;receipt_.reset();fragments_.reset();
        const QPointer<FrameReceiptProbe> guard(this);
        broker->captureComposedAncestry(original,[guard,callback](std::optional<FerricFrameBroker::DocumentReceipt> receipt){
            if(!guard)return;
            guard->receipt_=std::move(receipt);
            QVariantMap result{{"ok",guard->receipt_.has_value()}};
            if(guard->receipt_){
                const auto &value=*guard->receipt_;const auto &leaf=value.documents().last();
                result.insert("native_depth",value.documents().size()-1);
                result.insert("composed_depth",value.composedDepth()?QVariant(static_cast<int>(*value.composedDepth())):QVariant());
                result.insert("epoch",leaf.epoch);result.insert("origin",leaf.origin);result.insert("owner",leaf.owner);
            }
            guard->deliver(callback,result);
        },1000,documentAdmission(broker));
    }
    Q_INVOKABLE void validateFocused(QObject *object, const QJSValue &callback)
    {
        auto *broker = qobject_cast<FerricHintFrameBroker *>(object);
        if (!broker || !receipt_) { deliver(callback, false); return; }
        const QPointer<FrameReceiptProbe> guard(this);
        broker->revalidateFocusedDocument(*receipt_, [guard, callback](bool valid) {
            if (guard) guard->deliver(callback, valid);
        });
    }
    Q_INVOKABLE void validateDocuments(QObject *object, const QString &batch, const QString &stamp,
                                       quint64 revision, const QString &documents,
                                       const QJSValue &callback)
    {
        auto *broker = qobject_cast<FerricHintFrameBroker *>(object);
        if (!broker || !receipt_) { deliver(callback, false); return; }
        const QPointer<FrameReceiptProbe> guard(this);
        broker->validateWorldDocuments(*receipt_, batch, stamp, revision, documents,
            [guard, callback](bool valid) { if (guard) guard->deliver(callback, valid); });
    }
    Q_INVOKABLE void validateFocusedDocuments(QObject *object, QObject *selectedObject,
                                       const QString &batch, const QString &stamp, quint64 revision,
                                       const QString &documents, const QVariantList &required,
                                       const QJSValue &callback)
    {
        auto *broker=qobject_cast<FerricHintFrameBroker *>(object);
        auto *selected=qobject_cast<FrameReceiptProbe *>(selectedObject);
        if(!broker||!receipt_||!selected||!selected->receipt_||required.size()>80){deliver(callback,false);return;}
        QList<quint32> ids;
        for(const auto &value:required){
            const auto type=value.metaType().id();
            if(type!=QMetaType::Double&&type!=QMetaType::Int&&type!=QMetaType::UInt
                &&type!=QMetaType::LongLong&&type!=QMetaType::ULongLong){deliver(callback,false);return;}
            bool ok=false;
            const auto id=value.toDouble(&ok);
            if(!ok||!std::isfinite(id)||id<1||id>4294967295.0||std::floor(id)!=id){deliver(callback,false);return;}
            ids.append(static_cast<quint32>(id));
        }
        const QPointer<FrameReceiptProbe> guard(this);
        broker->validateFocusedWorldDocuments(*receipt_,{*selected->receipt_,ids},batch,stamp,revision,documents,
            [guard,callback](bool valid){if(guard)guard->deliver(callback,valid);});
    }
    Q_INVOKABLE void validateObjects(QObject *object, QObject *selectedObject,
                                       const QString &batch, const QString &stamp, quint64 revision,
                                       const QString &annotation, const QString &scope, const QVariantList &required,
                                       const QJSValue &callback)
    {
        auto *broker=qobject_cast<FerricHintFrameBroker *>(object);
        auto *selected=qobject_cast<FrameReceiptProbe *>(selectedObject);
        if(!broker||!receipt_||(selectedObject&&(!selected||!selected->receipt_))
            ||required.size()>80||(!selectedObject&&!required.isEmpty())) {deliver(callback,false);return;}
        QList<quint32> ids;
        for(const auto &value:required){
            const auto type=value.metaType().id();
            if(type!=QMetaType::Double&&type!=QMetaType::Int&&type!=QMetaType::UInt
                &&type!=QMetaType::LongLong&&type!=QMetaType::ULongLong){deliver(callback,false);return;}
            bool ok=false;const auto id=value.toDouble(&ok);
            if(!ok||!std::isfinite(id)||id<1||id>4294967295.0||std::floor(id)!=id){deliver(callback,false);return;}
            ids.append(static_cast<quint32>(id));
        }
        std::optional<FerricFrameBroker::FocusedDocuments> focused;
        if(selected)focused.emplace(FerricFrameBroker::FocusedDocuments{*selected->receipt_,ids});
        const QPointer<FrameReceiptProbe> guard(this);
        broker->validateWorldObjects(*receipt_,std::move(focused),batch,stamp,revision,annotation,scope,
            [guard,callback](bool valid){if(guard)guard->deliver(callback,valid);});
    }
    // Test-only native seam for binding a graph to an independently supplied
    // original plan. Production obtains this text from its retained request.
    Q_INVOKABLE void validatePlannedObjects(QObject *object, const QString &batch,
                                       const QString &stamp, quint64 revision,
                                       const QString &annotation, const QString &originalPlan,
                                       const QJSValue &callback)
    {
        auto *broker=qobject_cast<FerricHintFrameBroker *>(object);
        if(!broker||!receipt_||originalPlan.isEmpty()){deliver(callback,false);return;}
        const QPointer<FrameReceiptProbe> guard(this);
        broker->validateWorldObjects(*receipt_,std::nullopt,batch,stamp,revision,annotation,QStringLiteral("null"),
            [guard,callback](bool valid){if(guard)guard->deliver(callback,valid);},1000,std::nullopt,originalPlan);
    }
    Q_INVOKABLE void validateDebitedObjects(QObject *object, const QString &batch,
                                       const QString &stamp, quint64 revision,
                                       const QString &annotation, const QString &originalPlan,
                                       quint32 reported, quint32 granted, const QJSValue &callback)
    {
        auto *broker=qobject_cast<FerricHintFrameBroker *>(object);
        if(!broker||!receipt_||originalPlan.isEmpty()){deliver(callback,false);return;}
        const QPointer<FrameReceiptProbe> guard(this);
        broker->validateWorldObjects(*receipt_,std::nullopt,batch,stamp,revision,annotation,QStringLiteral("null"),
            [guard,callback](bool valid){if(guard)guard->deliver(callback,valid);},1000,std::nullopt,originalPlan,
            FerricFrameBroker::AnnotationDebit{reported,granted});
    }
    Q_INVOKABLE void captureFragments(QObject *object, const QString &batch,
                                       const QString &stamp, quint64 revision,
                                       const QString &annotation, const QString &originalPlan, const QString &ordinaryGeometry,
                                       quint32 reported, quint32 granted, double width, double height,
                                       const QJSValue &callback)
    {
        fragments_.reset();
        auto *broker=qobject_cast<FerricHintFrameBroker *>(object);
        if(!broker||!receipt_){deliver(callback,false);return;}
        const QPointer<FrameReceiptProbe> guard(this);
        FerricFrameBroker::FragmentRequest request{*receipt_,std::nullopt,batch,stamp,revision,
            annotation,QStringLiteral("null"),originalPlan,ordinaryGeometry,{reported,granted},QSizeF(width,height)};
        if(receipt_->documents().size()>1) {
            QList<quint32> required;
            const auto documents=QJsonDocument::fromJson(annotation.toUtf8()).object().value("documents").toArray();
            for(const auto &value:documents) {
                const auto document=value.toObject();
                if(document.value("frame_path").toString()==QStringLiteral("0"))
                    required.append(static_cast<quint32>(document.value("id").toInteger()));
            }
            request.focused.emplace(FerricFrameBroker::FocusedDocuments{*receipt_,required});
        }
        request.documentAdmission=documentAdmission(broker);
        // Test-only carrier keeps this probe's public ABI usable with pinned
        // older runners. Production obtains this input directly from Rust.
        const auto testPlan=QJsonDocument::fromJson(originalPlan.toUtf8()).object();
        const auto input=testPlan.value("__test_ordinary_input");
        if(!input.isUndefined()){
            if(!input.isObject()){deliver(callback,false);return;}
            request.ordinaryInput=QString::fromUtf8(QJsonDocument(input.toObject()).toJson(QJsonDocument::Compact));
        }
        const auto scope=testPlan.value("__test_original_scope");
        if(!scope.isUndefined()&&!scope.isNull()){
            if(!scope.isObject()){deliver(callback,false);return;}
            request.scope=QString::fromUtf8(QJsonDocument(scope.toObject()).toJson(QJsonDocument::Compact));
        }
        if (graphAccounting_) {
            if(graphAccounting_->interruption==QStringLiteral("destroy"))deletionTarget_=broker;
            request.graphAdmission = [state=graphAccounting_, broker=QPointer<FerricHintFrameBroker>(broker)](quint8 pass) {
                state->events.append(QVariantMap{{"kind", "reserve"}, {"pass", pass}});
                if (state->refusedPass == pass || state->issued.contains(pass))
                    return std::shared_ptr<FerricHintWorkTicket>();
                state->issued.insert(pass);
                auto ticket=std::make_shared<FerricHintWorkTicket>([state,pass](FerricHintWorkTicket::Outcome outcome,quint32 steps) {
                    const auto name=outcome==FerricHintWorkTicket::Outcome::Known?QStringLiteral("known"):
                        outcome==FerricHintWorkTicket::Outcome::NoScript?QStringLiteral("no-script"):QStringLiteral("unknown");
                    state->events.append(QVariantMap{{"kind", "complete"}, {"pass", pass}, {"outcome", name}, {"steps", steps}});
                    return state->interruption!=QStringLiteral("reject-settlement");
                });
                if (state->interruption==QStringLiteral("expire")) QThread::msleep(1100);
                if (state->interruption==QStringLiteral("clear") && broker)
                    QTimer::singleShot(0, broker.data(), [broker] { if(broker)broker->clear(); });
                if (state->interruption==QStringLiteral("destroy") && broker)
                    QTimer::singleShot(0, broker.data(), [broker] { if(broker)delete broker.data(); });
                return ticket;
            };
        }
        broker->captureFragmentCatalogue(std::move(request),[guard,callback](std::optional<FerricFrameBroker::FragmentCatalogue> catalogue) {
            if(!guard)return;
            guard->fragments_=std::move(catalogue);
            guard->deliver(callback,guard->fragments_.has_value());
        });
    }
    Q_INVOKABLE void revalidateFragments(QObject *object, const QJSValue &callback)
    {
        auto *broker=qobject_cast<FerricHintFrameBroker *>(object);
        if(!broker||!fragments_){deliver(callback,false);return;}
        const QPointer<FrameReceiptProbe> guard(this);
        broker->revalidateFragmentCatalogue(*fragments_,[guard,callback](bool valid){if(guard)guard->deliver(callback,valid);});
    }
    Q_INVOKABLE void fragmentGeometry(QObject *object, const QJSValue &callback)
    {
        auto *broker=qobject_cast<FerricHintFrameBroker *>(object);
        if(!broker||!fragments_){deliver(callback,QVariant());return;}
        const QPointer<FrameReceiptProbe> guard(this);
        broker->captureFragmentGeometry(*fragments_,[guard,callback](std::optional<QJsonObject> geometry) {
            if(guard)guard->deliver(callback,geometry?QVariant(geometry->toVariantMap()):QVariant());
        });
    }
    Q_INVOKABLE void projectFragments(QObject *object, const QJSValue &callback)
    {
        auto *broker=qobject_cast<FerricHintFrameBroker *>(object);
        if(!broker||!fragments_){deliver(callback,QVariant());return;}
        const QPointer<FrameReceiptProbe> guard(this);
        broker->projectFragmentCatalogue(*fragments_,[guard,callback](std::optional<QJsonObject> geometry) {
            if(guard)guard->deliver(callback,geometry?QVariant(geometry->toVariantMap()):QVariant());
        });
    }
    Q_INVOKABLE void validateEnteredObjects(QObject *object, QObject *enteredObject,
                                       const QString &batch, const QString &stamp, quint64 revision,
                                       const QString &annotation, const QString &scope,
                                       const QJSValue &callback)
    {
        auto *broker=qobject_cast<FerricHintFrameBroker *>(object);
        auto *entered=qobject_cast<FrameReceiptProbe *>(enteredObject);
        if(!broker||!receipt_||!entered||!entered->receipt_){deliver(callback,false);return;}
        const QPointer<FrameReceiptProbe> guard(this);
        broker->validateWorldObjects(*receipt_,std::nullopt,batch,stamp,revision,annotation,scope,
            [guard,callback](bool valid){if(guard)guard->deliver(callback,valid);},1000,
            FerricFrameBroker::EnteredDocument{*entered->receipt_});
    }
private:
    struct DocumentAccounting {
        QVariantList events;
        QSet<QString> issued;
        int refusedDocument = -1, refusedPass = -1;
        QString interruption;
    };
    FerricHintDocumentAdmission documentAdmission(FerricHintFrameBroker *broker) {
        if(!documentAccounting_)return {};
        if(documentAccounting_->interruption==QStringLiteral("destroy"))deletionTarget_=broker;
        return [state=documentAccounting_,broker=QPointer<FerricHintFrameBroker>(broker)](const QString &checkpoint,quint8 document,quint8 pass) {
            const auto key=checkpoint+QStringLiteral("/")+QString::number(document)+QStringLiteral("/")+QString::number(pass);
            state->events.append(QVariantMap{{"kind","reserve"},{"checkpoint",checkpoint},{"document",document},{"pass",pass}});
            if(state->issued.contains(key)||(state->refusedDocument==document&&state->refusedPass==pass))
                return std::shared_ptr<FerricHintWorkTicket>();
            state->issued.insert(key);
            auto ticket=std::make_shared<FerricHintWorkTicket>([state,checkpoint,document,pass](FerricHintWorkTicket::Outcome outcome,quint32 units) {
                const auto name=outcome==FerricHintWorkTicket::Outcome::Known?QStringLiteral("known"):
                    outcome==FerricHintWorkTicket::Outcome::NoScript?QStringLiteral("no-script"):QStringLiteral("unknown");
                state->events.append(QVariantMap{{"kind","complete"},{"checkpoint",checkpoint},{"document",document},
                    {"pass",pass},{"outcome",name},{"units",units}});
                return state->interruption!=QStringLiteral("reject-settlement");
            },FerricHintWorkTicket::Counter::Document);
            if(state->interruption==QStringLiteral("expire"))QThread::msleep(1100);
            if(broker&&state->interruption==QStringLiteral("clear"))
                QTimer::singleShot(0,broker.data(),[broker]{if(broker)broker->clear();});
            if(broker&&state->interruption==QStringLiteral("destroy"))
                QTimer::singleShot(0,broker.data(),[broker]{if(broker)delete broker.data();});
            return ticket;
        };
    }
    struct GraphAccounting {
        QVariantList events;
        QSet<quint8> issued;
        int refusedPass = -1;
        QString interruption;
    };
    void deliver(const QJSValue &callback, const QVariant &value)
    {
        if (auto *engine = qjsEngine(this); engine && callback.isCallable())
            callback.call({engine->toScriptValue(value)});
    }
    std::optional<FerricFrameBroker::DocumentReceipt> receipt_;
    std::optional<FerricFrameBroker::FragmentCatalogue> fragments_;
    QPointer<QObject> deletionTarget_;
    std::shared_ptr<GraphAccounting> graphAccounting_;
    std::shared_ptr<DocumentAccounting> documentAccounting_;
};

class PerformanceClock final : public QObject
{
    Q_OBJECT

public:
    PerformanceClock() { timer_.start(); }

    Q_INVOKABLE qint64 nowNsecs() const { return timer_.nsecsElapsed(); }

private:
    QElapsedTimer timer_;
};

static QObject *createPerformanceClock(QQmlEngine *, QJSEngine *)
{
    return new PerformanceClock;
}

class TestSetup final : public QObject
{
    Q_OBJECT
public:
    Q_INVOKABLE void sendInputMethod(QObject *receiver, const QString &preedit, const QString &commit)
    {
        if (!receiver) return;
        QInputMethodEvent event(preedit, {});
        event.setCommitString(commit);
        QCoreApplication::sendEvent(receiver, &event);
    }
public slots:
    void qmlEngineAvailable(QQmlEngine *engine)
    {
        engine->rootContext()->setContextProperty(
            "testArtifactDirectory", qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR"));
        engine->rootContext()->setContextProperty("testEmptySettingsOptions", QStringList{});
        engine->rootContext()->setContextProperty("testInputMethod", this);
    }
};

int main(int argc, char **argv)
{
    // WebEngine must configure shared OpenGL contexts before Qt Quick Test
    // constructs its QGuiApplication, including when tests run offscreen.
    QtWebEngineQuick::initialize();
    QVector<QByteArray> arguments;
    for (int index = 0; index < argc; ++index) arguments.append(QByteArray(argv[index]));
    auto application = ferric_browser_gui_application(arguments);
    if (!application) return 1;
    qInfo("Ferric test runtime: Qt %s; WebEngine %s; Chromium %s; security patch %s; QPA %s",
          qVersion(), qWebEngineVersion(), qWebEngineChromiumVersion(),
          qWebEngineChromiumSecurityPatchVersion(),
          qPrintable(QGuiApplication::platformName()));
    qmlRegisterType<FerricBrowserKeyRouter>("io.github.ferricbrowser", 1, 0, "BrowserKeyRouter");
    qmlRegisterType<FerricScriptRunner>("Ferric.Testing", 1, 0, "ScriptRunner");
    qmlRegisterType<FerricScriptRunner>("io.github.ferricbrowser", 1, 0, "FerricScriptRunner");
    qmlRegisterType<FerricBrowserSettingsModel>("io.github.ferricbrowser", 1, 0, "FerricSettingsModel");
    qmlRegisterType<FerricNativeFileChooser>("io.github.ferricbrowser", 1, 0, "NativeFileChooser");
    qmlRegisterType<FerricDownloadSource>("io.github.ferricbrowser", 1, 0, "FerricDownloadSource");
    qmlRegisterType<FerricHintFrameBroker>("io.github.ferricbrowser", 1, 0, "FerricHintFrameBroker");
    qmlRegisterType<FrameReceiptProbe>("Ferric.Testing", 1, 0, "FrameReceiptProbe");
    qmlRegisterSingletonType<PerformanceClock>("Ferric.Testing", 1, 0, "PerformanceClock",
                                               createPerformanceClock);
    TestSetup setup;
    return quick_test_main_with_setup(argc, argv, "ferric_qml_tests", nullptr, &setup);
}

#include "main.moc"
