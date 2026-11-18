#include "hint_request_adapter.h"
#include "ferric-browser-engine-qt/src/bridge.cxxqt.h"
#include <QDeadlineTimer>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMetaProperty>
#include <QJSEngine>
#include <QThread>
#include <QUuid>
#include <memory>
#include <limits>
#include <utility>
#include <algorithm>
#include <cmath>

namespace {
QObject *objectProperty(QObject *object, const char *name) {
    const auto value = object->property(name);
    if (auto *result = value.value<QObject *>()) return result;
    return value.value<QJSValue>().toQObject();
}
bool validScopedOwner(const QString &owner) {
    if(owner.isEmpty())return true;
    if(owner.size()!=42||!owner.startsWith(QStringLiteral("frame-")))return false;
    const auto id=QUuid::fromString(owner.mid(6));
    return !id.isNull()&&id.toString(QUuid::WithoutBraces)==owner.mid(6);
}
struct HintRequestHandle {
    explicit HintRequestHandle(rust::Box<ferric::hints::HintRequestCapture> value)
        : captured(std::move(value)) {}
    rust::Box<ferric::hints::HintRequestCapture> captured;
    QPointer<FerricHintRequestAdapter> adapter;
    QPointer<FerricProfileLifetime> lifetime;
    QPointer<QQuickItem> view;
    QPointer<FerricHintFrameBroker> broker;
    FerricProfileLifetime::CurrentView owner;
    QString tab, prompt, scopedOwner;
    QPointer<QObject> workspace, provider;
    int providerEpoch=-1, collectionEpoch=-1;
    bool collection=false;
    int index = -1;
    quint64 configurationGeneration = 0;
    bool bindingStarted = false;
    bool observationStarted = false;
    bool documentValidationStarted = false;
    bool documentValidated = false;
    bool collectionFinalized = false;
    bool preview = false;
    bool selectionCheckStarted = false;
    bool hasFocusedReceiver = false;
    bool focusedInTop = false;
    std::optional<FerricFrameBroker::DocumentReceipt> document;
    std::optional<FerricFrameBroker::DocumentReceipt> topDocument;
    std::optional<FerricFrameBroker::FragmentCatalogue> topFragments, selectedFragments;
    std::optional<QJsonObject> topGeometry, selectedGeometry;

    bool baseCurrent() const {
        const auto alive = [this] {
            return adapter && lifetime && view && broker && owner.ui && owner.host && owner.profile
                && QThread::currentThread() == adapter->thread()
                && adapter->configurationGeneration() == configurationGeneration
                && adapter->lifetime() == lifetime;
        };
        if (!alive()) return false;
        const bool matches = lifetime->matchesCurrentView(view, owner);
        return matches && alive();
    }
    bool issuerCurrent(bool requireView=false) const {
        if(!collection)return true;
        if(!workspace||!provider||!baseCurrent())return false;
        const auto actualWorkspace=objectProperty(owner.host,preview?"sitePolicyPreviewWorkspace":"hintWorkspace");
        if(!baseCurrent()||actualWorkspace!=workspace)return false;
        const auto actualProvider=objectProperty(workspace,"hintFrameProvider");
        if(!baseCurrent()||actualProvider!=provider||broker->parent()!=provider)return false;
        const auto host=objectProperty(workspace,"hostWindow");
        if(!baseCurrent()||host!=owner.host)return false;
        const auto ui=objectProperty(workspace,"browserUi");
        if(!baseCurrent()||ui!=owner.ui)return false;
        const auto active=objectProperty(workspace,"activeView");
        if(!baseCurrent()||active!=view)return false;
        const auto issuer=objectProperty(provider,"browserWindow");
        if(!baseCurrent()||issuer!=workspace)return false;
        const auto currentView=objectProperty(provider,"view");
        if(!baseCurrent()||(currentView&&currentView!=view)
            ||((requireView||document||!scopedOwner.isEmpty())&&currentView!=view))return false;
        const auto epoch=provider->property("epoch");
        if(!baseCurrent()||epoch.metaType().id()!=QMetaType::Int||epoch.toInt()!=providerEpoch)return false;
        const auto incarnation=workspace->property("hintCollectionEpoch");
        if(!baseCurrent()||incarnation.metaType().id()!=QMetaType::Int||incarnation.toInt()!=collectionEpoch)return false;
        const auto scope=provider->property("scopedOwner");
        return baseCurrent()&&scope.metaType().id()==QMetaType::QString&&scope.toString()==scopedOwner;
    }
    bool captureIssuer() {
        if(!baseCurrent())return false;
        workspace=objectProperty(owner.host,preview?"sitePolicyPreviewWorkspace":"hintWorkspace");
        if(!baseCurrent()||!workspace)return false;
        provider=objectProperty(workspace,"hintFrameProvider");
        if(!baseCurrent()||!provider||provider->thread()!=adapter->thread())return false;
        const auto brokers=provider->findChildren<FerricHintFrameBroker *>();
        if(brokers.size()!=1||brokers.first()!=broker)return false;
        const auto epoch=provider->property("epoch");
        if(!baseCurrent()||epoch.metaType().id()!=QMetaType::Int||epoch.toInt()<0)return false;
        providerEpoch=epoch.toInt();
        const auto incarnation=workspace->property("hintCollectionEpoch");
        if(!baseCurrent()||incarnation.metaType().id()!=QMetaType::Int||incarnation.toInt()<0)return false;
        collectionEpoch=incarnation.toInt();collection=true;
        return issuerCurrent();
    }
    bool current() const {
        if (!baseCurrent()) return false;
        // Every QML routing property read may reenter. Corroborate the actual
        // registered owner again before reading or using another Qt object.
        const auto viewUi = objectProperty(view, "viewUi");
        if (!baseCurrent() || viewUi != owner.ui) return false;
        const auto host = objectProperty(view, "viewHost");
        if (!baseCurrent() || host != owner.host) return false;
        const auto currentTab = view->property("stableTabId").toString();
        if (!baseCurrent() || currentTab != tab) return false;
        const auto currentPrompt = view->property("promptLeaseIdentity").toString();
        if (!baseCurrent() || currentPrompt != prompt) return false;
        const auto currentIndex = view->property("tabIndex");
        if (!baseCurrent() || currentIndex.metaType().id() != QMetaType::Int || currentIndex.toInt() != index) return false;
        const auto retired = view->property("viewRetired");
        if (!baseCurrent() || retired.metaType().id() != QMetaType::Bool || retired.toBool()) return false;
        const auto urlProperty = view->metaObject()->property(view->metaObject()->indexOfProperty("url"));
        if (!urlProperty.isValid() || !urlProperty.isFinal()) return false;
        const auto url = urlProperty.read(view).toUrl();
        if (!baseCurrent()) return false;
        auto *ui = qobject_cast<BrowserUi *>(owner.ui.data());
        const QPointer<BrowserUi> policyUi(qobject_cast<BrowserUi *>(lifetime->policyUi()));
        if (!ui || !policyUi || !baseCurrent()) return false;
        if (!policyUi || lifetime->policyUi() != policyUi) return false;
        const auto profile = ui->page_host_profile_id();
        return !profile.isEmpty() && profile == policyUi->page_host_profile_id() && baseCurrent()
            &&issuerCurrent()&&baseCurrent()&&policyUi&&lifetime->policyUi()==policyUi
            && ferric::hints::hint_request_is_current(*ui, *captured, tab, index, prompt, url.toString(QUrl::FullyEncoded));
    }
    bool matchesMainFrame(const FerricFrameBroker::DocumentReceipt &receipt) const {
        if (!current() || receipt.documents().isEmpty() || !broker->documentAssociationIsCurrent(receipt)) return false;
        const auto property = view->metaObject()->property(view->metaObject()->indexOfProperty("mainFrame"));
        if (!property.isValid() || !property.isFinal()) return false;
        const auto frame = property.read(view).value<QWebEngineFrame>();
        if (!current() || !frame.isValid() || frame != receipt.documents().first().frame
            || !broker->documentAssociationIsCurrent(receipt)) return false;
        // view.url can already name the provisional destination while this
        // native root still contains the previous document. Bind the original
        // policy to that captured native root URL, never a foreign leaf URL.
        auto *ui = qobject_cast<BrowserUi *>(owner.ui.data());
        return ui && ferric::hints::hint_request_is_current(*ui, *captured, tab, index, prompt,
            receipt.documents().first().url.toString(QUrl::FullyEncoded));
    }
    bool finalizeDocuments(const FerricFrameBroker::DocumentReceipt &receipt,
                           bool ordinary, const QDeadlineTimer *deadline) {
        if((deadline&&deadline->hasExpired())||!matchesMainFrame(receipt)||!issuerCurrent(true))return false;
        auto top=broker->topDocumentReceipt(receipt);
        if(!top||!matchesMainFrame(*top))return false;
        const auto &documents=receipt.documents();
        const auto &root=documents.first(),&leaf=documents.last();
        const bool focused=documents.size()>1;
        const auto composed=receipt.composedDepth();
        if(focused&&(!composed||*composed<documents.size()-1||*composed>8))return false;
        const bool inTop=focused&&root.origin!=QStringLiteral("null")
            &&std::all_of(documents.cbegin(),documents.cend(),[&root](const auto &value){return value.origin==root.origin;});
        // Same-origin entry keeps Top's namespace; its exact native Document
        // is carried separately into the producer and every object seal.
        if(!scopedOwner.isEmpty()&&(!focused||leaf.owner!=scopedOwner))return false;
        if(!ordinary) {
            QJsonArray structural;
            if(focused&&!inTop) {
                bool accessible=root.origin!=QStringLiteral("null");
                for(qsizetype index=1;index+1<documents.size();++index) {
                    accessible=accessible&&documents[index].origin==root.origin;
                    if(!accessible)structural.append(documents[index].owner);
                }
            }
            QJsonValue receiver(QJsonValue::Null);
            if(focused)receiver=QJsonObject{{"owner",inTop?QString():leaf.owner},
                {"origin",leaf.origin},{"depth",static_cast<int>(documents.size()-1)},
                {"composed_depth",static_cast<int>(*composed)}};
            const QJsonObject roles{{"structural_owners",structural},{"focused_receiver",receiver}};
            auto *ui=qobject_cast<BrowserUi *>(owner.ui.data());
            if(!ui||!ferric::hints::finalize_hint_request(*ui,*captured,
                QString::fromUtf8(QJsonDocument(roles).toJson(QJsonDocument::Compact))))return false;
        }
        if((deadline&&deadline->hasExpired())||!matchesMainFrame(receipt))return false;
        auto *ui=qobject_cast<BrowserUi *>(owner.ui.data());
        const QJsonObject binding{{"owner",focused&&!inTop?leaf.owner:QString()},
            {"depth",static_cast<int>(documents.size()-1)}};
        if(!ui||!ferric::hints::commit_hint_document_binding(*ui,*captured,
            QString::fromUtf8(QJsonDocument(binding).toJson(QJsonDocument::Compact))))return false;
        if((deadline&&deadline->hasExpired())||!matchesMainFrame(receipt))return false;
        document=receipt;topDocument=std::move(top);
        hasFocusedReceiver=focused;focusedInTop=inTop;
        return true;
    }
};
using SharedHintRequest = std::shared_ptr<HintRequestHandle>;

// Context is local to this original policy slot. The retained catalogue must
// never keep its handle alive through a callback ownership cycle.
using NativeAdmission = std::function<std::shared_ptr<FerricHintWorkTicket>(const QJsonObject &)>;
NativeAdmission nativeAdmissionFor(const SharedHintRequest &handle,const QString &phase,const QString &stage,
                                    const QString &owner,FerricHintWorkTicket::Counter counter) {
    return [weak=std::weak_ptr<HintRequestHandle>(handle),phase,stage,owner,counter](const QJsonObject &operation)->std::shared_ptr<FerricHintWorkTicket> {
        const auto original=weak.lock();
        if(!original||!original->current())return {};
        auto *ui=qobject_cast<BrowserUi *>(original->owner.ui.data());
        bool validStage=false;
        const auto number=stage.toUInt(&validStage);
        const bool policy=phase==QStringLiteral("policy");
        if(!ui||!validStage||QString::number(number)!=stage||(policy?(number<1||number>4):number!=0)
            ||(!policy&&phase!=QStringLiteral("preflight")&&phase!=QStringLiteral("validation")))return {};
        auto bindingOwner=owner;
        const auto checkpoint=operation.value("checkpoint").toString();
        if(phase==QStringLiteral("preflight")&&(checkpoint==QStringLiteral("bind-top")||checkpoint==QStringLiteral("fallback-top")))
            bindingOwner.clear();
        const QJsonObject binding{{"phase",phase},{"stage",static_cast<int>(number)},{"owner",bindingOwner}};
        const bool graph=counter==FerricHintWorkTicket::Counter::Graph;
        const QJsonObject limit{{"inspections",graph?0:1},{"verification_steps",graph?static_cast<int>(FerricHintWorkTicket::limit):0},
            {"annotation_bytes",0}};
        const auto compact=[](const QJsonObject &value) {
            return QString::fromUtf8(QJsonDocument(value).toJson(QJsonDocument::Compact));
        };
        const QJsonObject specification{{"operation",operation},{"binding",binding},{"limit",limit}};
        const auto granted=ferric::hints::reserve_hint_native_work(*ui,*original->captured,compact(specification));
        if(granted.isEmpty()||granted.size()>2048)return {};
        const auto grant=QJsonDocument::fromJson(granted.toUtf8());
        const auto token=grant.object().value("token");
        const auto grantLimit=grant.object().value("limit");
        if(!grant.isObject()||grant.object().size()!=2||!token.isObject()||token.toObject().isEmpty()
            ||!grantLimit.isObject()||grantLimit.toObject()!=limit) {
            if(token.isObject())ferric::hints::complete_hint_native_work(*ui,*original->captured,compact(token.toObject()),
                compact(binding),QStringLiteral("{}"));
            return {};
        }
        return std::make_shared<FerricHintWorkTicket>(
            [weak,token=compact(token.toObject()),binding=compact(binding),compact,graph](FerricHintWorkTicket::Outcome outcome,quint32 units) {
                const auto original=weak.lock();
                if(!original)return false;
                auto *ui=qobject_cast<BrowserUi *>(original->owner.ui.data());
                if(!ui||QThread::currentThread()!=ui->thread())return false;
                const auto kind=outcome==FerricHintWorkTicket::Outcome::Known?QStringLiteral("known"):
                    outcome==FerricHintWorkTicket::Outcome::NoScript?QStringLiteral("no-script"):QStringLiteral("unknown");
                const QJsonObject report{{"outcome",kind},{"spent",QJsonObject{{"inspections",graph?0:static_cast<int>(units)},
                    {"verification_steps",graph?static_cast<int>(units):0},{"annotation_bytes",0}}}};
                return ferric::hints::complete_hint_native_work(*ui,*original->captured,token,binding,compact(report));
            },counter);
    };
}
FerricHintGraphAdmission graphAdmissionFor(const SharedHintRequest &handle,const QString &stage,const QString &owner) {
    return [admit=nativeAdmissionFor(handle,QStringLiteral("policy"),stage,owner,FerricHintWorkTicket::Counter::Graph)](quint8 pass) {
        return admit(QJsonObject{{"kind","graph"},{"pass",pass}});
    };
}
FerricHintDocumentAdmission documentAdmissionFor(const SharedHintRequest &handle,const QString &phase,
                                                 const QString &stage,const QString &owner) {
    return [admit=nativeAdmissionFor(handle,phase,stage,owner,FerricHintWorkTicket::Counter::Document)](const QString &checkpoint,quint8 document,quint8 pass) {
        QJsonObject operation{{"kind",checkpoint==QStringLiteral("validation")?"document-validation":"document-facts"},
            {"document",document},{"pass",pass}};
        if(checkpoint!=QStringLiteral("validation"))operation.insert("checkpoint",checkpoint);
        return admit(operation);
    };
}

}
Q_DECLARE_METATYPE(SharedHintRequest)

void FerricHintRequestAdapter::setLifetime(FerricProfileLifetime *value) {
    if (lifetime_ == value || (value && value->thread() != thread())) return;
    if (configurationGeneration_ == std::numeric_limits<quint64>::max()) lifetime_ = nullptr;
    else { ++configurationGeneration_; lifetime_ = value; }
    emit lifetimeChanged();
}

void FerricHintRequestAdapter::deliver(const QJSValue &callback, const QVariant &value) {
    if (auto *engine = qjsEngine(this); engine && callback.isCallable())
        callback.call({engine->toScriptValue(value)});
}

void FerricHintRequestAdapter::capture(QQuickItem *view, FerricHintFrameBroker *broker,
                                      const QString &owner, const QJSValue &callback) {
    const QPointer<FerricHintRequestAdapter> self(this);
    const auto handle = begin(view, broker);
    if (self) self->bindDocuments(handle, owner, callback);
}

QVariant FerricHintRequestAdapter::begin(QQuickItem *view, FerricHintFrameBroker *broker) {
    QString state;
    return beginImpl(view,broker,{},false,state);
}

QVariantMap FerricHintRequestAdapter::beginCollection(QQuickItem *view, FerricHintFrameBroker *broker,
                                                    const QString &scopedOwner) {
    QString state;
    const auto request=beginImpl(view,broker,scopedOwner,true,state);
    QVariantMap result{{QStringLiteral("state"),state}};
    if(request.isValid())result.insert(QStringLiteral("request"),request);
    return result;
}

QString FerricHintRequestAdapter::collectionState(const QVariant &value) const {
    if(value.metaType()!=QMetaType::fromType<SharedHintRequest>())return QStringLiteral("closed");
    const auto handle=value.value<SharedHintRequest>();
    if(!handle||handle->adapter!=this||!handle->current())return QStringLiteral("closed");
    auto *ui=qobject_cast<BrowserUi *>(handle->owner.ui.data());
    return ui?ferric::hints::hint_request_classification(*ui,*handle->captured):QStringLiteral("closed");
}

QVariantMap FerricHintRequestAdapter::beginPreview(QQuickItem *view, FerricHintFrameBroker *broker,
                                                  const QString &preview, const QString &family) {
    QString state=QStringLiteral("closed");
    if(preview.isEmpty())return {{QStringLiteral("state"),state}};
    const auto request=beginImpl(view,broker,{},true,state,preview,family);
    QVariantMap result{{QStringLiteral("state"),state}};
    if(request.isValid())result.insert(QStringLiteral("request"),request);
    return result;
}

QString FerricHintRequestAdapter::publishPreview(const QVariant &value) {
    const auto rejected=QStringLiteral("{\"error\":\"stale-preview\"}");
    if(value.metaType()!=QMetaType::fromType<SharedHintRequest>())return rejected;
    const auto handle=value.value<SharedHintRequest>();
    if(!handle||!handle->preview||handle->adapter!=this||!handle->current())return rejected;
    auto *ui=qobject_cast<BrowserUi *>(handle->owner.ui.data());
    return ui?ferric::hints::publish_hint_collection(*ui,*handle->captured):rejected;
}

QVariantMap FerricHintRequestAdapter::beginCapture(QQuickItem *view, FerricHintFrameBroker *broker,
                                                  const QString &capture, const QString &family) {
    QString state=QStringLiteral("closed");
    if(capture.isEmpty())return {{QStringLiteral("state"),state}};
    const auto request=beginImpl(view,broker,{},true,state,capture,family);
    QVariantMap result{{QStringLiteral("state"),state}};
    if(request.isValid())result.insert(QStringLiteral("request"),request);
    return result;
}

QString FerricHintRequestAdapter::builderCandidate(const QVariant &value, const QString &label) const {
    const auto rejected=QStringLiteral("{\"error\":\"stale-capture\"}");
    if(value.metaType()!=QMetaType::fromType<SharedHintRequest>())return rejected;
    const auto handle=value.value<SharedHintRequest>();
    if(!handle||!handle->preview||handle->adapter!=this||!handle->current())return rejected;
    auto *ui=qobject_cast<BrowserUi *>(handle->owner.ui.data());
    return ui?ferric::hints::hint_builder_candidate(*ui,*handle->captured,label):rejected;
}

QVariant FerricHintRequestAdapter::beginImpl(QQuickItem *view, FerricHintFrameBroker *broker,
                                            const QString &scopedOwner, bool collection, QString &state,
                                            const QString &preview, const QString &family) {
    state=QStringLiteral("closed");
    const QPointer<FerricHintRequestAdapter> self(this);
    const QPointer<FerricProfileLifetime> lifetime(lifetime_);
    const QPointer<QQuickItem> item(view);
    const QPointer<FerricHintFrameBroker> nativeBroker(broker);
    const auto generation = configurationGeneration_;
    if (!lifetime || !item || !nativeBroker || QThread::currentThread() != thread()
        || item->thread() != thread() || broker->thread() != thread()
        || !item->inherits("QQuickWebEngineView")) return {};
    const auto ownerView = lifetime->currentView(item);
    if (!ownerView) return {};
    auto current = [&] {
        const auto alive = [&] {
            return self && lifetime && item && nativeBroker && self->lifetime() == lifetime
                && self->configurationGeneration() == generation;
        };
        if (!alive()) return false;
        const bool matches = lifetime->matchesCurrentView(item, *ownerView);
        return matches && alive();
    };
    if (!current()) return {};
    const auto tab = item->property("stableTabId").toString();
    if (!current()) return {};
    const auto prompt = item->property("promptLeaseIdentity").toString();
    if (!current()) return {};
    const auto index = item->property("tabIndex");
    if (!current()) return {};
    auto *ui = qobject_cast<BrowserUi *>(ownerView->ui.data());
    if (!ui || index.metaType().id() != QMetaType::Int) return {};
    // Freeze Rust ownership/policy BEFORE the first asynchronous document probe.
    auto captured = preview.isEmpty()
        ? ferric::hints::begin_hint_request(*ui,tab,index.toInt(),prompt,ownerView->incarnation)
        : ferric::hints::begin_hint_preview_request(*ui,tab,index.toInt(),prompt,ownerView->incarnation,preview,family);
    auto handle = std::make_shared<HintRequestHandle>(std::move(captured));
    handle->preview = !preview.isEmpty();
    handle->adapter = self; handle->lifetime = lifetime; handle->view = item;
    handle->broker = nativeBroker; handle->owner = *ownerView;
    handle->tab = tab; handle->prompt = prompt; handle->index = index.toInt();
    handle->configurationGeneration = generation;
    if(!validScopedOwner(scopedOwner))return {};
    handle->scopedOwner=scopedOwner;
    if(collection&&!handle->captureIssuer())return {};
    const auto classification=ferric::hints::hint_request_classification(*ui,*handle->captured);
    if(classification==QStringLiteral("pending")) {
        if(current()&&handle->baseCurrent())state=classification;
        return {};
    }
    if (!handle->current()) return {};
    if(classification!=QStringLiteral("ordinary")&&classification!=QStringLiteral("proof-ready"))return {};
    state=classification;
    return QVariant::fromValue(handle);
}

void FerricHintRequestAdapter::bindDocuments(const QVariant &value, const QString &owner, const QJSValue &callback) {
    const QDeadlineTimer deadline(1000, Qt::PreciseTimer);
    const QPointer<FerricHintRequestAdapter> self(this);
    if (value.metaType() != QMetaType::fromType<SharedHintRequest>()) { deliver(callback, {}); return; }
    const auto handle = value.value<SharedHintRequest>();
    if (!handle || handle->adapter != this || handle->bindingStarted) {
        if (self) self->deliver(callback, {});
        return;
    }
    handle->bindingStarted = true;
    if (!handle->current()||(!handle->scopedOwner.isEmpty()&&owner!=handle->scopedOwner)) {
        if (self) self->deliver(callback, {}); return;
    }
    handle->broker->captureDocument(owner, [handle, callback, deadline](std::optional<FerricFrameBroker::DocumentReceipt> receipt) {
        if (!handle->adapter) return;
        if (!receipt || !handle->matchesMainFrame(*receipt) || deadline.hasExpired()) {
            if (handle->adapter) handle->adapter->deliver(callback, {});
            return;
        }
        const auto captured = *receipt;
        const bool focused = captured.documents().size() > 1;
        const auto finish = [handle, callback, deadline](std::optional<FerricFrameBroker::DocumentReceipt> admitted) {
            if (!handle->adapter) return;
            if (!admitted || !handle->finalizeDocuments(*admitted,false,&deadline)) {
                if (handle->adapter) handle->adapter->deliver(callback, {});
                return;
            }
            handle->adapter->deliver(callback, QVariant::fromValue(handle));
        };
        if (focused) handle->broker->captureComposedAncestry(captured, finish, static_cast<int>(deadline.remainingTime()),
            documentAdmissionFor(handle,QStringLiteral("preflight"),QStringLiteral("0"),captured.documents().last().owner));
        else finish(captured);
    }, static_cast<int>(deadline.remainingTime()),FerricFrameBroker::DocumentWork{
        documentAdmissionFor(handle,QStringLiteral("preflight"),QStringLiteral("0"),owner),QStringLiteral("bind")});
}

void FerricHintRequestAdapter::bindCollection(const QVariant &value, const QString &topOwner,
                                             const QString &selectedOwner, const QJSValue &callback) {
    const QPointer<FerricHintRequestAdapter> self(this);
    if(value.metaType()!=QMetaType::fromType<SharedHintRequest>()){deliver(callback,{});return;}
    const auto handle=value.value<SharedHintRequest>();
    if(!handle||handle->adapter!=this||handle->bindingStarted){if(self)self->deliver(callback,{});return;}
    handle->bindingStarted=true;
    if(!handle->current()||topOwner.isEmpty()||!validScopedOwner(topOwner)
        ||(!handle->scopedOwner.isEmpty()&&selectedOwner!=handle->scopedOwner)) {
        if(self)self->deliver(callback,{});return;
    }
    const QDeadlineTimer topDeadline(1000,Qt::PreciseTimer);
    handle->broker->captureDocument(topOwner,[handle,selectedOwner,callback,topDeadline](std::optional<FerricFrameBroker::DocumentReceipt> top) {
        if(!handle->adapter)return;
        if(!top||top->documents().size()!=1||!top->documents().first().frame.isMainFrame()
            ||topDeadline.hasExpired()||!handle->matchesMainFrame(*top)) {
            if(handle->adapter)handle->adapter->deliver(callback,{});return;
        }
        const auto finish=[handle,callback](const FerricFrameBroker::DocumentReceipt &receipt,const QDeadlineTimer *deadline) {
            if(!handle->adapter)return;
            auto *ui=qobject_cast<BrowserUi *>(handle->owner.ui.data());
            const auto mode=ui?ferric::hints::hint_request_classification(*ui,*handle->captured):QStringLiteral("closed");
            const bool valid=(mode==QStringLiteral("ordinary")||mode==QStringLiteral("proof-ready"))
                &&handle->finalizeDocuments(receipt,mode==QStringLiteral("ordinary"),deadline);
            if(handle->adapter)handle->adapter->deliver(callback,valid?QVariant::fromValue(handle):QVariant());
        };
        if(selectedOwner.isEmpty()||selectedOwner==top->documents().first().owner) {
            if(!handle->scopedOwner.isEmpty()){handle->adapter->deliver(callback,{});return;}
            finish(*top,&topDeadline);return;
        }
        if(!validScopedOwner(selectedOwner)) {finish(*top,&topDeadline);return;}
        // Top and selected admission are separate bounded operations. Capture
        // and every focused-chain substep share this ONE receiver deadline.
        const QDeadlineTimer receiverDeadline(1000,Qt::PreciseTimer);
        const auto fallback=[handle,top,finish,callback] {
            if(!handle->adapter)return;
            if(!handle->scopedOwner.isEmpty()||!handle->matchesMainFrame(*top)) {
                if(handle->adapter)handle->adapter->deliver(callback,{});return;
            }
            // Seal the ORIGINAL receipt; never replace it or recapture policy.
            // This distinct bounded Top stage does not renew the receiver's
            // expired budget or the later object-proof deadline.
            const QDeadlineTimer topSealDeadline(1000,Qt::PreciseTimer);
            handle->broker->revalidateDocument(*top,[handle,top,finish,callback,topSealDeadline](bool valid) {
                if(!handle->adapter)return;
                if(!valid||topSealDeadline.hasExpired()||!handle->matchesMainFrame(*top)) {
                    if(handle->adapter)handle->adapter->deliver(callback,{});return;
                }
                // Still a draft: original request and live object sealing are
                // required before its facts can enter the production session.
                finish(*top,&topSealDeadline);
            },static_cast<int>(topSealDeadline.remainingTime()),FerricFrameBroker::DocumentWork{
                documentAdmissionFor(handle,QStringLiteral("preflight"),QStringLiteral("0"),QString()),QStringLiteral("fallback-top")});
        };
        handle->broker->captureDocument(selectedOwner,[handle,top,finish,fallback,receiverDeadline](std::optional<FerricFrameBroker::DocumentReceipt> selected) {
            if(!handle->adapter)return;
            if(!selected||selected->documents().size()<2||receiverDeadline.hasExpired()
                ||!handle->matchesMainFrame(*selected)) {fallback();return;}
            const auto &original=top->documents().first(),&root=selected->documents().first();
            if(original.frame!=root.frame||original.owner!=root.owner||original.parent!=root.parent
                ||original.epoch!=root.epoch||original.origin!=root.origin||original.url!=root.url) {fallback();return;}
            handle->broker->captureComposedAncestry(*selected,[handle,top,finish,fallback,receiverDeadline](std::optional<FerricFrameBroker::DocumentReceipt> admitted) {
                if(!handle->adapter)return;
                if(!admitted||receiverDeadline.hasExpired()){fallback();return;}
                if(!handle->matchesMainFrame(*top)){fallback();return;}
                finish(*admitted,&receiverDeadline);
            },static_cast<int>(receiverDeadline.remainingTime()),
                documentAdmissionFor(handle,QStringLiteral("preflight"),QStringLiteral("0"),selected->documents().last().owner));
        },static_cast<int>(receiverDeadline.remainingTime()),FerricFrameBroker::DocumentWork{
            documentAdmissionFor(handle,QStringLiteral("preflight"),QStringLiteral("0"),selectedOwner),QStringLiteral("bind-selected")});
    },static_cast<int>(topDeadline.remainingTime()),FerricFrameBroker::DocumentWork{
        documentAdmissionFor(handle,QStringLiteral("preflight"),QStringLiteral("0"),selectedOwner),QStringLiteral("bind-top")});
}

void FerricHintRequestAdapter::validate(const QVariant &value, const QJSValue &callback) {
    const QPointer<FerricHintRequestAdapter> self(this);
    if (value.metaType() != QMetaType::fromType<SharedHintRequest>()) { deliver(callback, false); return; }
    const auto handle = value.value<SharedHintRequest>();
    if (!handle || handle->adapter != this || !handle->current() || !handle->document
        || handle->documentValidationStarted) {
        if (self) self->deliver(callback, false);
        return;
    }
    // Admit before crossing the async boundary, including timeout/reentry.
    // This original capture cannot manufacture another validation allowance.
    auto *ui=qobject_cast<BrowserUi *>(handle->owner.ui.data());
    if(!ui||!ferric::hints::begin_hint_document_validation(*ui,*handle->captured)) {
        if(self)self->deliver(callback,false);return;
    }
    handle->documentValidationStarted = true;
    const auto owner=handle->hasFocusedReceiver&&!handle->focusedInTop?handle->document->documents().last().owner:QString();
    handle->broker->revalidateDocument(*handle->document, [handle, callback](bool valid) {
        ferric::hints::finish_hint_document_validation(*handle->captured);
        const bool current = valid && handle->document && handle->matchesMainFrame(*handle->document);
        handle->documentValidated=current;
        if (handle->adapter) handle->adapter->deliver(callback, current);
    },1000,FerricFrameBroker::DocumentWork{
        documentAdmissionFor(handle,QStringLiteral("validation"),QStringLiteral("0"),owner),QStringLiteral("validation")});
}

QString FerricHintRequestAdapter::collectorPlan(const QVariant &value) const {
    return collectorPlanForOwner(value, {});
}

QString FerricHintRequestAdapter::collectorPlanForOwner(const QVariant &value, const QString &owner) const {
    if (value.metaType() != QMetaType::fromType<SharedHintRequest>()) return {};
    const auto handle = value.value<SharedHintRequest>();
    if (!handle || handle->adapter != this || !handle->document || !handle->topDocument
        || !handle->matchesMainFrame(*handle->topDocument)) return {};
    if (!owner.isEmpty() && !handle->matchesMainFrame(*handle->document)) return {};
    const bool enteredTop=!handle->scopedOwner.isEmpty()&&handle->focusedInTop;
    if(!handle->scopedOwner.isEmpty()&&owner!=handle->scopedOwner&&!(enteredTop&&owner.isEmpty()))return {};
    auto *ui = qobject_cast<BrowserUi *>(handle->owner.ui.data());
    if (!ui) return {};
    auto plan = ferric::hints::hint_request_plan(*ui, *handle->captured);
    if (plan.isEmpty() || !plan.endsWith('}')) return {};
    QJsonObject context{{"start","top-document"},{"owner",""},{"base_depth",0}};
    const auto &documents = handle->document->documents();
    const auto &leaf = documents.last();
    if (!owner.isEmpty()) {
        if (!handle->hasFocusedReceiver || handle->focusedInTop || owner != leaf.owner) return {};
        const auto composed=handle->document->composedDepth();
        if(!composed)return {};
        context = QJsonObject{{"start", "focused-frame"}, {"owner", leaf.owner}, {"receiver_origin", leaf.origin},
            {"base_depth", static_cast<int>(*composed)}};
    } else if (handle->hasFocusedReceiver && handle->focusedInTop) {
        context.insert("focused_document", QJsonObject{{"epoch",leaf.epoch},{"origin",leaf.origin},
            {"depth",static_cast<int>(documents.size() - 1)}});
        if(enteredTop)context.insert("entered_document",QJsonObject{{"epoch",leaf.epoch},{"origin",leaf.origin},
            {"depth",static_cast<int>(documents.size()-1)}});
    }
    // Keep the original serialized stamp byte order. Re-encoding the entire
    // plan through QJsonObject would reorder it relative to the native seal.
    plan.chop(1);
    return plan + QStringLiteral(",\"context\":")
        + QString::fromUtf8(QJsonDocument(context).toJson(QJsonDocument::Compact)) + '}';
}

QString FerricHintRequestAdapter::collectorSelectors(const QVariant &value) const {
    if (value.metaType() != QMetaType::fromType<SharedHintRequest>()) return {};
    const auto handle = value.value<SharedHintRequest>();
    if (!handle || handle->adapter != this || !handle->current()
        || (handle->document && (!handle->topDocument || !handle->matchesMainFrame(*handle->topDocument)))) return {};
    auto *ui = qobject_cast<BrowserUi *>(handle->owner.ui.data());
    return ui ? ferric::hints::hint_request_selectors(*ui, *handle->captured) : QString();
}

QString FerricHintRequestAdapter::collectionGeometry(const QVariant &value,const QString &owner) const {
    if(value.metaType()!=QMetaType::fromType<SharedHintRequest>())return {};
    const auto handle=value.value<SharedHintRequest>();
    if(!handle||handle->adapter!=this||!handle->collection||!handle->observationStarted
        ||!handle->current()||!collectionWorkPlan(value).isEmpty())return {};
    if(!owner.isEmpty()&&(!handle->document||handle->document->documents().last().owner!=owner))return {};
    const auto &geometry=owner.isEmpty()?handle->topGeometry:handle->selectedGeometry;
    return geometry?QString::fromUtf8(QJsonDocument(*geometry).toJson(QJsonDocument::Compact)):QString();
}

QString FerricHintRequestAdapter::finishCollection(const QVariant &value,bool includeTop) {
    if(value.metaType()!=QMetaType::fromType<SharedHintRequest>())return {};
    const auto handle=value.value<SharedHintRequest>();
    if(!handle||handle->adapter!=this||!handle->collection||!handle->current()
        ||!handle->documentValidated||handle->collectionFinalized||!collectionWorkPlan(value).isEmpty()
        ||(!includeTop&&(!handle->hasFocusedReceiver||(handle->focusedInTop&&handle->scopedOwner.isEmpty()))))return {};
    handle->collectionFinalized=true;
    const bool retainTop=includeTop||(!handle->scopedOwner.isEmpty()&&handle->focusedInTop);
    QJsonArray projections;
    if(handle->topFragments&&handle->topGeometry)
        projections.append(QJsonObject{{"owner",""},{"geometry",*handle->topGeometry},{"excluded",!retainTop}});
    if(handle->selectedFragments&&handle->selectedGeometry&&handle->document)
        projections.append(QJsonObject{{"owner",handle->document->documents().last().owner},{"geometry",*handle->selectedGeometry}});
    auto *ui=qobject_cast<BrowserUi *>(handle->owner.ui.data());
    return ui?ferric::hints::finish_hint_collection(*ui,*handle->captured,
        QString::fromUtf8(QJsonDocument(projections).toJson(QJsonDocument::Compact))):QString();
}

QString FerricHintRequestAdapter::publishCollection(const QVariant &value) {
    const auto rejected=QStringLiteral("{\"error\":\"stale-policy-collection\"}");
    if(value.metaType()!=QMetaType::fromType<SharedHintRequest>())return rejected;
    const auto handle=value.value<SharedHintRequest>();
    if(!handle||handle->adapter!=this||!handle->collectionFinalized||!handle->current())return rejected;
    auto *ui=qobject_cast<BrowserUi *>(handle->owner.ui.data());
    return ui?ferric::hints::publish_hint_collection(*ui,*handle->captured):rejected;
}

QVariantMap FerricHintRequestAdapter::beginSelection(QQuickItem *view,FerricHintFrameBroker *broker,
    const QString &label,const QString &scopedOwner) {
    const QVariantMap closed{{"state","closed"}};
    if(!view||!broker||!lifetime_||QThread::currentThread()!=thread())return closed;
    const QPointer<FerricHintRequestAdapter> self(this);
    const auto owner=lifetime_?lifetime_->currentView(view):std::nullopt;
    const QPointer<BrowserUi> ui(owner?qobject_cast<BrowserUi *>(owner->ui.data()):nullptr);
    if(!ui)return closed;
    const auto state=ferric::hints::hint_anchor_state(*ui,label);
    if(state==QStringLiteral("ordinary"))return QVariantMap{{"state","ordinary"}};
    if(state!=QStringLiteral("live"))return closed;
    auto result=beginCollection(view,broker,scopedOwner);
    const auto value=result.value("request");
    if(!self||!ui||value.metaType()!=QMetaType::fromType<SharedHintRequest>())return closed;
    const auto handle=value.value<SharedHintRequest>();
    if(!handle||!handle->current()||!ferric::hints::freeze_hint_anchor(*ui,*handle->captured,label))return closed;
    return result;
}

QString FerricHintRequestAdapter::cancelSelection(const QVariant &value) {
    if(value.metaType()!=QMetaType::fromType<SharedHintRequest>())return QStringLiteral("{\"error\":\"stale-policy-selection\"}");
    const auto handle=value.value<SharedHintRequest>();
    auto *ui=handle?qobject_cast<BrowserUi *>(handle->owner.ui.data()):nullptr;
    if(!ui||handle->adapter!=this)return QStringLiteral("{\"error\":\"stale-policy-selection\"}");
    return ferric::hints::finish_hint_anchor_check(*ui,*handle->captured,false);
}

void FerricHintRequestAdapter::confirmSelection(const QVariant &value,const QJSValue &callback) {
    if(value.metaType()!=QMetaType::fromType<SharedHintRequest>()) {deliver(callback,QStringLiteral("{\"error\":\"stale-policy-selection\"}"));return;}
    const auto handle=value.value<SharedHintRequest>();
    if(!handle||handle->adapter!=this||!handle->current()||!handle->collectionFinalized||handle->selectionCheckStarted) {
        deliver(callback,QStringLiteral("{\"error\":\"stale-policy-selection\"}"));return;
    }
    handle->selectionCheckStarted=true;
    auto *ui=qobject_cast<BrowserUi *>(handle->owner.ui.data());
    const auto manifest=ui?QJsonDocument::fromJson(ferric::hints::hint_anchor_manifest(*ui,*handle->captured).toUtf8()).object():QJsonObject();
    const auto owner=manifest.value("owner");
    const auto focused=manifest.value("focused");
    const auto finish=[handle,callback](bool valid) {
        if(!handle->adapter)return;
        auto *ui=qobject_cast<BrowserUi *>(handle->owner.ui.data());
        const auto result=ui?ferric::hints::finish_hint_anchor_check(*ui,*handle->captured,valid&&handle->current())
            :QStringLiteral("{\"error\":\"stale-policy-selection\"}");
        if(handle->adapter)handle->adapter->deliver(callback,result);
    };
    if(!owner.isString()||!focused.isBool()){finish(false);return;}
    const auto catalogue=owner.toString().isEmpty()?handle->topFragments
        :(handle->document&&handle->document->documents().last().owner==owner.toString()?handle->selectedFragments:std::nullopt);
    if(!catalogue){finish(false);return;}
    handle->broker->validateSelectionCatalogue(*catalogue,focused.toBool(),finish);
}

bool FerricHintRequestAdapter::beginCollectionWork(const QVariant &value) {
    if(value.metaType()!=QMetaType::fromType<SharedHintRequest>())return false;
    const auto handle=value.value<SharedHintRequest>();
    if(!handle||!handle->collection||handle->adapter!=this||handle->observationStarted||!handle->document||!handle->topDocument
        ||!handle->matchesMainFrame(*handle->topDocument))return false;
    auto *ui=qobject_cast<BrowserUi *>(handle->owner.ui.data());
    if(!ui||!ferric::hints::begin_hint_collection_work(*ui,*handle->captured,
        handle->focusedInTop?QString():handle->scopedOwner))return false;
    handle->observationStarted=true;
    return true;
}

QString FerricHintRequestAdapter::collectionWorkPlan(const QVariant &value) const {
    if(value.metaType()!=QMetaType::fromType<SharedHintRequest>())return {};
    const auto handle=value.value<SharedHintRequest>();
    if(!handle||!handle->collection||handle->adapter!=this||!handle->document||!handle->topDocument
        ||!handle->matchesMainFrame(*handle->topDocument))return {};
    auto *ui=qobject_cast<BrowserUi *>(handle->owner.ui.data());
    return ui?ferric::hints::hint_collection_work_plan(*ui,*handle->captured):QString();
}

bool FerricHintRequestAdapter::acceptCollectionOrdinary(const QVariant &value,const QString &stage,
    const QString &owner,const QString &candidates,const QString &metadata) {
    if(value.metaType()!=QMetaType::fromType<SharedHintRequest>())return false;
    const auto handle=value.value<SharedHintRequest>();
    if(!handle||!handle->collection||handle->adapter!=this||!handle->document||!handle->topDocument
        ||!handle->matchesMainFrame(*handle->topDocument))return false;
    auto *ui=qobject_cast<BrowserUi *>(handle->owner.ui.data());
    if(!ui)return false;
    const auto parsed=QJsonDocument::fromJson(ferric::hints::hint_collection_work_plan(*ui,*handle->captured).toUtf8());
    if(!parsed.isObject())return false;
    const auto plan=parsed.object();
    if(plan.value("phase").toString()!=QStringLiteral("ordinary")||plan.value("stage").toString()!=stage
        ||plan.value("owner").toString()!=owner)return false;
    // The Rust step is the owner catalogue; caller strings alone never choose
    // a receipt. Optional selected-world expiry must not retire independent Top.
    const bool admitted=owner.isEmpty()||(handle->hasFocusedReceiver&&!handle->focusedInTop
        &&handle->document->documents().last().owner==owner&&handle->matchesMainFrame(*handle->document));
    ui=qobject_cast<BrowserUi *>(handle->owner.ui.data());
    if(!handle->adapter||!ui)return false;
    if(!admitted) {
        ferric::hints::reject_hint_collection_work(*ui,*handle->captured,stage,owner);
        return false;
    }
    return ui&&ferric::hints::accept_hint_collection_ordinary(*ui,*handle->captured,stage,owner,candidates,metadata);
}

void FerricHintRequestAdapter::verifyCollectionPolicy(const QVariant &value,const QString &stage,
    const QString &owner,const QString &annotation,const QString &metadata,const QJSValue &callback) {
    const QDeadlineTimer deadline(1000,Qt::PreciseTimer);
    const QPointer<FerricHintRequestAdapter> self(this);
    if(value.metaType()!=QMetaType::fromType<SharedHintRequest>()){if(self)self->deliver(callback,false);return;}
    const auto handle=value.value<SharedHintRequest>();
    if(!handle||!handle->collection||handle->adapter!=this||!handle->document||!handle->topDocument
        ||!handle->current()){if(self)self->deliver(callback,false);return;}
    auto *ui=qobject_cast<BrowserUi *>(handle->owner.ui.data());
    if(!ui||!ferric::hints::prepare_hint_collection_policy(*ui,*handle->captured,stage,owner,annotation,metadata)) {
        if(handle->adapter)handle->adapter->deliver(callback,false);
        return;
    }
    const auto finish=[handle,stage,owner,callback,deadline](std::optional<FerricFrameBroker::FragmentCatalogue> catalogue) {
        if(!handle->adapter)return;
        const bool current=catalogue&&!deadline.hasExpired()&&handle->current();
        auto *ui=qobject_cast<BrowserUi *>(handle->owner.ui.data());
        const bool accepted=ui&&ferric::hints::complete_hint_collection_policy(*ui,*handle->captured,stage,owner,current);
        if(accepted){
            if(owner.isEmpty())handle->topFragments=std::move(catalogue);
            else handle->selectedFragments=std::move(catalogue);
        }
        if(handle->adapter)handle->adapter->deliver(callback,accepted);
    };
    const auto manifest=ferric::hints::hint_collection_policy_manifest(*ui,*handle->captured,stage,owner);
    const auto raw=ferric::hints::hint_collection_policy_annotation(*ui,*handle->captured,stage,owner);
    const auto ordinaryGeometry=ferric::hints::hint_collection_policy_geometry(*ui,*handle->captured,stage,owner);
    const auto ordinaryInput=ferric::hints::hint_collection_policy_input(*ui,*handle->captured,stage,owner);
    if(manifest.isEmpty()||manifest.size()>8192||raw.isEmpty()||ordinaryInput.isEmpty()||deadline.hasExpired()) {finish(std::nullopt);return;}
    const auto parsed=QJsonDocument::fromJson(manifest.toUtf8());
    if(!parsed.isObject()){finish(std::nullopt);return;}
    const auto plan=parsed.object();
    const auto slotOwner=plan.value("owner").toString();
    if(slotOwner!=owner||!handle->document||!handle->topDocument||!handle->matchesMainFrame(*handle->topDocument)) {
        finish(std::nullopt);return;
    }
    // Only the native/Rust slot catalogue chooses this receipt. Neither the
    // caller nor the raw renderer graph can supply or replace a native world.
    auto world=*handle->topDocument;
    if(!slotOwner.isEmpty()) {
        if(!handle->hasFocusedReceiver||handle->focusedInTop||handle->document->documents().last().owner!=slotOwner
            ||!handle->matchesMainFrame(*handle->document)) {
            finish(std::nullopt);return;
        }
        world=*handle->document;
    }
    const auto batch=plan.value("nonce").toString(),revision=plan.value("revision").toString();
    const auto stamp=plan.value("stamp").toString(),scope=plan.value("scope").toString();
    const auto focusedJson=plan.value("focused_documents").toString();
    bool validRevision=false;
    const auto number=revision.toULongLong(&validRevision);
    if(!validRevision||QString::number(number)!=revision||number>9007199254740991ULL
        ||batch.size()!=36||stamp.isEmpty()||scope.isEmpty()||focusedJson.isEmpty()||focusedJson.size()>1024) {
        finish(std::nullopt);return;
    }
    const auto required=QJsonDocument::fromJson(focusedJson.toUtf8());
    if(!required.isArray()||required.array().size()>80){finish(std::nullopt);return;}
    QList<quint32> focusedDocuments;
    for(const auto &entry:required.array()) {
        const auto id=entry.toDouble();
        if(!entry.isDouble()||!std::isfinite(id)||id<1||id>std::numeric_limits<quint32>::max()||std::floor(id)!=id) {
            finish(std::nullopt);return;
        }
        focusedDocuments.append(static_cast<quint32>(id));
    }
    const auto dependency=(!slotOwner.isEmpty()||!focusedDocuments.isEmpty()||!handle->scopedOwner.isEmpty())?*handle->document:world;
    const auto validDebit=[](const QJsonValue &value) {
        const auto number=value.toDouble(-1);
        return value.isDouble()&&std::isfinite(number)&&number>=0&&number<=2*1024*1024&&std::floor(number)==number;
    };
    const auto reported=plan.value("annotation_bytes"),granted=plan.value("annotation_limit");
    if(!validDebit(reported)||!validDebit(granted)||reported.toDouble()>granted.toDouble()){finish(std::nullopt);return;}
    const FerricFrameBroker::AnnotationDebit debit{static_cast<quint32>(reported.toDouble()),static_cast<quint32>(granted.toDouble())};
    const auto originalPlan=self?self->collectorPlanForOwner(value,slotOwner):QString();
    if(originalPlan.isEmpty()){finish(std::nullopt);return;}
    const auto viewport=plan.value("viewport").toObject();
    const auto width=viewport.value("width").toDouble(-1),height=viewport.value("height").toDouble(-1);
    if(!std::isfinite(width)||width<=0||!std::isfinite(height)||height<=0){finish(std::nullopt);return;}
    const auto current=[handle,world,dependency,deadline] {
        return handle->adapter&&!deadline.hasExpired()&&handle->matchesMainFrame(world)
            &&handle->matchesMainFrame(dependency);
    };
    std::optional<FerricFrameBroker::FocusedDocuments> focused;
    if(!focusedDocuments.isEmpty())focused.emplace(FerricFrameBroker::FocusedDocuments{*handle->document,focusedDocuments});
    FerricFrameBroker::FragmentRequest fragmentRequest{world,std::move(focused),batch,stamp,number,
        raw,scope,originalPlan,ordinaryGeometry,debit,QSizeF(width,height),ordinaryInput};
    fragmentRequest.graphAdmission=graphAdmissionFor(handle,stage,slotOwner);
    fragmentRequest.documentAdmission=documentAdmissionFor(handle,QStringLiteral("policy"),stage,slotOwner);
    if(!handle->scopedOwner.isEmpty()&&handle->focusedInTop)
        fragmentRequest.entered=FerricFrameBroker::EnteredDocument{*handle->document};
    const auto documentAdmission=fragmentRequest.documentAdmission;
    handle->broker->captureFragmentCatalogue(std::move(fragmentRequest),
        [handle,dependency,deadline,finish,current,documentAdmission,slotOwner,stage](std::optional<FerricFrameBroker::FragmentCatalogue> catalogue) {
        if(!catalogue||!current()){finish(std::nullopt);return;}
        // Top depends on the selected chain only for an actual focused-root
        // obligation. Otherwise optional foreign failure leaves Top independent.
        handle->broker->revalidateDocument(dependency,[handle,deadline,catalogue,finish,current,slotOwner,stage](bool valid) {
            if(!valid||!current()){finish(std::nullopt);return;}
            handle->broker->revalidateFragmentCatalogue(*catalogue,[handle,deadline,catalogue,finish,current,slotOwner,stage](bool sealed) {
                if(!sealed||!current()){finish(std::nullopt);return;}
                auto ticket=nativeAdmissionFor(handle,QStringLiteral("policy"),stage,slotOwner,
                    FerricHintWorkTicket::Counter::Graph)(QJsonObject{{"kind","geometry"}});
                if(!ticket){finish(std::nullopt);return;}
                handle->broker->projectFragmentCatalogue(*catalogue,[handle,catalogue,finish,current,slotOwner](std::optional<QJsonObject> geometry) {
                    if(!geometry||!current()){finish(std::nullopt);return;}
                    if(slotOwner.isEmpty())handle->topGeometry=std::move(geometry);
                    else handle->selectedGeometry=std::move(geometry);
                    finish(catalogue);
                },static_cast<int>(deadline.remainingTime()),std::move(ticket));
            },static_cast<int>(deadline.remainingTime()));
        },static_cast<int>(deadline.remainingTime()),FerricFrameBroker::DocumentWork{documentAdmission,
            slotOwner.isEmpty()?QStringLiteral("top-dependency"):QStringLiteral("selected-dependency")});
    },static_cast<int>(deadline.remainingTime()));
}

bool FerricHintRequestAdapter::rejectCollectionWork(const QVariant &value,const QString &stage,const QString &owner) {
    if(value.metaType()!=QMetaType::fromType<SharedHintRequest>())return false;
    const auto handle=value.value<SharedHintRequest>();
    if(!handle||!handle->collection||handle->adapter!=this||!handle->document||!handle->topDocument
        ||!handle->matchesMainFrame(*handle->topDocument))return false;
    auto *ui=qobject_cast<BrowserUi *>(handle->owner.ui.data());
    return ui&&ferric::hints::reject_hint_collection_work(*ui,*handle->captured,stage,owner);
}

void FerricHintRequestAdapter::verifyObservationDocuments(const QVariant &value, const QString &candidates,
                                                         const QString &annotation, const QString &batch,
                                                         const QString &revision, const QJSValue &callback) {
    verifyObservation(value, candidates, annotation, batch, revision, callback, false);
}

void FerricHintRequestAdapter::verifyObservationObjects(const QVariant &value, const QString &candidates,
                                                         const QString &annotation, const QString &batch,
                                                         const QString &revision, const QJSValue &callback) {
    verifyObservation(value, candidates, annotation, batch, revision, callback, true);
}

void FerricHintRequestAdapter::verifyObservation(const QVariant &value, const QString &candidates,
                                                const QString &annotation, const QString &batch,
                                                const QString &revision, const QJSValue &callback, bool objects) {
    const QDeadlineTimer deadline(1000,Qt::PreciseTimer);
    const QPointer<FerricHintRequestAdapter> self(this);
    if (value.metaType() != QMetaType::fromType<SharedHintRequest>()) { deliver(callback, false); return; }
    const auto handle = value.value<SharedHintRequest>();
    if (!handle || handle->adapter != this || handle->observationStarted || !handle->document || !handle->topDocument
        || !handle->matchesMainFrame(*handle->document)) {
        if (self) self->deliver(callback, false);
        return;
    }
    handle->observationStarted = true;
    bool validRevision = false;
    const auto number = revision.toULongLong(&validRevision);
    if (!validRevision || QString::number(number) != revision || number > 9007199254740991ULL
        || batch.size() != 36) { if (self) self->deliver(callback, false); return; }
    auto *ui = qobject_cast<BrowserUi *>(handle->owner.ui.data());
    if (!ui) { if (self) self->deliver(callback, false); return; }
    const auto documents = ferric::hints::prepare_hint_observation_documents(*ui, *handle->captured, candidates, annotation);
    const auto stamp = ferric::hints::hint_observation_stamp(*ui, *handle->captured);
    const auto required = ferric::hints::hint_observation_focused_documents(*ui, *handle->captured);
    const auto original = objects ? ferric::hints::hint_observation_annotation(*ui, *handle->captured) : QString();
    const auto scope = objects ? ferric::hints::hint_observation_scope(*ui, *handle->captured) : QString();
    const auto originalPlan=objects&&self?self->collectorPlan(value):QString();
    if (objects && (original.isEmpty() || scope.isEmpty() || originalPlan.isEmpty())) { if (self) self->deliver(callback, false); return; }
    if(required.isEmpty()||required.size()>1024){if(self)self->deliver(callback,false);return;}
    const auto parsedRequired = QJsonDocument::fromJson(required.toUtf8());
    if (!parsedRequired.isArray() || parsedRequired.array().size()>80) {
        if (self) self->deliver(callback,false);
        return;
    }
    QList<quint32> focusedDocuments;
    for(const auto &value:parsedRequired.array()) {
        const auto id=value.toDouble();
        if(!value.isDouble()||!std::isfinite(id)||id<1||id>std::numeric_limits<quint32>::max()||std::floor(id)!=id) {
            if (self) self->deliver(callback,false);
            return;
        }
        focusedDocuments.append(static_cast<quint32>(id));
    }
    if (documents.isEmpty() || stamp.isEmpty() || !handle->matchesMainFrame(*handle->document)) {
        if (self) self->deliver(callback, false);
        return;
    }
    const auto finish = [handle, callback, deadline, objects](bool associated) {
        if (!handle->adapter) return;
        const bool current = associated && handle->document
            && handle->matchesMainFrame(*handle->document) && !deadline.hasExpired();
        auto *ui = qobject_cast<BrowserUi *>(handle->owner.ui.data());
        const bool checked = ui && (objects
            ? ferric::hints::complete_hint_observation_objects(*ui, *handle->captured, current)
            : ferric::hints::complete_hint_observation_documents(*ui, *handle->captured, current));
        if (handle->adapter) handle->adapter->deliver(callback, checked);
    };
    const auto checkTop = [handle,batch,stamp,number,documents,focusedDocuments,deadline,objects,original,scope,originalPlan](std::function<void(bool)> completion) {
        if (objects) {
            std::optional<FerricFrameBroker::FocusedDocuments> focused;
            if (!focusedDocuments.isEmpty()) focused.emplace(FerricFrameBroker::FocusedDocuments{*handle->document,focusedDocuments});
            handle->broker->validateWorldObjects(*handle->topDocument, std::move(focused), batch, stamp, number,
                original, scope, std::move(completion), static_cast<int>(deadline.remainingTime()),std::nullopt,originalPlan);
        } else if(focusedDocuments.isEmpty()) {
            handle->broker->validateWorldDocuments(*handle->topDocument,batch,stamp,number,documents,
                std::move(completion),static_cast<int>(deadline.remainingTime()));
        } else {
            handle->broker->validateFocusedWorldDocuments(*handle->topDocument,
                FerricFrameBroker::FocusedDocuments{*handle->document,focusedDocuments},batch,stamp,number,documents,
                std::move(completion),static_cast<int>(deadline.remainingTime()));
        }
    };
    checkTop([handle, deadline, finish, checkTop](bool associated) {
            if (!associated || deadline.hasExpired() || !handle->document
                || !handle->matchesMainFrame(*handle->document)) { finish(false); return; }
            // Seal the complete original raw graph before canonical matching
            // can filter it. Classification does not publish checked status;
            // its CPU work and the final original-graph seal share this deadline.
            auto *ui=qobject_cast<BrowserUi *>(handle->owner.ui.data());
            if(!ui||!ferric::hints::classify_hint_observation(*ui,*handle->captured)
                ||deadline.hasExpired()||!handle->document
                ||!handle->matchesMainFrame(*handle->document)){finish(false);return;}
            // The selected native chain is distinct from Top's local table.
            // Native map/URL equality does not prove a same-URL leaf's epoch.
            handle->broker->revalidateDocument(*handle->document,
                [handle, deadline, finish, checkTop](bool current) {
                    if (!current || deadline.hasExpired() || !handle->document || !handle->topDocument
                        || !handle->matchesMainFrame(*handle->document)) { finish(false); return; }
                    // The selected-chain probe is asynchronous too. Seal Top's
                    // exact batch again after it, under the same deadline.
                    checkTop(finish);
                }, static_cast<int>(deadline.remainingTime()));
        });
}

bool FerricHintRequestAdapter::observationDocumentsChecked(const QVariant &value) const {
    if (value.metaType() != QMetaType::fromType<SharedHintRequest>()) return false;
    const auto handle = value.value<SharedHintRequest>();
    if (!handle || handle->adapter != this || !handle->document
        || !handle->matchesMainFrame(*handle->document)) return false;
    auto *ui = qobject_cast<BrowserUi *>(handle->owner.ui.data());
    return ui && ferric::hints::hint_observation_documents_checked(*ui, *handle->captured);
}

bool FerricHintRequestAdapter::observationObjectsChecked(const QVariant &value) const {
    if (value.metaType() != QMetaType::fromType<SharedHintRequest>()) return false;
    const auto handle = value.value<SharedHintRequest>();
    if (!handle || handle->adapter != this || !handle->document
        || !handle->matchesMainFrame(*handle->document)) return false;
    auto *ui = qobject_cast<BrowserUi *>(handle->owner.ui.data());
    return ui && ferric::hints::hint_observation_objects_checked(*ui, *handle->captured);
}
