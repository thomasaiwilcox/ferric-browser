#include "hint_frame_broker.h"
#include "hint_proof_object_probe.h"
#include "hint_frame_ancestry_probe.h"

#include <QtCore/QDeadlineTimer>
#include <QtCore/QCoreApplication>
#include <QtCore/QFile>
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QLoggingCategory>
#include <QtCore/QPointer>
#include <QtCore/QRectF>
#include <QtCore/QTimer>
#include <QtCore/QSet>
#include <QtCore/QThread>
#include <QtCore/QUuid>
#include <QtQml/QJSEngine>
#include <QtWebEngineCore/QWebEngineScript>
#include <algorithm>
#include <cmath>
#include <memory>
#include <map>
#include <utility>

Q_LOGGING_CATEGORY(pageFocusLog, "ferric.pagekeys.focus", QtWarningMsg)

namespace {
QString quoted(const QString &value) {
    const auto json = QJsonDocument(QJsonArray{value}).toJson(QJsonDocument::Compact);
    return QString::fromUtf8(json.mid(1, json.size() - 2));
}
QString hintGeometryRuntime() {
    // Reuse the shipping mapping implementation. QML directives are the only
    // non-JavaScript lines; the imported policy builders are not invoked here.
    static const QString runtime=[] {
        QFile file(QStringLiteral(":/qt/qml/io/github/ferricbrowser/qml/scripts/PageInteractionScripts.js"));
        if(!file.open(QIODevice::ReadOnly))return QString();
        QString source;
        for(const auto &line:QString::fromUtf8(file.readAll()).split('\n'))
            if(!line.startsWith(QStringLiteral(".pragma "))&&!line.startsWith(QStringLiteral(".import ")))source+=line+'\n';
        QJSEngine engine;
        if(engine.evaluate(source).isError())return QString();
        const auto value=engine.globalObject().property(QStringLiteral("scrollTargetRuntime")).call();
        const auto witnesses=engine.globalObject().property(QStringLiteral("hintNameRuntime")).call();
        return value.isString()&&witnesses.isString()?value.toString()+witnesses.toString():QString();
    }();
    return runtime;
}
QVariant failure(const char *reason) {
    return QVariantMap{{QStringLiteral("error"), QString::fromLatin1(reason)}};
}
std::optional<FerricFrameBroker::DocumentWork> documentWork(const FerricHintDocumentAdmission &admission,
                                                           const QString &checkpoint) {
    if(!admission)return std::nullopt;
    return FerricFrameBroker::DocumentWork{admission,checkpoint};
}
QString focusedLeafPredicate() {
    return QStringLiteral(R"JS(function(doc,hasChildren){
        if(doc.hasFocus()!==true)return false;
        let active=doc.activeElement;
        for(let depth=0;active&&active.shadowRoot&&depth<8;depth++){
            const inner=active.shadowRoot.activeElement;
            if(!inner||inner===active)break;
            active=inner;
        }
        if(!active||(active.shadowRoot&&active.shadowRoot.activeElement))return false;
        if(['iframe','frame','object','embed'].includes(active.localName))return false;
        // https://dom.spec.whatwg.org/#valid-shadow-host-name
        const hiddenShadow=active.namespaceURI==='http://www.w3.org/1999/xhtml'
            &&!active.shadowRoot&&(active.localName.includes('-')
            ||['article','aside','blockquote','body','div','footer','h1','h2','h3','h4','h5','h6',
                'header','main','nav','p','section','span'].includes(active.localName));
        if(hasChildren&&hiddenShadow)return null;
        return true;
    })JS");
}

}

struct FerricFrameBroker::FragmentCatalogueData {
    FragmentRequest request;
    QString json;
    QJsonArray targets;
};

const QString &FerricFrameBroker::FragmentCatalogue::json() const { return data_->json; }
const QJsonArray &FerricFrameBroker::FragmentCatalogue::targets() const { return data_->targets; }

void FerricFrameBroker::captureFragmentCatalogue(FragmentRequest request, FragmentCompletion completion, int timeoutMs) {
    captureFragmentCatalogueImpl(std::move(request),std::move(completion),timeoutMs,0);
}

void FerricFrameBroker::captureFragmentCatalogueImpl(FragmentRequest request, FragmentCompletion completion,
                                                     int timeoutMs, quint8 graphBase) {
    if (!completion) return;
    const QDeadlineTimer deadline(timeoutMs, Qt::PreciseTimer);
    if (timeoutMs <= 0 || timeoutMs > 1000 || request.plan.isEmpty() || !std::isfinite(request.viewport.width()) || request.viewport.width() <= 0
        || !std::isfinite(request.viewport.height()) || request.viewport.height() <= 0) {
        completion(std::nullopt); return;
    }
    const auto snapshot = std::make_shared<QString>();
    // The string is returned by the graph probe itself, then compared exactly
    // by the final probe after native document checks. There is no separate
    // unsealed fetch between a bool validation and receipt construction.
    validateWorldObjectsImpl(request.world, request.focused, request.batch, request.stamp, request.revision,
        request.annotation, request.scope,
        [request, snapshot, completion, deadline](bool valid) {
            if (!valid || snapshot->isEmpty() || deadline.hasExpired()) { completion(std::nullopt); return; }
            const auto parsed = QJsonDocument::fromJson(snapshot->toUtf8());
            if (!parsed.isObject() || !parsed.object().value("targets").isArray()) { completion(std::nullopt); return; }
            auto data=std::make_shared<const FragmentCatalogueData>(FragmentCatalogueData{
                request, *snapshot, parsed.object().value("targets").toArray()});
            if(deadline.hasExpired()){completion(std::nullopt);return;}
            completion(FragmentCatalogue(std::move(data)));
        }, static_cast<int>(deadline.remainingTime()), request.entered, request.plan, request.debit, request.viewport,
        [snapshot](const QString &json) { *snapshot = json; }, request.ordinaryGeometry, request.ordinaryInput,
        request.graphAdmission,graphBase,request.documentAdmission);
}

void FerricFrameBroker::validateSelectionCatalogue(const FragmentCatalogue &catalogue,
    bool requireFocus,std::function<void(bool)> completion,int timeoutMs) {
    if(!completion)return;
    const auto original=catalogue.data_;
    if(!original||timeoutMs<=0||timeoutMs>1000||!matchesReceipt(original->request.world)){completion(false);return;}
    const auto &request=original->request;
    const auto leaf=request.world.documents_.last();
    QString focusCheck=QStringLiteral("true");
    QList<QWebEngineFrame> children;
    if(requireFocus) {
        if(!request.focused||request.focused->requiredDocuments.size()>80
            ||!matchesReceipt(request.focused->selected)){completion(false);return;}
        const auto focused=request.focused->selected.documents_.last();
        children=focused.frame.children();
        QJsonArray ids;for(const auto id:request.focused->requiredDocuments)ids.append(static_cast<double>(id));
        focusCheck=QStringLiteral("(()=>{const check=")+focusedLeafPredicate()+QStringLiteral(";let selected=null,count=0;")
            +QStringLiteral("for(const id of ")+QString::fromUtf8(QJsonDocument(ids).toJson(QJsonDocument::Compact))
            +QStringLiteral("){const entry=p.documents.get(id),doc=entry&&entry.document&&WeakRef.prototype.deref.call(entry.document);")
            +QStringLiteral("if(doc&&doc.defaultView&&doc.defaultView[")+quoted(documentProperty_)+QStringLiteral("]===")+quoted(focused.epoch)
            +QStringLiteral("&&String(doc.defaultView.origin)===")+quoted(focused.origin)
            +QStringLiteral("){selected=doc;++count;}}return count===1&&check(selected,")
            +(children.isEmpty()?QStringLiteral("false"):QStringLiteral("true"))+QStringLiteral(")===true;})()");
    }
    const auto script=QStringLiteral("(()=>{const s=window.__ferric_browserHintState,p=s&&s.interactionProof;return window.")
        +documentProperty_+QStringLiteral("===")+quoted(leaf.epoch)
        +QStringLiteral("&&String(window.origin)===")+quoted(leaf.origin)
        // The sealed fresh collection is the uniqueness proof point. Global
        // tracking dirtiness after it may be an unrelated clock/counter;
        // existing action freshness separately checks the selected target.
        +QStringLiteral("&&!!s&&!!p&&p.batch===")+quoted(request.batch)
        +QStringLiteral("&&p.stamp===")+quoted(request.stamp)
        +QStringLiteral("&&p.revision===")+QString::number(request.revision)
        +QStringLiteral("&&innerWidth===")+QString::number(request.viewport.width(),'g',17)
        +QStringLiteral("&&innerHeight===")+QString::number(request.viewport.height(),'g',17)
        +QStringLiteral("&&(")+focusCheck+QStringLiteral(");})()");
    evaluate(leaf.frame,script,[this,original,children,requireFocus,completion](const QVariant &result){
        completion(result.metaType().id()==QMetaType::Bool&&result.toBool()&&matchesReceipt(original->request.world)
            &&(!requireFocus||(original->request.focused&&matchesReceipt(original->request.focused->selected)
                &&original->request.focused->selected.documents_.last().frame.children()==children)));
    },timeoutMs);
}

void FerricFrameBroker::revalidateFragmentCatalogue(const FragmentCatalogue &catalogue,
    std::function<void(bool)> completion, int timeoutMs) {
    if (!completion) return;
    const QDeadlineTimer deadline(timeoutMs, Qt::PreciseTimer);
    if (timeoutMs <= 0 || timeoutMs > 1000 || !catalogue.data_) { completion(false); return; }
    const auto original = catalogue.data_;
    captureFragmentCatalogueImpl(original->request, [original, completion,deadline](std::optional<FragmentCatalogue> current) {
        const bool same=current&&current->json()==original->json;
        completion(same&&!deadline.hasExpired());
    }, static_cast<int>(deadline.remainingTime()),2);
}

// A cleanup reservation occupies one of the same 128 dispatch slots before an
// observer can be installed. Native callbacks and retained receipts own this
// state independently of broker/user-callback cancellation and destruction.
class FerricFrameDispatcher : public std::enable_shared_from_this<FerricFrameDispatcher> {
public:
    struct Cleanup {
        QWebEngineFrame frame;
        QString epoch, issuer;
        QSet<QString> live, pending;
        bool running = false, closed = false, failed = false;
    };

    int occupied() const { return ordinary_ + static_cast<int>(cleanup_.size()); }

    bool reserveOrdinary() {
        retireInvalid();
        if (occupied() >= 128) return false;
        ++ordinary_;
        return true;
    }

    void finishOrdinary() { --ordinary_; }

    std::shared_ptr<Cleanup> reserveCleanup(const QWebEngineFrame &frame,
        const QString &epoch, const QString &issuer, const QString &token) {
        retireInvalid();
        if (!frame.isValid() || occupied() >= 128) return {};
        for (const auto &entry : std::as_const(cleanup_)) {
            if (entry->frame == frame && entry->epoch == epoch && entry->issuer == issuer) {
                if (entry->closed || entry->live.size() >= 64 || entry->live.contains(token)) return {};
                entry->live.insert(token);
                return entry;
            }
        }
        // Leave a second slot for the capture script itself. Installation must
        // never start if its eventual release cannot be dispatched at capacity.
        if (occupied() >= 127) return {};
        auto entry = std::make_shared<Cleanup>(Cleanup{frame, epoch, issuer, {token}, {}, false, false, false});
        cleanup_.append(entry);
        return entry;
    }

    void release(const std::shared_ptr<Cleanup> &entry, const QString &token) {
        if (!entry->live.contains(token)) return;
        entry->pending.insert(token);
        drain(entry);
    }

    void releaseAll() {
        const auto entries = cleanup_;
        for (const auto &entry : entries) {
            entry->closed = true;
            entry->pending.unite(entry->live);
            drain(entry);
        }
    }

private:
    void retireInvalid() {
        cleanup_.erase(std::remove_if(cleanup_.begin(), cleanup_.end(), [](const auto &entry) {
            if (entry->frame.isValid()) return false;
            entry->closed = true;
            entry->live.clear();
            entry->pending.clear();
            return true;
        }), cleanup_.end());
    }

    void drain(const std::shared_ptr<Cleanup> &entry) {
        retireInvalid();
        if (entry->running || entry->failed || entry->pending.isEmpty() || !entry->frame.isValid()) return;
        const auto released = std::exchange(entry->pending, {});
        QJsonArray tokens;
        for (const auto &token : released) tokens.append(token);
        const QJsonObject request{{"mode", "release"}, {"issuer", entry->issuer}, {"tokens", tokens}};
        const auto script = QStringLiteral("(") + ferricHintFrameAncestryProbe() + QStringLiteral(")(JSON.parse(")
            + quoted(QString::fromUtf8(QJsonDocument(request).toJson(QJsonDocument::Compact))) + QStringLiteral("))");
        entry->running = true;
        auto frame = entry->frame;
        frame.runJavaScript(script, QWebEngineScript::ApplicationWorld,
            [self = shared_from_this(), entry, released](const QVariant &result) {
                // A native reply proves completion, not observer release. An
                // unknown/failed acknowledgement burns this reservation until
                // definitive native retirement; do not start a retry loop.
                if (result.metaType().id() != QMetaType::Bool || !result.toBool()) {
                    entry->running = false;
                    entry->closed = entry->failed = true;
                    self->retireInvalid();
                    return;
                }
                for (const auto &token : released) entry->live.remove(token);
                entry->running = false;
                if (entry->live.isEmpty()) self->cleanup_.removeOne(entry);
                else self->drain(entry);
            });
    }

    int ordinary_ = 0;
    QList<std::shared_ptr<Cleanup>> cleanup_;
};

FerricFrameBroker::FerricFrameBroker(QObject *parent, const QString &documentProperty)
    : QObject(parent), documentProperty_(documentProperty), ancestryIssuer_(QUuid::createUuid().toString(QUuid::WithoutBraces)),
      dispatcher_(std::make_shared<FerricFrameDispatcher>()) {}

int FerricFrameBroker::pendingScripts() const { return dispatcher_->occupied(); }

struct FerricFrameBroker::AncestryLease {
    struct Entry {
        QWebEngineFrame parent;
        QString token;
        quint8 shadows = 0;
        std::shared_ptr<FerricFrameDispatcher::Cleanup> cleanup;
    };
    QString issuer;
    std::shared_ptr<FerricFrameDispatcher> dispatcher;
    QList<Entry> entries;
    quint8 depth = 0;
    ~AncestryLease() {
        auto *application=QCoreApplication::instance();
        if(!application)return;
        // The last receipt copy can be retired independently of broker stop.
        // Release on the GUI thread; never retain observed roots indefinitely.
        QMetaObject::invokeMethod(application,[entries=entries,dispatcher=dispatcher] {
            for(const auto &entry:entries) dispatcher->release(entry.cleanup,entry.token);
        },Qt::AutoConnection);
    }
};

std::optional<quint8> FerricFrameBroker::DocumentReceipt::composedDepth() const {
    if(documents_.size()==1)return 0;
    return ancestry_?std::optional<quint8>(ancestry_->depth):std::nullopt;
}

void FerricFrameBroker::captureFragmentGeometry(const FragmentCatalogue &catalogue,
    std::function<void(std::optional<QJsonObject>)> completion,int timeoutMs) {
    if(!completion)return;
    if(timeoutMs<=0||timeoutMs>1000||!catalogue.data_
        ||!matchesReceipt(catalogue.data_->request.world)) {completion(std::nullopt);return;}
    const auto &receipt=catalogue.data_->request.world;
    const auto count=receipt.documents_.size();
    if(count<1||count>9||(count>1&&(!receipt.ancestry_||receipt.ancestry_->entries.size()!=count-1))) {
        completion(std::nullopt);return;
    }
    struct Read {
        std::shared_ptr<const FragmentCatalogueData> catalogue;
        std::function<void(std::optional<QJsonObject>)> completion;
        QDeadlineTimer deadline;
        QString runtime;
        QJsonArray sizes[2],mappings[2];
        int pass=0;
        qsizetype position=0;
    };
    auto read=std::make_shared<Read>(Read{catalogue.data_,std::move(completion),
        QDeadlineTimer(timeoutMs,Qt::PreciseTimer),count>1?hintGeometryRuntime():QString(),{},{},0,0});
    if(count>1&&read->runtime.isEmpty()){read->completion(std::nullopt);return;}
    auto advance=std::make_shared<std::function<void()>>();
    const std::weak_ptr<std::function<void()>> weakAdvance=advance;
    *advance=[this,read,weakAdvance,count] {
        const auto next=weakAdvance.lock();
        const auto &request=read->catalogue->request;
        const auto &world=request.world;
        const auto fail=[read]{read->completion(std::nullopt);};
        if(!next||read->deadline.hasExpired()||!matchesReceipt(world)){fail();return;}
        if(read->position==count*2-1) {
            const auto leaf=read->sizes[read->pass].last().toObject();
            if(leaf.value("width").toDouble()!=request.viewport.width()
                ||leaf.value("height").toDouble()!=request.viewport.height()){fail();return;}
            if(read->pass==0){read->pass=1;read->position=0;(*next)();return;}
            if(read->sizes[0]!=read->sizes[1]||read->mappings[0]!=read->mappings[1]){fail();return;}
            read->completion(QJsonObject{{"viewport",read->sizes[0].first()},
                {"mappings",read->mappings[0]}});return;
        }
        const bool mapping=read->position>=count;
        const qsizetype index=mapping?read->position-count:read->position;
        const auto &document=world.documents_[index];
        QString script=QStringLiteral("(()=>{try{if(window[")+quoted(documentProperty_)
            +QStringLiteral("]!==")+quoted(document.epoch)+QStringLiteral("||String(window.origin)!==")
            +quoted(document.origin)+QStringLiteral(")return null;");
        if(!mapping) {
            if(index==count-1) {
                script+=QStringLiteral("const s=window.__ferric_browserHintState,b=s&&s.interactionProof;"
                    "if(!s||s.dirty||!b||s.revision!==")+QString::number(request.revision)
                    +QStringLiteral("||b.revision!==s.revision||b.batch!==")+quoted(request.batch)
                    +QStringLiteral("||b.stamp!==")+quoted(request.stamp)
                    +QStringLiteral("||b.annotation!==")+quoted(request.annotation)
                    +QStringLiteral("||b.fragment_catalogue!==")+quoted(read->catalogue->json)
                    +QStringLiteral(")return null;");
            }
            script+=QStringLiteral("return JSON.stringify({width:innerWidth,height:innerHeight});");
        } else {
            const auto &entry=world.ancestry_->entries[index];
            script+=QStringLiteral("const verified=")+ancestryScript(world,index,entry.token,false)
                +QStringLiteral(";if(!verified)return null;")+read->runtime
                +QStringLiteral("const record=window.__ferric_hintNativeAncestry.get(")+quoted(entry.token)
                +QStringLiteral("),host=record&&WeakRef.prototype.deref.call(record.host),viewport=JSON.parse(")
                +quoted(QString::fromUtf8(QJsonDocument(read->sizes[read->pass][index+1].toObject()).toJson(QJsonDocument::Compact)))
                +QStringLiteral(");const mapping=host&&ftFrameMapping(host,ftLocalVisible(host,window,'pointer'),null,viewport);"
                    "if(!mapping)return null;delete mapping.parentView;return JSON.stringify(mapping);");
        }
        script+=QStringLiteral("}catch(error){return null;}})()");
        evaluate(document.frame,script,[read,next,mapping,fail](const QVariant &value) {
            if(value.metaType().id()!=QMetaType::QString){fail();return;}
            const auto parsed=QJsonDocument::fromJson(value.toString().toUtf8());
            if(!parsed.isObject()){fail();return;}
            const auto object=parsed.object();
            const auto number=[&object](const char *key,bool positive) {
                const auto value=object.value(QLatin1String(key));
                return value.isDouble()&&std::isfinite(value.toDouble())&&(!positive||value.toDouble()>0);
            };
            if(mapping) {
                const auto content=object.value("content").toObject();
                if(!number("left",false)||!number("top",false)||!number("scaleX",true)||!number("scaleY",true)
                    ||content.size()!=6||!content.value("left").isDouble()||!content.value("top").isDouble()
                    ||!content.value("right").isDouble()||!content.value("bottom").isDouble()
                    ||!(content.value("right").toDouble()>content.value("left").toDouble())
                    ||!(content.value("bottom").toDouble()>content.value("top").toDouble())) {fail();return;}
                read->mappings[read->pass].append(object);
            } else {
                if(object.size()!=2||!number("width",true)||!number("height",true)){fail();return;}
                read->sizes[read->pass].append(object);
            }
            ++read->position;(*next)();
        },static_cast<int>(read->deadline.remainingTime()));
    };
    (*advance)();
}

void FerricFrameBroker::projectFragmentCatalogue(const FragmentCatalogue &catalogue,
    std::function<void(std::optional<QJsonObject>)> completion,int timeoutMs,
    std::shared_ptr<FerricHintWorkTicket> work) {
    if(!completion){if(work)work->settle(FerricHintWorkTicket::Outcome::NoScript);return;}
    if(timeoutMs<=0||timeoutMs>1000||!catalogue.data_||!matchesReceipt(catalogue.data_->request.world)
        ||(work&&work->counter!=FerricHintWorkTicket::Counter::Graph)) {
        if(work)work->settle(FerricHintWorkTicket::Outcome::NoScript);
        completion(std::nullopt);return;
    }
    struct Sample { int candidate,fragment;QRectF rect,local;QPointF point;bool valid=true; };
    struct Projection {
        FragmentCatalogue catalogue;
        std::function<void(std::optional<QJsonObject>)> completion;
        std::shared_ptr<FerricHintWorkTicket> work;
        QDeadlineTimer deadline;
        QJsonObject geometry;
        QList<Sample> samples;
        std::map<int,QJsonObject> targets;
        quint32 used=0;
        qsizetype frame=0;
        bool limited=false;
    };
    const auto count=catalogue.data_->request.world.documents_.size();
    auto state=std::make_shared<Projection>(Projection{catalogue,std::move(completion),std::move(work),
        QDeadlineTimer(timeoutMs,Qt::PreciseTimer),{},{},{},static_cast<quint32>(4*count+4*(count-1)*1025),0,false});
    const auto fail=[state] {
        if(state->work)state->work->settle(FerricHintWorkTicket::Outcome::Unknown);
        state->completion(std::nullopt);
    };
    // Fund both double geometry reads and bounded ancestry walks up front.
    // Remaining ticks cover projection and worst-case common-point checks.
    captureFragmentGeometry(catalogue,[this,state,fail,count](std::optional<QJsonObject> geometry) {
        if(!geometry||state->deadline.hasExpired()){fail();return;}
        state->geometry=*geometry;
        const auto maps=geometry->value("mappings").toArray();
        const auto remember=[state](int candidate,int fragment,const QRectF &rect) {
            const auto previous=state->targets.find(candidate);
            if(previous!=state->targets.end()) {
                const auto old=previous->second.value("rect").toObject();
                const auto y=old.value("y").toDouble(),x=old.value("x").toDouble();
                if(y<rect.y()||(y==rect.y()&&(x<rect.x()||(x==rect.x()&&previous->second.value("fragment_index").toInt()<=fragment))))return;
            }
            state->targets[candidate]=QJsonObject{{"index",candidate},{"fragment_index",fragment},
                {"rect",QJsonObject{{"x",rect.x()},{"y",rect.y()},{"width",rect.width()},{"height",rect.height()}}}};
        };
        if(count>1)for(const auto &value:state->catalogue.targets()) {
            const auto target=value.toObject(),fragments=target.value("fragments").toObject();
            const auto candidate=target.value("candidate").toObject().value("index").toInt(-1);
            const auto policy=fragments.value("policy").toString();
            for(const auto &alternative:fragments.value("eligible_fragments").toArray()) {
                if(state->used+maps.size()>FerricHintWorkTicket::limit){state->limited=true;break;}
                const auto source=alternative.toObject(),r=source.value("rect").toObject();
                QRectF rect(r.value("x").toDouble(),r.value("y").toDouble(),r.value("width").toDouble(),r.value("height").toDouble());
                for(qsizetype index=maps.size();index-->0;) {
                    ++state->used;
                    const auto map=maps[index].toObject(),clip=map.value("content").toObject();
                    const auto sx=map.value("scaleX").toDouble(),sy=map.value("scaleY").toDouble();
                    rect=QRectF(map.value("left").toDouble()+rect.x()*sx,map.value("top").toDouble()+rect.y()*sy,
                        rect.width()*sx,rect.height()*sy).intersected(QRectF(clip.value("left").toDouble(),clip.value("top").toDouble(),
                        clip.value("width").toDouble(),clip.value("height").toDouble()));
                    if(rect.isEmpty())break;
                }
                if(rect.isEmpty())continue;
                const auto fragment=source.value("index").toInt(-1);
                if(policy!=QStringLiteral("pointer")&&policy!=QStringLiteral("link")){remember(candidate,fragment,rect);continue;}
                const auto cost=static_cast<quint32>(3*(count+8));
                if(state->used+cost>FerricHintWorkTicket::limit){state->limited=true;break;}
                state->used+=cost;
                auto local=rect;
                for(const auto &value:maps) {
                    const auto map=value.toObject();
                    local=QRectF((local.x()-map.value("left").toDouble())/map.value("scaleX").toDouble(),
                        (local.y()-map.value("top").toDouble())/map.value("scaleY").toDouble(),
                        local.width()/map.value("scaleX").toDouble(),local.height()/map.value("scaleY").toDouble());
                }
                const auto ix=std::min(4.,rect.width()/2),iy=std::min(4.,rect.height()/2);
                for(const auto &point:{rect.center(),QPointF(rect.left()+ix,rect.top()+iy),QPointF(rect.right()-ix,rect.bottom()-iy)})
                    state->samples.append(Sample{candidate,fragment,rect,local,point,true});
            }
            if(state->limited)break;
        }
        auto advance=std::make_shared<std::function<void()>>();
        const std::weak_ptr<std::function<void()>> weakAdvance=advance;
        *advance=[this,state,fail,count,maps,remember,weakAdvance] {
            const auto next=weakAdvance.lock();
            const auto &request=state->catalogue.data_->request;
            const auto &world=request.world;
            if(!next||state->deadline.hasExpired()||!matchesReceipt(world)){fail();return;}
            if(state->frame==count) {
                for(const auto &sample:state->samples)if(sample.valid)remember(sample.candidate,sample.fragment,sample.rect);
                captureFragmentGeometry(state->catalogue,[state,fail](std::optional<QJsonObject> sealed) {
                    if(!sealed||*sealed!=state->geometry||state->deadline.hasExpired()){fail();return;}
                    QJsonArray targets;for(const auto &entry:state->targets)targets.append(entry.second);
                    auto result=state->geometry;result.insert("targets",targets);result.insert("limited",state->limited);
                    if(state->work&&!state->work->settle(FerricHintWorkTicket::Outcome::Known,state->used)){state->completion(std::nullopt);return;}
                    state->completion(result);
                },static_cast<int>(state->deadline.remainingTime()));return;
            }
            QJsonArray points;QList<qsizetype> indices;
            for(qsizetype index=0;index<state->samples.size();++index) {
                const auto &sample=state->samples[index];if(!sample.valid)continue;
                auto point=sample.point;
                for(qsizetype parent=0;parent<state->frame;++parent) {
                    const auto map=maps[parent].toObject();
                    point=QPointF((point.x()-map.value("left").toDouble())/map.value("scaleX").toDouble(),
                        (point.y()-map.value("top").toDouble())/map.value("scaleY").toDouble());
                }
                points.append(QJsonArray{sample.candidate,point.x(),point.y(),sample.local.x(),sample.local.y(),sample.local.width(),sample.local.height()});
                indices.append(index);
            }
            if(points.isEmpty()){++state->frame;(*next)();return;}
            const auto &document=world.documents_[state->frame];
            QString script=QStringLiteral("(()=>{try{if(window[")+quoted(documentProperty_)+QStringLiteral("]!==")+quoted(document.epoch)
                +QStringLiteral("||String(window.origin)!==")+quoted(document.origin)+QStringLiteral(")return null;")+hintGeometryRuntime()
                +QStringLiteral("const points=JSON.parse(")+quoted(QString::fromUtf8(QJsonDocument(points).toJson(QJsonDocument::Compact)))+QStringLiteral(");");
            if(state->frame<count-1) {
                const auto token=world.ancestry_->entries[state->frame].token;
                script+=QStringLiteral("const records=window.__ferric_hintNativeAncestry,entry=records&&records.get(")+quoted(token)
                    +QStringLiteral(");const host=entry&&WeakRef.prototype.deref.call(entry.host);if(!host||!host.isConnected)return null;"
                        "return JSON.stringify(points.map(p=>ftContains(host,ftDeepHit(document,p[1],p[2]))));");
            } else {
                script+=QStringLiteral("const s=window.__ferric_browserHintState,b=s&&s.interactionProof;"
                    "if(!s||s.dirty||!b||s.revision!==")+QString::number(request.revision)
                    +QStringLiteral("||b.batch!==")+quoted(request.batch)+QStringLiteral("||b.fragment_catalogue!==")+quoted(state->catalogue.json())
                    +QStringLiteral(")return null;let used=0;function inspect(){return ++used<=points.length*9;}"
                    "return JSON.stringify(points.map(p=>{if(!inspect())return false;const target=b.targets.get(p[0]),"
                    "el=target&&WeakRef.prototype.deref.call(target.element),ref=target&&target.reference,record=ref&&s.elements.get(ref.element_id);"
                    "if(!el||!el.isConnected||!record||record.element!==el||record.ownerDocument!==el.ownerDocument)return false;"
                    "const f=target.fragment.fragments,c=f.owner_document_clip_css,evidence={complete:true};"
                    "return ftHintFragmentHit(el,{left:c.x,top:c.y,right:c.x+c.width,bottom:c.y+c.height},"
                    "{x:p[3],y:p[4],width:p[5],height:p[6]},f.policy,null,evidence,{x:p[1],y:p[2]})&&evidence.complete;}));");
            }
            script+=QStringLiteral("}catch(error){return null;}})()");
            evaluate(document.frame,script,[state,next,fail,indices](const QVariant &value) {
                if(value.metaType().id()!=QMetaType::QString){fail();return;}
                const auto parsed=QJsonDocument::fromJson(value.toString().toUtf8());
                if(!parsed.isArray()||parsed.array().size()!=indices.size()){fail();return;}
                const auto values=parsed.array();
                for(qsizetype index=0;index<indices.size();++index) {
                    if(!values[index].isBool()){fail();return;}
                    state->samples[indices[index]].valid=values[index].toBool();
                }
                ++state->frame;(*next)();
            },static_cast<int>(state->deadline.remainingTime()));
        };
        (*advance)();
    },static_cast<int>(state->deadline.remainingTime()));
}

struct FerricFrameBroker::AncestryCapture {
    DocumentReceipt receipt;
    std::shared_ptr<AncestryLease> lease;
    ReceiptCompletion completion;
    QDeadlineTimer deadline;
    qsizetype index=0;
    FerricHintDocumentAdmission documentAdmission;
};

struct FerricFrameBroker::Pending {
    Completion completion;
    std::shared_ptr<FerricHintWorkTicket> workTicket;
    QWebEngineFrame frame;
    QPointer<QTimer> timer;
    bool delivered = false;
    bool engineFinished = false;
    bool dispatched = false;
};

struct FerricFrameBroker::Capture {
    explicit Capture(DocumentReceipt value) : receipt(std::move(value)) {}
    DocumentReceipt receipt;
    ReceiptCompletion completion;
    QDeadlineTimer deadline{1000, Qt::PreciseTimer};
    qsizetype index = 0;
    bool verifying = false;
    std::optional<DocumentWork> work;
};

FerricFrameBroker::~FerricFrameBroker() { releaseAncestry(); cancelPending(); }

void FerricFrameBroker::releaseAncestry() {
    dispatcher_->releaseAll();
}

void FerricFrameBroker::cancelPending() {
    QList<std::shared_ptr<FerricHintWorkTicket>> tickets;
    for (const auto &entry : std::as_const(pending_)) {
        const auto pending = entry.lock();
        if (!pending) continue;
        if (!pending->engineFinished && !pending->frame.isValid()) {
            pending->engineFinished = true;
            dispatcher_->finishOrdinary();
        }
        if (pending->delivered) continue;
        pending->delivered = true;
        pending->completion = {};
        if (pending->timer) {
            pending->timer->stop();
            pending->timer->deleteLater();
            pending->timer.clear();
        }
        // Accounting survives suppression of the user continuation. A queued
        // script cannot be called NoScript, even when its frame has retired.
        if(auto ticket=std::exchange(pending->workTicket,{}))tickets.append(std::move(ticket));
    }
    // Settlement may reenter; finish all accesses to broker-owned state first.
    for(const auto &ticket:tickets)ticket->settle(FerricHintWorkTicket::Outcome::Unknown);
}

void FerricFrameBroker::deliver(const QJSValue &callback, const QVariant &value) {
    if (callback.isCallable()) {
        if (auto *engine = qjsEngine(this)) callback.call({engine->toScriptValue(value)});
    }
}

void FerricFrameBroker::evaluate(QWebEngineFrame frame, const QString &script,
                                    Completion completion, int timeoutMs,
                                    std::shared_ptr<FerricHintWorkTicket> workTicket) {
    const QDeadlineTimer deadline(timeoutMs,Qt::PreciseTimer);
    const QPointer<FerricFrameBroker> guard(this);
    const auto generation = generation_;
    if (timeoutMs <= 0) {
        if(workTicket)workTicket->settle(FerricHintWorkTicket::Outcome::NoScript);
        if(guard&&guard->generation_==generation)completion(failure("frame-timeout"));
        return;
    }
    QList<std::shared_ptr<FerricHintWorkTicket>> retiredTickets;
    pending_.erase(std::remove_if(pending_.begin(), pending_.end(), [this,&retiredTickets](const auto &entry) {
        const auto pending = entry.lock();
        if (pending && !pending->engineFinished && !pending->frame.isValid()) {
            pending->engineFinished = true;
            dispatcher_->finishOrdinary();
            if(auto ticket=std::exchange(pending->workTicket,{}))retiredTickets.append(std::move(ticket));
        }
        return !pending || pending->engineFinished;
    }), pending_.end());
    for(const auto &ticket:retiredTickets)ticket->settle(FerricHintWorkTicket::Outcome::Unknown);
    if(!guard||guard->generation_!=generation) {
        if(workTicket)workTicket->settle(FerricHintWorkTicket::Outcome::NoScript);
        return;
    }
    if(deadline.hasExpired()) {
        if(workTicket)workTicket->settle(FerricHintWorkTicket::Outcome::NoScript);
        if(guard&&guard->generation_==generation)completion(failure("frame-timeout"));
        return;
    }
    if (!frame.isValid() || !dispatcher_->reserveOrdinary()) {
        if(workTicket)workTicket->settle(FerricHintWorkTicket::Outcome::NoScript);
        if(guard&&guard->generation_==generation)completion(failure("frame-operation-limit"));
        return;
    }
    auto pending = std::make_shared<Pending>();
    pending->completion = std::move(completion);
    pending->workTicket = std::move(workTicket);
    pending->frame = frame;
    auto *timer = new QTimer(this);
    timer->setSingleShot(true);
    timer->setTimerType(Qt::PreciseTimer);
    pending->timer = timer;
    pending_.append(pending);
    auto finish = [guard, generation, deadline, pending, dispatcher = dispatcher_](const QVariant &value, bool fromEngine) {
        // Delivery expiry cannot cancel a script already queued in WebEngine.
        // Release its dispatch slot only when that native callback completes.
        if (!pending->engineFinished && (fromEngine || !pending->dispatched || !pending->frame.isValid())) {
            pending->engineFinished = true;
            dispatcher->finishOrdinary();
        }
        if (pending->delivered) return;
        pending->delivered = true;
        if (pending->timer) { pending->timer->stop(); pending->timer->deleteLater(); pending->timer.clear(); }
        auto completion = std::move(pending->completion);
        pending->completion = {};
        QVariant result=fromEngine&&deadline.hasExpired()?failure("frame-timeout"):value;
        if(auto ticket=std::exchange(pending->workTicket,{})) {
            if(!pending->dispatched)ticket->settle(FerricHintWorkTicket::Outcome::NoScript);
            else {
                bool known=false;
                quint32 steps=0;
                if(fromEngine&&guard&&guard->generation_==generation&&!deadline.hasExpired()
                    &&pending->frame.isValid()&&value.metaType().id()==QMetaType::QVariantMap) {
                    const auto report=value.toMap();
                    const auto version=report.value(QStringLiteral("version"));
                    const bool graph=ticket->counter==FerricHintWorkTicket::Counter::Graph;
                    const auto spent=report.value(graph?QStringLiteral("graph_steps"):QStringLiteral("document_probes"));
                    const auto actual=report.value(QStringLiteral("value"));
                    const auto number=spent.toDouble();
                    const auto numeric=[](const QVariant &value) {
                        return value.metaType().id()==QMetaType::Double||value.metaType().id()==QMetaType::Int
                            ||value.metaType().id()==QMetaType::UInt;
                    };
                    known=report.size()==3&&report.contains(QStringLiteral("value"))&&numeric(version)&&version.toDouble()==1
                        &&numeric(spent)&&std::isfinite(number)&&number>=0&&std::floor(number)==number
                        &&(graph?number<=FerricHintWorkTicket::limit:number==1)
                        &&(!graph||actual.metaType().id()==QMetaType::Bool||actual.metaType().id()==QMetaType::QString);
                    if(known){steps=static_cast<quint32>(number);result=actual;}
                }
                const bool settled=ticket->settle(known?FerricHintWorkTicket::Outcome::Known:
                    FerricHintWorkTicket::Outcome::Unknown,steps);
                if(!known||!settled)result=failure("graph-work-report");
            }
        }
        if (guard && guard->generation_ == generation && completion)
            completion(result);
    };
    connect(timer, &QTimer::timeout, this, [finish] { finish(failure("frame-timeout"), false); });
    if(deadline.hasExpired()){finish(failure("frame-timeout"),false);return;}
    timer->start(static_cast<int>(deadline.remainingTime()));
    pending->dispatched=true;
    frame.runJavaScript(script, QWebEngineScript::ApplicationWorld,
                        [finish](const QVariant &value) { finish(value, true); });
}

void FerricFrameBroker::evaluateGraph(QWebEngineFrame frame,const QString &script,Completion completion,
                                     int timeoutMs,const FerricHintGraphAdmission &admission,quint8 pass) {
    const QDeadlineTimer deadline(timeoutMs,Qt::PreciseTimer);
    const QPointer<FerricFrameBroker> guard(this);
    const auto generation=generation_;
    auto ticket=admission?admission(pass):std::shared_ptr<FerricHintWorkTicket>();
    if(!guard||generation_!=generation) {
        if(ticket)ticket->settle(FerricHintWorkTicket::Outcome::NoScript);
        return;
    }
    if(admission&&!ticket){completion(failure("graph-work-admission"));return;}
    if(ticket&&ticket->counter!=FerricHintWorkTicket::Counter::Graph) {
        ticket->settle(FerricHintWorkTicket::Outcome::Unknown);
        if(guard&&guard->generation_==generation)completion(failure("graph-work-admission"));
        return;
    }
    const auto wrapped=QStringLiteral("(()=>{const graphWork={steps:0};const value=(")+script
        +QStringLiteral(");return ")+(ticket?QStringLiteral("{version:1,value:value,graph_steps:graphWork.steps}"):
            QStringLiteral("value"))+QStringLiteral(";})()");
    evaluate(frame,wrapped,std::move(completion),static_cast<int>(deadline.remainingTime()),std::move(ticket));
}

void FerricFrameBroker::evaluateDocument(QWebEngineFrame frame,const QString &script,Completion completion,
    int timeoutMs,const std::optional<DocumentWork> &work,quint8 document,quint8 pass) {
    if(!work){evaluate(frame,script,std::move(completion),timeoutMs);return;}
    const QDeadlineTimer deadline(timeoutMs,Qt::PreciseTimer);
    const QPointer<FerricFrameBroker> guard(this);
    const auto generation=generation_;
    auto ticket=work->admission?work->admission(work->checkpoint,document,pass):std::shared_ptr<FerricHintWorkTicket>();
    if(!guard||guard->generation_!=generation) {
        if(ticket)ticket->settle(FerricHintWorkTicket::Outcome::NoScript);
        return;
    }
    if(!ticket){completion(failure("document-work-admission"));return;}
    if(ticket->counter!=FerricHintWorkTicket::Counter::Document) {
        ticket->settle(FerricHintWorkTicket::Outcome::Unknown);
        if(guard&&guard->generation_==generation)completion(failure("document-work-admission"));
        return;
    }
    // One dispatch unit on every return/exception, including epoch rejection.
    // This does not meter String/window-origin work or Qt result allocation.
    const auto wrapped=QStringLiteral("(()=>{let value=null;try{value=(")+script
        +QStringLiteral(");}catch(error){}return {version:1,value:value,document_probes:1};})()");
    evaluate(frame,wrapped,std::move(completion),static_cast<int>(deadline.remainingTime()),std::move(ticket));
}

bool FerricFrameBroker::reachable(const QString &owner) const {
    auto current = frames_.constFind(owner);
    for (int depth = 0; current != frames_.cend() && depth <= 8; ++depth) {
        if (!current->frame.isValid()) return false;
        if (current->parent.isEmpty()) return current->frame.isMainFrame();
        const auto parent = frames_.constFind(current->parent);
        if (parent == frames_.cend() || !parent->frame.isValid()
            || !parent->frame.children().contains(current->frame)) return false;
        current = parent;
    }
    return false;
}

void FerricFrameBroker::attachFrame(const QWebEngineFrame &frame, const QString &parent,
                                       Completion completion, const QDeadlineTimer &deadline, bool allowRebind) {
    if (deadline.hasExpired()) { completion(failure("frame-timeout")); return; }
    if (!frame.isValid()) { completion(failure("frame-limit-or-invalid")); return; }
    for (auto it = frames_.cbegin(); it != frames_.cend(); ++it) {
        if (it->frame == frame && it->parent == parent) {
            // At most one old-document retirement and one initialization may
            // belong to an attachment. Concurrent rebinding cannot restart it.
            if (!allowRebind) { completion(failure("frame-rebind-raced")); return; }
            const auto owner = it.key();
            const auto epoch = it->epoch;
            evaluate(frame, QStringLiteral("window.") + documentProperty_ + QStringLiteral("===") + quoted(epoch),
                     [this, frame, parent, owner, completion, deadline](const QVariant &value) {
                if (deadline.hasExpired()) { completion(failure("frame-timeout")); return; }
                if (value.metaType().id() != QMetaType::Bool) {
                    completion(failure("frame-document-unavailable")); return;
                }
                if (value.toBool() && reachable(owner)) completion(QVariantMap{{"owner", owner}});
                else {
                    // A new collection may bind the same native frame's new
                    // document. Old owner IDs remain invalid for pending actions.
                    QSet<QString> retired{owner};
                    for (int depth = 0; depth <= 8; ++depth)
                        for (auto it = frames_.cbegin(); it != frames_.cend(); ++it)
                            if (retired.contains(it->parent)) retired.insert(it.key());
                    for (const auto &id : retired) frames_.remove(id);
                    attachFrame(frame, parent, completion, deadline, false);
                }
            }, static_cast<int>(deadline.remainingTime()));
            return;
        }
    }
    if (frames_.size() >= 64) { completion(failure("frame-limit-or-invalid")); return; }
    const auto owner = QStringLiteral("frame-") + QUuid::createUuid().toString(QUuid::WithoutBraces);
    const auto epoch = QUuid::createUuid().toString(QUuid::WithoutBraces);
    evaluate(frame, QStringLiteral("(()=>{window.") + documentProperty_ + QStringLiteral("=") + quoted(epoch)
             + QStringLiteral(";return true;})()"),
             [this, frame, parent, owner, epoch, completion, deadline](const QVariant &value) {
        if (deadline.hasExpired()) { completion(failure("frame-timeout")); return; }
        if (value.metaType().id() != QMetaType::Bool || !value.toBool() || !frame.isValid()) {
            completion(failure("frame-initialization-failed")); return;
        }
        frames_.insert(owner, Lease{frame, epoch, parent});
        if (!reachable(owner)) { frames_.remove(owner); completion(failure("invalid-frame")); return; }
        const QPointer<FerricFrameBroker> guard(this);
        const auto generation = generation_;
        emit leasesChanged();
        if (!guard || guard->generation_ != generation || !guard->reachable(owner)) return;
        if (deadline.hasExpired()) { completion(failure("frame-timeout")); return; }
        completion(QVariantMap{{"owner", owner}});
    }, static_cast<int>(deadline.remainingTime()));
}

void FerricFrameBroker::attach(const QWebEngineFrame &frame, const QJSValue &callback) {
    if (!frame.isMainFrame()) { deliver(callback, failure("main-frame-required")); return; }
    attachFrame(frame, {}, [this, callback](const QVariant &value) { deliver(callback, value); });
}

void FerricFrameBroker::scanFocusedChild(const QString &parent,
                                              std::function<void(FocusScanResult)> completion,
                                              int timeoutMs) {
    if (!reachable(parent)) { completion({QStringLiteral("invalid-parent-frame"), {}}); return; }
    if (timeoutMs <= 0) { completion({QStringLiteral("frame-timeout"), {}}); return; }
    const auto lease = frames_.value(parent);
    const auto children = lease.frame.children();
    if (children.size() > 64) { completion({QStringLiteral("frame-enumeration-limit"), {}}); return; }
    for (const auto &child : children) {
        if (!inspected_.contains(child)) {
            if (inspected_.size() >= 64) { completion({QStringLiteral("frame-enumeration-limit"), {}}); return; }
            inspected_.append(child);
        }
    }
    // Absence is successful only for a fully observed, unchanged child set.
    // An incomplete/ambiguous scan must never be interpreted as a terminal leaf.
    if (children.isEmpty()) { completion({{}, {}}); return; }
    struct Scan { int remaining; QList<QWebEngineFrame> focused; bool failed = false; };
    auto scan = std::make_shared<Scan>();
    scan->remaining = children.size();
    const QDeadlineTimer deadline(timeoutMs, Qt::PreciseTimer);
    for (const auto &child : children) {
        evaluate(child, QStringLiteral("document.hasFocus()"),
                 [this, scan, child, parent, lease, children, deadline, completion](const QVariant &value) {
            if (value.metaType().id() != QMetaType::Bool) scan->failed = true;
            else if (value.toBool()) scan->focused.append(child);
            if (--scan->remaining != 0) return;
            const auto current = frames_.constFind(parent);
            if (deadline.hasExpired()) { completion({QStringLiteral("frame-timeout"), {}}); return; }
            if (current == frames_.cend() || current->frame != lease.frame
                || current->epoch != lease.epoch || current->parent != lease.parent
                || !reachable(parent) || lease.frame.children() != children) {
                completion({QStringLiteral("frame-association-changed"), {}}); return;
            }
            if (scan->failed || scan->focused.size() > 1) {
                completion({QStringLiteral("no-unique-focused-child"), {}}); return;
            }
            completion({{}, scan->focused.isEmpty() ? std::nullopt
                : std::optional<QWebEngineFrame>(scan->focused.first())});
        }, static_cast<int>(deadline.remainingTime()));
    }
}

void FerricFrameBroker::focusedChild(const QString &parent, const QJSValue &callback) {
    const QDeadlineTimer deadline(1000, Qt::PreciseTimer);
    scanFocusedChild(parent, [this, parent, callback, deadline](FocusScanResult result) {
        if (!result.error.isEmpty()) { deliver(callback, QVariantMap{{"error", result.error}}); return; }
        if (!result.child) { deliver(callback, failure("no-unique-focused-child")); return; }
        attachFrame(*result.child, parent,
                    [this, callback](const QVariant &value) { deliver(callback, value); }, deadline);
    }, static_cast<int>(deadline.remainingTime()));
}

void FerricFrameBroker::verifyFocusedLeaf(const QString &owner, const QJSValue &callback) {
    verifyFocusedLeafNative(owner, [this, callback](const QVariant &value) { deliver(callback,value); });
}

void FerricFrameBroker::verifyFocusedLeafNative(const QString &owner, Completion completion, int timeoutMs) {
    if(!completion)return;
    if(timeoutMs<=0||timeoutMs>1000){completion(failure("frame-timeout"));return;}
    if (!reachable(owner)) { completion(failure("invalid-frame")); return; }
    const auto lease = frames_.value(owner);
    const auto children = lease.frame.children();
    const QDeadlineTimer deadline(timeoutMs, Qt::PreciseTimer);
    const auto currentLease = [this, owner, lease, children] {
        const auto current = frames_.constFind(owner);
        return current != frames_.cend() && current->frame == lease.frame
            && current->epoch == lease.epoch && current->parent == lease.parent
            && reachable(owner) && lease.frame.children() == children;
    };
    const auto probe = QStringLiteral("window.") + documentProperty_
        + QStringLiteral("===") + quoted(lease.epoch) + QStringLiteral(" && document.hasFocus()");
    evaluate(lease.frame, probe, [this, owner, lease, hasChildren = !children.isEmpty(), deadline, currentLease, completion](const QVariant &value) {
        if (value.metaType().id() != QMetaType::Bool || !value.toBool()) {
            completion(failure("frame-focus-lost")); return;
        }
        if (!currentLease()) { completion(failure("frame-association-changed")); return; }
        scanFocusedChild(owner, [this, lease, hasChildren, deadline, currentLease, completion](FocusScanResult result) {
            if (deadline.hasExpired()) { completion(failure("frame-timeout")); return; }
            if (!result.error.isEmpty()) { completion(QVariantMap{{"error", result.error}}); return; }
            if (result.child) { completion(failure("frame-focus-changed")); return; }
            // Focus can move into an existing child after that child's reply.
            // Recheck the exposed active path, bounded like hintFrameHost. A
            // closed shadow host can hide a post-sample transition. Until
            // exact child-host association is available, that exposed terminal
            // is unknown when native children exist, never proved absent.
            // These observations are not an atomic cross-frame focus snapshot.
            const auto finalProbe = QStringLiteral("(()=>{if(window.") + documentProperty_
                + QStringLiteral("!==") + quoted(lease.epoch) + QStringLiteral(")return false;const check=")
                + focusedLeafPredicate() + QStringLiteral(";return check(document,")
                + (hasChildren ? QStringLiteral("true") : QStringLiteral("false")) + QStringLiteral(");})()");
            evaluate(lease.frame, finalProbe,
                     [this, deadline, currentLease, completion](const QVariant &verified) {
                if (deadline.hasExpired()) { completion(failure("frame-timeout")); return; }
                if (!currentLease()) { completion(failure("frame-association-changed")); return; }
                if (verified.metaType().id() != QMetaType::Bool) {
                    completion(failure("frame-focus-unavailable")); return;
                }
                if (!verified.toBool()) { completion(failure("frame-focus-changed")); return; }
                completion(true);
            }, static_cast<int>(deadline.remainingTime()));
        }, static_cast<int>(deadline.remainingTime()));
    }, static_cast<int>(deadline.remainingTime()));
}

void FerricFrameBroker::run(const QString &owner, const QString &script, const QJSValue &callback) {
    execute(owner, script, callback, false);
}

void FerricFrameBroker::dispatch(const QString &owner, const QString &script, const QJSValue &callback) {
    execute(owner, script, callback, true);
}

void FerricFrameBroker::execute(const QString &owner, const QString &script,
                                   const QJSValue &callback, bool action) {
    if (!reachable(owner) || script.size() > 1024 * 1024) {
        deliver(callback, failure("invalid-frame-operation")); return;
    }
    const auto lease = frames_.value(owner);
    const QDeadlineTimer deadline(1000, Qt::PreciseTimer);
    const auto currentLease = [this, owner, lease] {
        const auto current = frames_.constFind(owner);
        return current != frames_.cend() && current->frame == lease.frame
            && current->epoch == lease.epoch && current->parent == lease.parent
            && reachable(owner);
    };
    const auto wrapped = QStringLiteral("(()=>{if(window.") + documentProperty_ + QStringLiteral("!==")
        + quoted(lease.epoch) + QStringLiteral(")return {error:'stale-frame-document'};return (")
        + script + QStringLiteral(");})()");
    evaluate(lease.frame, wrapped,
             [this, callback, action, lease, deadline, currentLease](const QVariant &value) {
        if (deadline.hasExpired()) {
            deliver(callback, currentLease() ? failure("frame-timeout") : failure("invalid-frame")); return;
        }
        // A successful action may intentionally remove its own frame. This
        // exception remains dispatch-only and requires an exact boolean true.
        const bool acknowledged = action && value.metaType().id() == QMetaType::Bool && value.toBool();
        if (acknowledged) { deliver(callback, value); return; }
        if (!currentLease()) { deliver(callback, failure("invalid-frame")); return; }
        // Native frame reachability survives some document navigations. Check
        // the captured document again, sharing the original operation deadline.
        // The retained QVariant is copied; the first callback reference expires.
        const auto probe = QStringLiteral("window.") + documentProperty_
            + QStringLiteral("===") + quoted(lease.epoch);
        evaluate(lease.frame, probe,
                 [this, callback, deadline, currentLease, retained = QVariant(value)](const QVariant &receipt) {
            if (deadline.hasExpired()) { deliver(callback, failure("frame-timeout")); return; }
            if (!currentLease()) { deliver(callback, failure("invalid-frame")); return; }
            // This fixed probe returns only a boolean. Maps are native evaluate
            // failures, including the shared deadline; preserve their category.
            if (receipt.metaType().id() == QMetaType::QVariantMap) {
                deliver(callback, receipt); return;
            }
            const bool sameDocument = receipt.metaType().id() == QMetaType::Bool && receipt.toBool();
            deliver(callback, deadline.hasExpired() ? failure("frame-timeout")
                : sameDocument ? retained : failure("stale-frame-document"));
        }, static_cast<int>(deadline.remainingTime()));
    }, static_cast<int>(deadline.remainingTime()));
}

void FerricFrameBroker::clear() {
    const QPointer<FerricFrameBroker> guard(this);
    releaseAncestry();
    ++generation_;
    cancelPending();
    if(!guard)return;
    frames_.clear();
    inspected_.clear();
    emit leasesChanged();
}

bool FerricFrameBroker::matchesReceipt(const DocumentReceipt &receipt) const {
    if (QThread::currentThread() != thread() || receipt.broker_ != this
        || receipt.generation_ != generation_ || receipt.documents_.isEmpty()
        || receipt.documents_.size() > 9) return false;
    for (const auto &document : receipt.documents_) {
        const auto current = frames_.constFind(document.owner);
        if (current == frames_.cend() || current->frame != document.frame
            || current->epoch != document.epoch || current->parent != document.parent
            || !reachable(document.owner) || current->frame.url() != document.url) return false;
    }
    return true;
}

void FerricFrameBroker::capturePageFocus(const QWebEngineFrame &mainFrame, const QString &script,
                                        FocusCompletion completion) {
    if (!completion) return;
    if (!mainFrame.isValid() || !mainFrame.isMainFrame() || script.isEmpty()
        || script.size() > 256 * 1024) { completion(std::nullopt, {}); return; }
    const QDeadlineTimer deadline(1000, Qt::PreciseTimer);
    attachFrame(mainFrame, {}, [this, script, deadline, completion](const QVariant &value) {
        const auto owner = value.toMap().value(QStringLiteral("owner")).toString();
        if (owner.isEmpty()) { qCDebug(pageFocusLog) << "root attachment" << value; completion(std::nullopt, {}); return; }
        capturePageFocusAt(owner, 0, script, deadline, completion);
    }, deadline);
}

void FerricFrameBroker::capturePageFocusAt(const QString &owner, int depth, const QString &script,
                                         const QDeadlineTimer &deadline, FocusCompletion completion) {
    if (deadline.hasExpired()) { completion(std::nullopt, {}); return; }
    scanFocusedChild(owner, [this, owner, depth, script, deadline, completion](FocusScanResult scan) {
        if (!scan.error.isEmpty() || deadline.hasExpired()) { qCDebug(pageFocusLog) << "child scan" << scan.error << deadline.hasExpired(); completion(std::nullopt, {}); return; }
        if (scan.child) {
            if (depth >= 8) { completion(std::nullopt, {}); return; }
            attachFrame(*scan.child, owner, [this, depth, script, deadline, completion](const QVariant &value) {
                const auto child = value.toMap().value(QStringLiteral("owner")).toString();
                if (child.isEmpty()) { qCDebug(pageFocusLog) << "child attachment" << value; completion(std::nullopt, {}); return; }
                capturePageFocusAt(child, depth + 1, script, deadline, completion);
            }, deadline);
            return;
        }
        captureDocument(owner, [this, script, deadline, completion](std::optional<DocumentReceipt> receipt) {
            if (!receipt || deadline.hasExpired() || !receipt->documents().last().focused) {
                qCDebug(pageFocusLog) << "document capture" << bool(receipt) << deadline.hasExpired()
                    << (receipt && receipt->documents().last().focused);
                completion(std::nullopt, {}); return;
            }
            const auto document = receipt->documents().last();
            const auto wrapped = QStringLiteral("(()=>{if(window[") + quoted(documentProperty_)
                + QStringLiteral("]!==") + quoted(document.epoch) + QStringLiteral(")return null;return eval(")
                + quoted(script) + QStringLiteral(");})()");
            evaluate(document.frame, wrapped, [this, receipt, deadline, completion](const QVariant &value) {
                const auto state = value.toMap();
                if (deadline.hasExpired() || state.isEmpty() || !matchesReceipt(*receipt)) {
                    qCDebug(pageFocusLog) << "focus state" << deadline.hasExpired() << state.isEmpty() << matchesReceipt(*receipt);
                    completion(std::nullopt, {}); return;
                }
                const auto checked = [receipt, state, completion](bool valid) {
                    if (!valid) qCDebug(pageFocusLog) << "focus path revalidation";
                    completion(valid ? receipt : std::nullopt, valid ? state : QVariantMap{});
                };
                revalidateDocument(*receipt, [this, receipt, checked, deadline](bool valid) {
                    if (!valid) { qCDebug(pageFocusLog) << "document revalidation"; checked(false); return; }
                    verifyPageFocusPath(*receipt, 0, deadline, checked);
                }, static_cast<int>(deadline.remainingTime()));
            }, static_cast<int>(deadline.remainingTime()));
        }, static_cast<int>(deadline.remainingTime()));
    }, static_cast<int>(deadline.remainingTime()));
}

void FerricFrameBroker::verifyPageFocusPath(const DocumentReceipt &receipt, int index,
    const QDeadlineTimer &deadline, std::function<void(bool)> completion) {
    if (deadline.hasExpired() || !matchesReceipt(receipt)
        || index >= receipt.documents().size()
        || std::any_of(receipt.documents().cbegin(), receipt.documents().cend(),
            [](const auto &document) { return !document.focused; })) { completion(false); return; }
    // Keyboard eligibility observes native document focus, not a Hint target or
    // DOM-root witness. A complete child scan must agree with every native hop.
    // This remains an asynchronous observation, not an atomic focus lock.
    const auto children = receipt.documents()[index].frame.children();
    scanFocusedChild(receipt.documents()[index].owner,
        [this, receipt, index, children, deadline, completion](FocusScanResult scan) {
            if (!scan.error.isEmpty() || deadline.hasExpired() || !matchesReceipt(receipt)
                || receipt.documents()[index].frame.children() != children) { completion(false); return; }
            if (index + 1 < receipt.documents().size()) {
                if (!scan.child || *scan.child != receipt.documents()[index + 1].frame) { completion(false); return; }
                verifyPageFocusPath(receipt, index + 1, deadline, completion);
            } else if (scan.child) completion(false);
            else revalidateDocument(receipt, completion, static_cast<int>(deadline.remainingTime()));
        }, static_cast<int>(deadline.remainingTime()));
}

void FerricFrameBroker::captureDocument(const QString &owner, ReceiptCompletion completion, int timeoutMs,
                                        std::optional<DocumentWork> work) {
    captureDocumentImpl(owner,std::move(completion),timeoutMs,std::move(work),std::nullopt);
}

void FerricFrameBroker::captureDocumentImpl(const QString &owner, ReceiptCompletion completion, int timeoutMs,
                                            std::optional<DocumentWork> work,std::optional<DocumentReceipt> original) {
    if (!completion) return;
    if (QThread::currentThread() != thread() || !reachable(owner) || timeoutMs <= 0 || timeoutMs > 1000) {
        completion(std::nullopt); return;
    }
    auto capture = std::make_shared<Capture>(DocumentReceipt{});
    capture->completion = std::move(completion);
    capture->deadline = QDeadlineTimer(timeoutMs, Qt::PreciseTimer);
    capture->receipt.broker_ = this;
    capture->receipt.generation_ = generation_;
    capture->work=std::move(work);
    QString current = owner;
    for (int depth = 0; depth <= 8; ++depth) {
        const auto found = frames_.constFind(current);
        if (found == frames_.cend() || !found->frame.isValid()) break;
        capture->receipt.documents_.prepend(DocumentReceipt::Document{
            current, found->parent, found->epoch, {}, found->frame, found->frame.url(), false});
        if (found->parent.isEmpty()) {
            if (found->frame.isMainFrame()) {
                if(original) {
                    // Positional accounting uses the retained native catalogue,
                    // never a reconstructed chain with different identities.
                    if(!matchesReceipt(*original)||original->documents_.size()!=capture->receipt.documents_.size())break;
                    bool same=true;
                    for(qsizetype index=0;index<original->documents_.size();++index) {
                        const auto &before=original->documents_[index],&now=capture->receipt.documents_[index];
                        if(before.owner!=now.owner||before.parent!=now.parent||before.epoch!=now.epoch
                            ||before.frame!=now.frame||before.url!=now.url){same=false;break;}
                    }
                    if(!same)break;
                }
                if(capture->work&&capture->work->checkpoint==QStringLiteral("bind"))
                    capture->work->checkpoint=capture->receipt.documents_.size()==1?QStringLiteral("bind-top"):QStringLiteral("bind-selected");
                advanceCapture(capture);return;
            }
            break;
        }
        current = found->parent;
    }
    auto failed = std::move(capture->completion);
    failed(std::nullopt);
}

QString FerricFrameBroker::ancestryScript(const DocumentReceipt &receipt, qsizetype index,
                                         const QString &token, bool capture) const {
    const auto &parent=receipt.documents_[index], &child=receipt.documents_[index+1];
    const QJsonObject request{{"mode",capture?"capture":"verify"},{"issuer",ancestryIssuer_},{"token",token},
        {"property",documentProperty_},{"epoch",parent.epoch},{"origin",parent.origin},
        {"child",child.owner},{"child_epoch",child.epoch}};
    return QStringLiteral("(")+ferricHintFrameAncestryProbe()+QStringLiteral(")(JSON.parse(")
        +quoted(QString::fromUtf8(QJsonDocument(request).toJson(QJsonDocument::Compact)))+QStringLiteral("))");
}

void FerricFrameBroker::captureComposedAncestry(const DocumentReceipt &receipt, ReceiptCompletion completion, int timeoutMs,
                                                FerricHintDocumentAdmission admission) {
    if(!completion)return;
    if(timeoutMs<=0||timeoutMs>1000||!matchesReceipt(receipt)){completion(std::nullopt);return;}
    const QDeadlineTimer deadline(timeoutMs,Qt::PreciseTimer);
    if(receipt.documents_.size()==1){
        revalidateDocument(receipt,[receipt,completion,deadline](bool valid){
            completion(valid&&!deadline.hasExpired()?std::optional<DocumentReceipt>(receipt):std::nullopt);
        },static_cast<int>(deadline.remainingTime()),documentWork(admission,QStringLiteral("capture-final")));return;
    }
    revalidateFocusedDocument(receipt,[this,receipt,completion,deadline,admission](bool valid) {
        if(!valid||deadline.hasExpired()||!matchesReceipt(receipt)){completion(std::nullopt);return;}
        auto lease=std::make_shared<AncestryLease>();lease->issuer=ancestryIssuer_;lease->dispatcher=dispatcher_;
        lease->depth=static_cast<quint8>(receipt.documents_.size()-1);
        auto capture=std::make_shared<AncestryCapture>(AncestryCapture{receipt,lease,completion,deadline,0,admission});
        advanceAncestryCapture(capture);
    },static_cast<int>(deadline.remainingTime()),admission);
}

void FerricFrameBroker::advanceAncestryCapture(const std::shared_ptr<AncestryCapture> &capture) {
    const auto fail=[capture]{auto completion=std::move(capture->completion);if(completion)completion(std::nullopt);};
    if(capture->deadline.hasExpired()||!matchesReceipt(capture->receipt)){fail();return;}
    if(capture->index+1==capture->receipt.documents_.size()){
        capture->receipt.ancestry_=capture->lease;
        revalidateDocument(capture->receipt,[capture,fail](bool valid){
            if(!valid||capture->deadline.hasExpired()){fail();return;}
            auto completion=std::move(capture->completion);completion(std::move(capture->receipt));
        },static_cast<int>(capture->deadline.remainingTime()),documentWork(capture->documentAdmission,QStringLiteral("capture-final")));
        return;
    }
    const auto index=capture->index;
    const auto parent=capture->receipt.documents_[index],child=capture->receipt.documents_[index+1];
    scanFocusedChild(parent.owner,[this,capture,index,parent,child,fail](FocusScanResult selected){
        if(!selected.error.isEmpty()||!selected.child||*selected.child!=child.frame
            ||capture->deadline.hasExpired()||!matchesReceipt(capture->receipt)){fail();return;}
        const auto token=QUuid::createUuid().toString(QUuid::WithoutBraces);
        const auto cleanup=dispatcher_->reserveCleanup(parent.frame,parent.epoch,ancestryIssuer_,token);
        if(!cleanup){fail();return;}
        // Own cleanup before queuing the script, including cancellation and
        // timeout after the renderer has already installed its observers.
        capture->lease->entries.append({parent.frame,token,0,cleanup});
        evaluate(parent.frame,ancestryScript(capture->receipt,index,token,true),
            [this,capture,parent,child,fail](const QVariant &value){
            const auto facts=value.toMap();const auto depth=facts.value("depth");
            const auto numeric=depth.metaType().id()==QMetaType::Int||depth.metaType().id()==QMetaType::Double;
            const auto number=depth.toDouble();
            if(value.metaType().id()!=QMetaType::QVariantMap||!numeric||!std::isfinite(number)||number<0
                ||number>8||std::floor(number)!=number||capture->lease->depth+number>8
                ||capture->deadline.hasExpired()||!matchesReceipt(capture->receipt)){fail();return;}
            capture->lease->entries.last().shadows=static_cast<quint8>(number);
            capture->lease->depth+=static_cast<quint8>(number);
            scanFocusedChild(parent.owner,[this,capture,child,fail](FocusScanResult after){
                if(!after.error.isEmpty()||!after.child||*after.child!=child.frame
                    ||capture->deadline.hasExpired()||!matchesReceipt(capture->receipt)){fail();return;}
                ++capture->index;advanceAncestryCapture(capture);
            },static_cast<int>(capture->deadline.remainingTime()));
        },static_cast<int>(capture->deadline.remainingTime()));
    },static_cast<int>(capture->deadline.remainingTime()));
}

void FerricFrameBroker::validateAncestryAt(const DocumentReceipt &receipt,qsizetype index,
    const QDeadlineTimer &deadline,std::function<void(bool)> completion) {
    if(deadline.hasExpired()||!matchesReceipt(receipt)||!receipt.ancestry_
        ||receipt.ancestry_->issuer!=ancestryIssuer_||receipt.ancestry_->entries.size()+1!=receipt.documents_.size()){
        completion(false);return;
    }
    if(index==receipt.ancestry_->entries.size()){completion(true);return;}
    const auto parent=receipt.documents_[index],child=receipt.documents_[index+1];
    const auto entry=receipt.ancestry_->entries[index];
    if(entry.parent!=parent.frame){completion(false);return;}
    scanFocusedChild(parent.owner,[this,receipt,index,parent,child,entry,deadline,completion](FocusScanResult selected){
        if(!selected.error.isEmpty()||!selected.child||*selected.child!=child.frame||deadline.hasExpired()
            ||!matchesReceipt(receipt)){completion(false);return;}
        evaluate(parent.frame,ancestryScript(receipt,index,entry.token,false),
            [this,receipt,index,parent,child,entry,deadline,completion](const QVariant &value){
            const auto facts=value.toMap();const auto depth=facts.value("depth");
            if(value.metaType().id()!=QMetaType::QVariantMap
                ||(depth.metaType().id()!=QMetaType::Int&&depth.metaType().id()!=QMetaType::Double)
                ||depth.toDouble()!=entry.shadows||deadline.hasExpired()||!matchesReceipt(receipt)){
                completion(false);return;
            }
            scanFocusedChild(parent.owner,[this,receipt,index,parent,child,entry,deadline,completion](FocusScanResult after){
                if(!after.error.isEmpty()||!after.child||*after.child!=child.frame||deadline.hasExpired()
                    ||!matchesReceipt(receipt)){completion(false);return;}
                evaluate(parent.frame,ancestryScript(receipt,index,entry.token,false),
                    [this,receipt,index,entry,deadline,completion](const QVariant &sealed){
                    const auto depth=sealed.toMap().value("depth");
                    if(sealed.metaType().id()!=QMetaType::QVariantMap
                        ||(depth.metaType().id()!=QMetaType::Int&&depth.metaType().id()!=QMetaType::Double)
                        ||depth.toDouble()!=entry.shadows||deadline.hasExpired()||!matchesReceipt(receipt)){
                        completion(false);return;
                    }
                    validateAncestryAt(receipt,index+1,deadline,completion);
                },static_cast<int>(deadline.remainingTime()));
            },static_cast<int>(deadline.remainingTime()));
        },static_cast<int>(deadline.remainingTime()));
    },static_cast<int>(deadline.remainingTime()));
}

void FerricFrameBroker::advanceCapture(const std::shared_ptr<Capture> &capture) {
    if (capture->deadline.hasExpired() || !matchesReceipt(capture->receipt)) {
        auto completion = std::move(capture->completion);
        completion(std::nullopt); return;
    }
    if (!capture->verifying && capture->index == capture->receipt.documents_.size()) {
        capture->verifying = true;
    }
    if (capture->verifying && capture->index == 0) {
        auto completion = std::move(capture->completion);
        if (capture->deadline.hasExpired()) { completion(std::nullopt); return; }
        completion(std::move(capture->receipt)); return;
    }
    const auto index = capture->verifying ? capture->index - 1 : capture->index;
    const auto &document = capture->receipt.documents_[index];
    const auto epoch = QStringLiteral("window.") + documentProperty_ + QStringLiteral("===") + quoted(document.epoch);
    const auto script = capture->verifying
        ? QStringLiteral("(") + epoch + QStringLiteral("&&String(window.origin)===") + quoted(document.origin)
            + QStringLiteral("&&document.hasFocus()===") + (document.focused ? QStringLiteral("true") : QStringLiteral("false")) + QStringLiteral(")")
        : QStringLiteral("(()=>{if(!(") + epoch + QStringLiteral("))return null;return [String(window.origin),document.hasFocus()];})()");
    evaluateDocument(document.frame, script, [this, capture, index](const QVariant &value) {
        bool valid = false;
        if (capture->verifying) {
            valid = value.metaType().id() == QMetaType::Bool && value.toBool();
            if (valid) --capture->index;
        } else if (value.metaType().id() == QMetaType::QVariantList) {
            const auto facts = value.toList();
            if (facts.size() == 2 && facts[0].metaType().id() == QMetaType::QString
                && facts[1].metaType().id() == QMetaType::Bool) {
                const auto origin = facts[0].toString();
                // Fact budget, not a new authored-origin grammar. Never derive
                // effective origin from URL text (srcdoc/sandbox differ).
                valid = !origin.isEmpty() && origin.size() <= 128 * 1024
                    && origin.toUtf8().size() <= 128 * 1024;
                if (valid) {
                    capture->receipt.documents_[index].origin = origin;
                    capture->receipt.documents_[index].focused = facts[1].toBool();
                    ++capture->index;
                }
            }
        }
        if (!valid) {
            auto completion = std::move(capture->completion);
            completion(std::nullopt); return;
        }
        advanceCapture(capture);
    }, static_cast<int>(capture->deadline.remainingTime()),capture->work,static_cast<quint8>(index),capture->verifying?1:0);
}

void FerricFrameBroker::revalidateDocument(const DocumentReceipt &receipt, std::function<void(bool)> completion,
                                           int timeoutMs,std::optional<DocumentWork> work) {
    if (!completion) return;
    if (!matchesReceipt(receipt)) { completion(false); return; }
    const auto owner = receipt.documents_.last().owner;
    const QDeadlineTimer deadline(timeoutMs,Qt::PreciseTimer);
    captureDocumentImpl(owner, [this,original = receipt, completion = std::move(completion),deadline](std::optional<DocumentReceipt> current) {
        if (!current || original.documents_.size() != current->documents_.size()) {
            completion(false); return;
        }
        for (qsizetype index = 0; index < original.documents_.size(); ++index) {
            const auto &old = original.documents_[index], &now = current->documents_[index];
            if (old.owner != now.owner || old.parent != now.parent || old.epoch != now.epoch
                || old.frame != now.frame || old.url != now.url || old.origin != now.origin || old.focused != now.focused) {
                completion(false); return;
            }
        }
        if(original.ancestry_){
            const auto children=original.documents_.last().frame.children();
            validateAncestryAt(original,0,deadline,[this,original,children,deadline,completion](bool valid){
                if(!valid||deadline.hasExpired()||!matchesReceipt(original)
                    ||original.documents_.last().frame.children()!=children){completion(false);return;}
                const auto &leaf=original.documents_.last();
                // Parent ancestry and hasFocus() also hold when this former
                // leaf becomes an ancestor. Seal its active path last.
                const auto seal=QStringLiteral("(()=>{if(window.")+documentProperty_
                    +QStringLiteral("!==")+quoted(leaf.epoch)
                    +QStringLiteral("||String(window.origin)!==")+quoted(leaf.origin)
                    +QStringLiteral(")return false;const check=")+focusedLeafPredicate()
                    +QStringLiteral(";return check(document,")
                    +(children.isEmpty()?QStringLiteral("false"):QStringLiteral("true"))
                    +QStringLiteral(")===true;})()");
                evaluate(leaf.frame,seal,[this,original,children,deadline,completion](const QVariant &sealed){
                    completion(sealed.metaType().id()==QMetaType::Bool&&sealed.toBool()
                        &&!deadline.hasExpired()&&matchesReceipt(original)
                        &&original.documents_.last().frame.children()==children);
                },static_cast<int>(deadline.remainingTime()));
            });
            return;
        }
        completion(!deadline.hasExpired());
    }, static_cast<int>(deadline.remainingTime()),std::move(work),receipt);
}

std::optional<FerricFrameBroker::DocumentReceipt> FerricFrameBroker::topDocumentReceipt(const DocumentReceipt &receipt) const {
    if (!matchesReceipt(receipt) || !receipt.documents_.first().frame.isMainFrame()) return std::nullopt;
    auto top = receipt;
    top.documents_ = {receipt.documents_.first()};
    top.ancestry_.reset();
    return top;
}

void FerricFrameBroker::revalidateFocusedDocument(const DocumentReceipt &receipt,
                                                  std::function<void(bool)> completion, int timeoutMs,
                                                  FerricHintDocumentAdmission admission) {
    if (!completion) return;
    if (timeoutMs <= 0 || timeoutMs > 1000 || !matchesReceipt(receipt)
        || receipt.documents_.size() < 2
        || std::any_of(receipt.documents_.cbegin(), receipt.documents_.cend(),
                       [](const auto &document) { return !document.focused; })) {
        completion(false); return;
    }
    const auto &leaf = receipt.documents_.last();
    const QUrl origin(leaf.origin);
    if (!origin.isValid() || (origin.scheme() != QStringLiteral("http") && origin.scheme() != QStringLiteral("https"))
        || origin.host().isEmpty() || !origin.userInfo().isEmpty() || !origin.path().isEmpty()
        || origin.hasQuery() || origin.hasFragment()) { completion(false); return; }
    const QDeadlineTimer deadline(timeoutMs, Qt::PreciseTimer);
    const auto children = leaf.frame.children();
    const auto current = [this, receipt, children, deadline] {
        return !deadline.hasExpired() && matchesReceipt(receipt)
            && receipt.documents_.last().frame.children() == children;
    };
    // Recheck the full receipt after the terminal scan too. hasFocus() alone
    // is true for ancestors, and the scan itself crosses async boundaries.
    revalidateDocument(receipt, [this, receipt, completion, deadline, current, hasChildren = !children.isEmpty(),admission](bool valid) {
        if (!valid || !current()) { completion(false); return; }
        verifyFocusedLeafNative(receipt.documents_.last().owner,
            [this, receipt, completion, deadline, current, hasChildren,admission](const QVariant &terminal) {
                if (terminal.metaType().id() != QMetaType::Bool || !terminal.toBool() || !current()) {
                    completion(false); return;
                }
                revalidateDocument(receipt, [this, receipt, completion, current, deadline, hasChildren](bool valid) {
                    if (!valid || !current()) { completion(false); return; }
                    const auto &leaf = receipt.documents_.last();
                    // The final receipt pass is asynchronous. Focus may move
                    // into an existing child while ancestor hasFocus() stays
                    // true, so seal the exposed terminal path after that pass.
                    const auto seal = QStringLiteral("(()=>{if(window.") + documentProperty_
                        + QStringLiteral("!==") + quoted(leaf.epoch)
                        + QStringLiteral("||String(window.origin)!==") + quoted(leaf.origin)
                        + QStringLiteral(")return false;const check=") + focusedLeafPredicate()
                        + QStringLiteral(";return check(document,")
                        + (hasChildren ? QStringLiteral("true") : QStringLiteral("false"))
                        + QStringLiteral(")===true;})()");
                    evaluate(leaf.frame, seal, [completion, current](const QVariant &sealed) {
                        completion(sealed.metaType().id() == QMetaType::Bool && sealed.toBool() && current());
                    }, static_cast<int>(deadline.remainingTime()));
                }, static_cast<int>(deadline.remainingTime()),documentWork(admission,QStringLiteral("focused-before-second")));
            }, static_cast<int>(deadline.remainingTime()));
    }, static_cast<int>(deadline.remainingTime()),documentWork(admission,QStringLiteral("focused-before-first")));
}

void FerricFrameBroker::validateWorldDocuments(const DocumentReceipt &receipt, const QString &batch,
                                                const QString &stamp, quint64 revision,
                                                const QString &documentsJson,
                                                std::function<void(bool)> completion, int timeoutMs) {
    validateLocalDocuments(receipt, std::nullopt, batch, stamp, revision, documentsJson, std::nullopt,
        QJsonValue(QJsonValue::Null), std::nullopt, std::nullopt, QJsonValue(QJsonValue::Null), std::move(completion), timeoutMs);
}

void FerricFrameBroker::validateFocusedWorldDocuments(const DocumentReceipt &world, FocusedDocuments focused,
                                                const QString &batch, const QString &stamp, quint64 revision,
                                                const QString &documentsJson,
                                                std::function<void(bool)> completion, int timeoutMs) {
    validateLocalDocuments(world, std::move(focused), batch, stamp, revision, documentsJson, std::nullopt,
        QJsonValue(QJsonValue::Null), std::nullopt, std::nullopt, QJsonValue(QJsonValue::Null), std::move(completion), timeoutMs);
}

void FerricFrameBroker::validateWorldObjects(const DocumentReceipt &world, std::optional<FocusedDocuments> focused,
                                            const QString &batch, const QString &stamp, quint64 revision,
                                            const QString &annotationJson, const QString &scopeJson,
                                            std::function<void(bool)> completion, int timeoutMs,
                                            std::optional<EnteredDocument> entered,
                                            std::optional<QString> originalPlan,
                                            std::optional<AnnotationDebit> annotationDebit) {
    validateWorldObjectsImpl(world, std::move(focused), batch, stamp, revision, annotationJson, scopeJson,
        std::move(completion), timeoutMs, std::move(entered), std::move(originalPlan), annotationDebit, std::nullopt, {}, QString());
}

void FerricFrameBroker::validateWorldObjectsImpl(const DocumentReceipt &world, std::optional<FocusedDocuments> focused,
                                            const QString &batch, const QString &stamp, quint64 revision,
                                            const QString &annotationJson, const QString &scopeJson,
                                            std::function<void(bool)> completion, int timeoutMs,
                                            std::optional<EnteredDocument> entered,
                                            std::optional<QString> originalPlan,
                                            std::optional<AnnotationDebit> annotationDebit,
                                            std::optional<QSizeF> viewport,
                                            std::function<void(const QString &)> fragmentCapture,
                                            const QString &ordinaryGeometry, const QString &ordinaryInput,
                                            FerricHintGraphAdmission graphAdmission, quint8 graphBase,
                                            FerricHintDocumentAdmission documentAdmission) {
    if (!completion) return;
    if (timeoutMs <= 0 || timeoutMs > 1000 || annotationJson.isEmpty() || annotationJson.size() > 2 * 1024 * 1024
        || scopeJson.isEmpty() || scopeJson.size() > 2048
        || (originalPlan && originalPlan->isEmpty())) { completion(false); return; }
    const QDeadlineTimer deadline(timeoutMs, Qt::PreciseTimer);
    const auto encoded = annotationJson.toUtf8(), encodedScope = scopeJson.toUtf8();
    if (encoded.size() > 2 * 1024 * 1024 || encodedScope.size() > 2048) { completion(false); return; }
    QJsonValue annotationWork(QJsonValue::Null);
    QJsonValue geometry(QJsonValue::Null);
    if(fragmentCapture){
        if(!annotationDebit||!viewport||ordinaryGeometry.isEmpty()||ordinaryGeometry.size()>2*1024*1024){completion(false);return;}
        const auto bytes=ordinaryGeometry.toUtf8();
        if(bytes.size()>2*1024*1024){completion(false);return;}
        const auto parsed=QJsonDocument::fromJson(bytes);
        if(!parsed.isArray()||parsed.array().size()>5000){completion(false);return;}
        geometry=parsed.array();
    }
    if(annotationDebit) {
        if(!originalPlan||annotationDebit->granted>2*1024*1024
            ||annotationDebit->reported>annotationDebit->granted
            ||annotationDebit->reported<static_cast<quint32>(encoded.size())) {completion(false);return;}
        annotationWork=QJsonObject{{"bytes",static_cast<int>(annotationDebit->reported)},
            {"limit",static_cast<int>(annotationDebit->granted)},{"raw_bytes",static_cast<int>(encoded.size())}};
    }
    const auto facts = QJsonDocument::fromJson(encoded);
    if (!facts.isObject() || !facts.object().value("documents").isArray()
        || facts.object().value("documents").toArray().size() > 50000) { completion(false); return; }
    QJsonValue scope(QJsonValue::Null);
    if (scopeJson.trimmed() != QStringLiteral("null")) {
        const auto parsed = QJsonDocument::fromJson(encodedScope);
        if (!parsed.isObject()) { completion(false); return; }
        scope = parsed.object();
    }
    const auto documents = QString::fromUtf8(QJsonDocument(facts.object().value("documents").toArray()).toJson(QJsonDocument::Compact));
    validateLocalDocuments(world, std::move(focused), batch, stamp, revision, documents, annotationJson, scope,
        std::move(entered), std::move(originalPlan), annotationWork, std::move(completion), static_cast<int>(deadline.remainingTime()),
        viewport, std::move(fragmentCapture), geometry, ordinaryInput,std::move(graphAdmission),graphBase,
        std::move(documentAdmission));
}

void FerricFrameBroker::validateLocalDocuments(const DocumentReceipt &receipt, std::optional<FocusedDocuments> focused,
                                                const QString &batch,
                                                const QString &stamp, quint64 revision,
                                                const QString &documentsJson,
                                                const std::optional<QString> &annotationJson,
                                                const QJsonValue &scope,
                                                std::optional<EnteredDocument> entered,
                                                std::optional<QString> originalPlan,
                                                const QJsonValue &annotationWork,
                                                std::function<void(bool)> completion, int timeoutMs,
                                                std::optional<QSizeF> viewport,
                                                std::function<void(const QString &)> fragmentCapture,
                                                QJsonValue ordinaryGeometry, const QString &ordinaryInput,
                                                FerricHintGraphAdmission graphAdmission, quint8 graphBase,
                                                FerricHintDocumentAdmission documentAdmission) {
    if (!completion) return;
    if (timeoutMs <= 0 || timeoutMs > 1000) { completion(false); return; }
    const QDeadlineTimer deadline(timeoutMs, Qt::PreciseTimer);
    constexpr qsizetype annotationBytes = 2 * 1024 * 1024;
    const auto batchId = QUuid::fromString(batch);
    if (!matchesReceipt(receipt) || batchId.isNull()
        || batchId.toString(QUuid::WithoutBraces) != batch || stamp.size() > 4096
        || revision > 9007199254740991ULL || documentsJson.size() > annotationBytes) {
        completion(false); return;
    }
    const auto encodedDocuments = documentsJson.toUtf8();
    if (encodedDocuments.size() > annotationBytes) { completion(false); return; }
    const auto parsed = QJsonDocument::fromJson(encodedDocuments);
    if (!parsed.isArray() || parsed.array().size() > 50000) { completion(false); return; }
    const auto &world = receipt.documents_.last();
    const auto documentPrefix=(world.frame.isMainFrame()?QStringLiteral("top-"):QStringLiteral("selected-"))
        +(graphBase==0?QStringLiteral("capture-"):QStringLiteral("reseal-"));
    QJsonValue enteredFacts(QJsonValue::Null);
    if(entered) {
        if(!annotationJson||!world.frame.isMainFrame()||receipt.documents_.size()!=1
            ||!matchesReceipt(entered->selected)||entered->selected.documents_.size()<2) {
            completion(false);return;
        }
        const auto &documents=entered->selected.documents_;
        const auto &root=documents.first();
        if(root.frame!=world.frame||root.owner!=world.owner||root.epoch!=world.epoch||root.origin!=world.origin||root.url!=world.url
            ||root.origin.isEmpty()||root.origin==QStringLiteral("null")) {
            completion(false);return;
        }
        QJsonArray chain;
        for(const auto &document:documents) {
            if(!document.focused||document.origin!=root.origin){completion(false);return;}
            chain.append(QJsonObject{{"epoch",document.epoch},{"origin",document.origin}});
        }
        enteredFacts=QJsonObject{{"depth",static_cast<int>(documents.size()-1)},{"chain",chain}};
    }
    QList<QWebEngineFrame> focusedChildren;
    QJsonValue selected(QJsonValue::Null);
    if (focused) {
        if (!matchesReceipt(focused->selected) || focused->requiredDocuments.isEmpty()
            || focused->requiredDocuments.size()>80) { completion(false); return; }
        const auto &leaf=focused->selected.documents_.last();
        const auto &root=focused->selected.documents_.first();
        const auto &worldRoot=receipt.documents_.first();
        const QUrl origin(leaf.origin);
        if(leaf.frame.isMainFrame()||!leaf.focused||root.frame!=worldRoot.frame||root.epoch!=worldRoot.epoch
            ||(!world.frame.isMainFrame()&&world.frame!=leaf.frame)||!origin.isValid()
            ||(origin.scheme()!=QStringLiteral("http")&&origin.scheme()!=QStringLiteral("https"))
            ||origin.host().isEmpty()||!origin.userInfo().isEmpty()||!origin.path().isEmpty()
            ||origin.hasQuery()||origin.hasFragment()) { completion(false); return; }
        QSet<quint32> unique;
        QJsonArray required;
        for(const auto id:focused->requiredDocuments){
            if(!id||unique.contains(id)){completion(false);return;}
            unique.insert(id);required.append(static_cast<double>(id));
        }
        focusedChildren=leaf.frame.children();
        selected=QJsonObject{{"epoch",leaf.epoch},{"origin",leaf.origin},{"documents",required},
            {"depth",static_cast<int>(focused->selected.documents_.size()-1)},
            {"hasChildren",!focusedChildren.isEmpty()}};
    }
    const auto focusedCurrent=[this,focused,focusedChildren,entered] {
        return (!focused||(matchesReceipt(focused->selected)
            &&focused->selected.documents_.last().frame.children()==focusedChildren))
            &&(!entered||matchesReceipt(entered->selected));
    };
    const QJsonObject expected{{"batch", batch}, {"stamp", stamp}, {"revision", static_cast<double>(revision)},
        {"owner", world.frame.isMainFrame() ? QString() : world.owner},
        {"baseDepth", static_cast<int>(receipt.composedDepth().value_or(static_cast<quint8>(receipt.documents_.size()-1)))},
        {"nativeBaseDepth",static_cast<int>(receipt.documents_.size()-1)},
        {"nativeEpoch", world.epoch}, {"nativeOrigin", world.origin},
        {"documents", annotationJson ? QJsonValue(QJsonValue::Null) : QJsonValue(parsed.array())},
        {"annotation", annotationJson ? QJsonValue(*annotationJson) : QJsonValue(QJsonValue::Null)},
        {"plan", originalPlan ? QJsonValue(*originalPlan) : QJsonValue(QJsonValue::Null)},
        {"annotation_work",annotationWork},
        {"capture_fragments",static_cast<bool>(fragmentCapture)},
        {"viewport",viewport ? QJsonValue(QJsonObject{{"width",viewport->width()},{"height",viewport->height()}}) : QJsonValue(QJsonValue::Null)},
        {"ordinary_geometry",ordinaryGeometry},
        {"ordinary_input",ordinaryInput.isEmpty()?QJsonValue(QJsonValue::Null):QJsonValue(ordinaryInput)},
        {"scope", scope}, {"nativeProperty",documentProperty_},{"selected",selected},{"entered",enteredFacts}};
    // Encode as a JSON string before parsing in the isolated world. Object
    // literals must not give an untrusted __proto__ field special semantics.
    const auto script = QStringLiteral("(()=>{try{const graphCheck=") + ferricHintProofObjectProbe()
        + QStringLiteral(";const leafFocus=") + focusedLeafPredicate()
        + QStringLiteral(";const expected=JSON.parse(")
        + quoted(QString::fromUtf8(QJsonDocument(expected).toJson(QJsonDocument::Compact)))
        + QStringLiteral(");if(expected.annotation!==null){expected.facts=JSON.parse(expected.annotation);expected.documents=expected.facts.documents;}if(!Array.isArray(expected.documents)||expected.documents.length>50000)return false;if(window.") + documentProperty_
        + QStringLiteral(R"JS(!==expected.nativeEpoch||String(window.origin)!==expected.nativeOrigin)return false;
        const state=window.__ferric_browserHintState,batch=state&&state.interactionProof;
        if(!state||state.dirty||!batch||batch.batch!==expected.batch||batch.stamp!==expected.stamp
            ||batch.revision!==expected.revision||state.revision!==expected.revision
            ||!(batch.documents instanceof Map)||batch.documents.size>50000)return false;
        if(expected.viewport!==null&&(innerWidth!==expected.viewport.width||innerHeight!==expected.viewport.height))return false;
        // Validate and return the same retained strings, even if the renderer
        // replaced a private field with a getter between native continuations.
        const captured={annotation:batch.annotation,catalogue:batch.fragment_catalogue};
        function result(valid){return valid?(expected.capture_fragments?captured.catalogue:true):false;}
        function keys(value,names){return !!value&&typeof value==='object'&&!Array.isArray(value)
            &&Object.keys(value).length===names.length&&names.every(function(name){return Object.prototype.hasOwnProperty.call(value,name);});}
        function id(value){return Number.isInteger(value)&&value>0&&value<=4294967295;}
        function deref(value){return value?WeakRef.prototype.deref.call(value):null;}
        const documents=new Map();
        let root=null;
        for(const fact of expected.documents){
            if(!keys(fact,['id','frame_owner','frame_path','epoch','origin','document_node','parent_host'])
                ||!id(fact.id)||fact.document_node!==fact.id||documents.has(fact.id)
                ||fact.frame_owner!==expected.owner||typeof fact.frame_path!=='string'
                ||fact.frame_path.length>256||!/^0(?:\.[0-9]+){0,8}$/.test(fact.frame_path))return false;
            const entry=batch.documents.get(fact.id),doc=entry&&deref(entry.document);
            if(!doc||doc.nodeType!==9||!doc.defaultView||doc.defaultView.document!==doc
                ||state.weak.get(doc)!==fact.id||!state.proofEpochs||state.proofEpochs.get(doc)!==fact.epoch
                ||String(doc.defaultView.origin)!==fact.origin||entry.path!==fact.frame_path)return false;
            if(doc===document){
                if(root!==null||fact.parent_host!==null||fact.frame_path!=='0'||entry.host||entry.parent
                    ||fact.origin!==expected.nativeOrigin)return false;
                root=fact.id;
            }else if(!keys(fact.parent_host,['document','node'])
                ||!id(fact.parent_host.document)||!id(fact.parent_host.node))return false;
            documents.set(fact.id,{fact:fact,entry:entry,doc:doc});
        }
        if(documents.size===0)return result(expected.selected===null&&(expected.annotation===null||graphCheck(state,batch,expected,documents,captured,graphWork)));
        if(root===null)return false;
        const depths=new Map([[root,0]]);
        for(const value of documents.values()){
            if(value.fact.id===root)continue;
            const parent=documents.get(value.fact.parent_host.document),host=deref(value.entry.host);
            if(!parent||!host||!host.isConnected||host.ownerDocument!==parent.doc
                ||deref(value.entry.parent)!==parent.doc||state.weak.get(host)!==value.fact.parent_host.node
                ||host.contentDocument!==value.doc||host.contentWindow!==value.doc.defaultView
                ||value.doc.defaultView.frameElement!==host)return false;
            let current=value,depth=0;
            while(current.fact.id!==root){
                if(depth+expected.baseDepth>=8||!current.fact.parent_host)return false;
                current=documents.get(current.fact.parent_host.document);
                if(!current)return false;
                depth++;
            }
            depths.set(value.fact.id,depth);
        }
        if(expected.selected!==null){
            for(const id of expected.selected.documents){
                const value=documents.get(id);
                if(!value||depths.get(id)+expected.nativeBaseDepth!==expected.selected.depth
                    ||value.doc.defaultView[expected.nativeProperty]!==expected.selected.epoch
                    ||String(value.doc.defaultView.origin)!==expected.selected.origin
                    ||leafFocus(value.doc,expected.selected.hasChildren)!==true)return false;
            }
        }
        return result(expected.annotation===null||graphCheck(state,batch,expected,documents,captured,graphWork));
    }catch(error){return false;}})())JS");
    const auto firstCatalogue=std::make_shared<QString>();
    const auto accepted=[fragmentCapture](const QVariant &value) {
        return fragmentCapture ? value.metaType().id()==QMetaType::QString&&!value.toString().isEmpty()
            : value.metaType().id()==QMetaType::Bool&&value.toBool();
    };
    const auto seal=[this,receipt,focusedCurrent,script,deadline,completion,entered,fragmentCapture,firstCatalogue,accepted,graphAdmission,graphBase] {
        const auto check=[this,receipt,focusedCurrent,script,deadline,completion,fragmentCapture,firstCatalogue,accepted,graphAdmission,graphBase] {
            evaluateGraph(receipt.documents_.last().frame,script,
            [this,receipt,focusedCurrent,deadline,completion,fragmentCapture,firstCatalogue,accepted](const QVariant &sealed) {
                const bool valid=accepted(sealed)&&matchesReceipt(receipt)&&focusedCurrent()&&!deadline.hasExpired()
                    &&(!fragmentCapture||sealed.toString()==*firstCatalogue);
                if(valid&&fragmentCapture)fragmentCapture(*firstCatalogue);
                completion(valid);
            },static_cast<int>(deadline.remainingTime()),graphAdmission,graphBase+1);
        };
        if(!entered){check();return;}
        // The explicit entered Document can contain a focused descendant.
        // Revalidate the original chain, without imposing terminal-leaf rules
        // belonging only to focused-root receiver obligations.
        revalidateDocument(entered->selected,[focusedCurrent,deadline,completion,check](bool valid) {
            if(!valid||!focusedCurrent()||deadline.hasExpired()){completion(false);return;}
            check();
        },static_cast<int>(deadline.remainingTime()));
    };
    revalidateDocument(receipt, [this, receipt, focused, focusedCurrent, script, deadline, completion, seal,fragmentCapture,firstCatalogue,accepted,graphAdmission,graphBase,documentAdmission,documentPrefix](bool valid) {
        if (!valid || deadline.hasExpired() || !matchesReceipt(receipt) || !focusedCurrent()) { completion(false); return; }
        evaluateGraph(receipt.documents_.last().frame, script,
                 [this, receipt, focused, focusedCurrent, deadline, completion, seal,fragmentCapture,firstCatalogue,accepted,documentAdmission,documentPrefix](const QVariant &value) {
            if (deadline.hasExpired() || !accepted(value)
                || !matchesReceipt(receipt) || !focusedCurrent()) { completion(false); return; }
            if(fragmentCapture)*firstCatalogue=value.toString();
            revalidateDocument(receipt, [this, receipt, focused, focusedCurrent, deadline, completion, seal,documentAdmission,documentPrefix](bool current) {
                if (!current || deadline.hasExpired() || !matchesReceipt(receipt) || !focusedCurrent()) { completion(false); return; }
                if (!focused) { seal(); return; }
                revalidateDocument(focused->selected,[this,focused,focusedCurrent,deadline,completion,seal](bool selectedCurrent) {
                    if(!selectedCurrent||deadline.hasExpired()||!focusedCurrent()){completion(false);return;}
                    verifyFocusedLeafNative(focused->selected.documents_.last().owner,
                        [focusedCurrent,deadline,completion,seal](const QVariant &leaf) {
                            if(leaf.metaType().id()!=QMetaType::Bool||!leaf.toBool()
                                ||deadline.hasExpired()||!focusedCurrent()){completion(false);return;}
                            seal();
                        },static_cast<int>(deadline.remainingTime()));
                },static_cast<int>(deadline.remainingTime()),documentWork(documentAdmission,documentPrefix+QStringLiteral("focused")));
            }, static_cast<int>(deadline.remainingTime()),documentWork(documentAdmission,documentPrefix+QStringLiteral("after")));
        }, static_cast<int>(deadline.remainingTime()),graphAdmission,graphBase);
    }, static_cast<int>(deadline.remainingTime()),documentWork(documentAdmission,documentPrefix+QStringLiteral("before")));
}
