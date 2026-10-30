#include "public_page_host.h"
#include "public_page_owner.h"

#include <QRegularExpression>
#include <QDialog>
#include <QDialogButtonBox>
#include <QLabel>
#include <QPushButton>
#include <QVBoxLayout>
#include <QWebEngineFrame>
#include <QWebEngineFindTextResult>
#include <QWebEngineHistory>
#include <QWebEnginePage>
#include <QWebEngineView>
#include <QWebEngineScript>
#include <cmath>

bool FerricPublicPageHost::confirmFormReload(const QVariantMap &row)
{
    if (m_formReloadDialog) {
        m_formReloadDialog->raise();
        return true;
    }
    if (!isVisible() || !isActiveWindow()) {
        m_browserUi->setProperty("status_text", "Select this page to confirm resending its form");
        return false;
    }
    // Rust records reload intent before Qt is asked to reload. While asking
    // about a POST, retain the actual settled page state and connection.
    if (!m_pageOwner->page()->isLoading()) {
        const int index = currentTabIndex();
        const char *method = m_engineLoadState == "complete" ? "navigation_completed_for"
            : m_engineLoadState == "failed" ? "navigation_failed_for" : "navigation_stopped_for";
        QMetaObject::invokeMethod(m_browserUi, method, Qt::DirectConnection, Q_ARG(int, index));
    }
    const auto operation = m_pageOwner->captureOperation();
    const auto url = m_pageOwner->page()->url();
    auto *dialog = new QDialog(m_pageView);
    dialog->setObjectName("nativeFormReloadConfirmation");
    dialog->setWindowTitle(tr("Resend form?"));
    dialog->setWindowModality(Qt::WindowModal);
    dialog->setAttribute(Qt::WA_DeleteOnClose);
    auto *layout = new QVBoxLayout(dialog);
    auto *label = new QLabel(tr("Reloading this page will send its form again. This could repeat a purchase or another action."), dialog);
    label->setWordWrap(true);
    label->setTextFormat(Qt::PlainText);
    layout->addWidget(label);
    auto *buttons = new QDialogButtonBox(QDialogButtonBox::Cancel, dialog);
    auto *resend = buttons->addButton(tr("Resend form"), QDialogButtonBox::AcceptRole);
    resend->setObjectName("nativeFormReloadResend");
    resend->setAutoDefault(false);
    buttons->button(QDialogButtonBox::Cancel)->setDefault(true);
    layout->addWidget(buttons);
    connect(buttons, &QDialogButtonBox::accepted, dialog, &QDialog::accept);
    connect(buttons, &QDialogButtonBox::rejected, dialog, &QDialog::reject);
    m_formReloadDialog = dialog;
    connect(dialog, &QDialog::finished, this, [this, row, operation, url](int result) {
        m_formReloadDialog = nullptr;
        if (result != QDialog::Accepted || !m_pageOwner || !m_pageOwner->isCurrent(operation)
            || !requestIsCurrent(row) || m_pageOwner->page()->url() != url) return;
        m_navigationRequest = row;
        m_navigationRequest.insert("url", url.toString());
        m_pageOwner->page()->triggerAction(row.value("bypassCache").toBool()
            ? QWebEnginePage::ReloadAndBypassCache : QWebEnginePage::Reload);
    });
    dialog->open();
    return true;
}

// The Rust queue and live native registry establish the target before entering
// here. Keep all asynchronous feedback bound to that same target and native
// document/presentation; callbacks must not update a replacement page's UI.
bool FerricPublicPageHost::runCapturedScript(
    const QVariantMap &request, const QString &script, quint32 world)
{
    if (script.isEmpty() || script.size() > 1024 * 1024
        || (world != QWebEngineScript::MainWorld && world != QWebEngineScript::ApplicationWorld)) return false;
    const auto operation = m_pageOwner->captureOperation();
    QPointer<FerricPublicPageHost> guard(this);
    QPointer<FerricPublicPageOwner> owner(m_pageOwner);
    m_pageOwner->page()->mainFrame().runJavaScript(script, world,
        [guard, owner, operation, request](const QVariant &value) {
            if (!guard || !owner || !owner->isCurrent(operation) || !guard->requestIsCurrent(request)) return;
            emit guard->capturedCommandFinished(request, value);
            if (request.value("kind").toString() == "evaluate") {
                guard->m_browserUi->setProperty("status_text", "JavaScript evaluation completed");
            }
        });
    return true;
}

bool FerricPublicPageHost::executePageCommand(const QVariantMap &row)
{
    auto *page = m_pageOwner->page();
    const QString kind = row.value("kind").toString();
    if (kind == "navigate" || kind == "view-source") {
        const QUrl url((kind == "view-source" ? QStringLiteral("view-source:") : QString())
                       + row.value("url").toString());
        if (!url.isValid() || url.isRelative()) return false;
        m_navigationRequest = row;
        m_navigationRequest.insert("url", url.toString());
        page->setUrl(url);
    } else if (kind == "reload") {
        if (m_documentHasFormData) return confirmFormReload(row);
        m_navigationRequest = row;
        m_navigationRequest.insert("url", page->url().toString());
        page->triggerAction(row.value("bypassCache").toBool()
            ? QWebEnginePage::ReloadAndBypassCache : QWebEnginePage::Reload);
    } else if (kind == "stop") {
        page->triggerAction(QWebEnginePage::Stop);
    } else if (kind == "history") {
        bool valid = false;
        const int offset = row.value("offset").toInt(&valid);
        if (!valid || !offset || offset < -100 || offset > 100) return false;
        auto *history = page->history();
        const auto destination = history->itemAt(history->currentItemIndex() + offset);
        if (!destination.isValid()) {
            m_browserUi->setProperty("status_text", offset < 0 ? "No earlier history" : "No later history");
            return false;
        }
        m_navigationRequest = row;
        m_navigationRequest.insert("url", destination.url().toString());
        const int index = currentTabIndex();
        QMetaObject::invokeMethod(m_browserUi, "navigation_kind_for", Qt::DirectConnection,
            Q_ARG(int, index), Q_ARG(bool, true));
        history->goToItem(destination);
    } else if (kind == "find" || kind == "clear-find") {
        const QString query = kind == "find" ? row.value("query").toString() : QString();
        if (query.size() > 65536) return false;
        QWebEnginePage::FindFlags flags;
        if (row.value("backward").toBool()) flags |= QWebEnginePage::FindBackward;
        const QString mode = row.value("caseMode").toString();
        const bool uppercase = query.contains(QRegularExpression(QStringLiteral("\\p{Lu}")));
        if (mode == "sensitive" || (mode == "smart" && uppercase)) flags |= QWebEnginePage::FindCaseSensitively;
        const auto serial = ++m_findSerial;
        const auto operation = m_pageOwner->captureOperation();
        QPointer<FerricPublicPageHost> guard(this);
        QPointer<FerricPublicPageOwner> owner(m_pageOwner);
        page->findText(query, flags, [guard, owner, operation, row, serial](const QWebEngineFindTextResult &result) {
            if (!guard || !owner || guard->m_findSerial != serial || !owner->isCurrent(operation)
                || !guard->requestIsCurrent(row)) return;
            const QVariantMap value{{"count", result.numberOfMatches()}, {"activeMatch", result.activeMatch()}};
            emit guard->capturedCommandFinished(row, value);
            guard->m_browserUi->setProperty("status_text", result.numberOfMatches()
                ? QStringLiteral("Match %1 of %2").arg(result.activeMatch()).arg(result.numberOfMatches())
                : QStringLiteral("No matches"));
        });
    } else if (kind == "edit") {
        const QString operation = row.value("operation").toString();
        const QHash<QString, QWebEnginePage::WebAction> actions{
            {"copy", QWebEnginePage::Copy}, {"cut", QWebEnginePage::Cut},
            {"paste", QWebEnginePage::Paste}, {"paste-plain", QWebEnginePage::PasteAndMatchStyle},
            {"undo", QWebEnginePage::Undo}, {"redo", QWebEnginePage::Redo},
            {"select-all", QWebEnginePage::SelectAll}};
        if (!actions.contains(operation)) return false;
        page->triggerAction(actions.value(operation));
    } else if (kind == "zoom") {
        bool valid = false;
        const double factor = row.value("factor").toDouble(&valid);
        if (!valid || !std::isfinite(factor) || factor < 0.25 || factor > 5.0) return false;
        page->setZoomFactor(factor);
    } else if (kind == "evaluate" || kind == "scroll" || kind == "scroll-page"
               || kind == "scroll-to" || kind == "scroll-target") {
        const auto world = kind == "evaluate" && row.value("world").toString() == "page"
            ? QWebEngineScript::MainWorld : QWebEngineScript::ApplicationWorld;
        if (row.value("script").toString().isEmpty()) return false;
        return runCapturedScript(row, row.value("script").toString(), world);
    } else {
        return false;
    }
    return true;
}
