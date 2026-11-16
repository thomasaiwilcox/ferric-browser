#include "site_policy_observation.h"
#include "profile_lifetime.h"
#include "ferric-browser-engine-qt/src/bridge.cxxqt.h"
#include <QMetaProperty>
#include <QTimer>
#include <QThread>
#include <QUrl>
#include <functional>

FerricProfileLifetime *FerricSitePolicyObserver::lifetime() const { return lifetime_; }
void FerricSitePolicyObserver::setLifetime(FerricProfileLifetime *value) {
    if (lifetime_ == value || (value && value->thread() != thread())) return;
    disconnect(association_);
    lifetime_ = value;
    if (value) association_ = connect(value, &FerricProfileLifetime::sitePolicyAssociationChanged,
                                     this, &FerricSitePolicyObserver::schedule);
    emit lifetimeChanged();
}

void FerricSitePolicyObserver::schedule(QQuickItem *item) {
    const QPointer<FerricSitePolicyObserver> self(this);
    const QPointer<FerricProfileLifetime> lifetime(lifetime_);
    const QPointer<QQuickItem> view(item);
    const auto expected = lifetime && view ? lifetime->currentView(view) : std::nullopt;
    if (!self || !lifetime || !view || !expected) return;
    // Core publication and all QML routing properties finish in this turn.
    // No property read is inserted into native association -> Core commit.
    QTimer::singleShot(0, this, [self, lifetime, view, expected = *expected] {
        auto current = [&] {
            return self && lifetime && view && self->lifetime_ == lifetime
                && lifetime->matchesCurrentView(view, expected);
        };
        if (!current() || !view->inherits("QQuickWebEngineView")) return;
        auto *ui = qobject_cast<BrowserUi *>(expected.ui.data());
        const QPointer<BrowserUi> guardedUi(ui);
        if (!ui) return;
        // The pinned Qt URL property is FINAL and supplied by the real native
        // WebEngine view. QML model addresses cannot supply this observation.
        const auto urlProperty = view->metaObject()->property(view->metaObject()->indexOfProperty("url"));
        if (!urlProperty.isValid() || !urlProperty.isFinal()) return;
        const QString tab = view->property("stableTabId").toString();
        if (!current() || !guardedUi || tab.isEmpty()) return;
        auto capture = ferric::site_policy::begin_association(*ui, tab);
        if (!current() || !guardedUi) return;
        const QString confirmedTab = view->property("stableTabId").toString();
        if (!current() || !guardedUi || confirmedTab != tab) return;
        const QUrl url = urlProperty.read(view).toUrl();
        if (!current() || !guardedUi) return;
        ferric::site_policy::complete_association(*ui, *capture, url.toString(QUrl::FullyEncoded));
    });
}

void ferric_observe_public_site_policy(QObject *object, const QString &tab,
                                      const std::function<bool()> &current,
                                      const std::function<QUrl()> &actualUrl) {
    const QPointer<BrowserUi> ui(qobject_cast<BrowserUi *>(object));
    if (!ui || ui->thread() != QThread::currentThread() || !current || !actualUrl || !current()) return;
    auto capture = ferric::site_policy::begin_association(*ui, tab);
    if (!ui || !current()) return;
    const auto url = actualUrl();
    if (!ui || !current()) return;
    ferric::site_policy::complete_association(*ui, *capture, url.toString(QUrl::FullyEncoded));
}
