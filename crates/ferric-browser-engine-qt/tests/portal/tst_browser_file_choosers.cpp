#include "../../src/gui_application.h"
#include "../../src/url_display.h"
#include <QDBusArgument>
#include <QDBusConnectionInterface>
#include <QDBusInterface>
#include <QDBusReply>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QHostAddress>
#include <QJsonArray>
#include <QJsonDocument>
#include <QPointer>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQmlExpression>
#include <QQuickItem>
#include <QQuickWindow>
#include <QRegularExpression>
#include <QTcpServer>
#include <QTcpSocket>
#include <QTemporaryDir>
#include <QUuid>
#include <QtTest>
#include <QtWebEngineCore/QWebEngineFrame>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <memory>

class BrowserFileChooserTests final : public QObject {
  Q_OBJECT
  QTemporaryDir directory;
  QTcpServer server;
  std::unique_ptr<QDBusInterface> control;
  std::unique_ptr<QQmlApplicationEngine> engine;
  QQuickWindow *window = nullptr;
  QString viewExpression = "testWindow.activeWebView()";
  QString hostExpression = "testWindow";
  QString uiExpression = "testWindow.browserUi";
  QString sourceUrl;
  QString upload;
  QString clickedWindow;
  int downloadHits = 0;

  static QString quote(const QString &s) {
    const auto json =
        QJsonDocument(QJsonArray{s}).toJson(QJsonDocument::Compact);
    return QString::fromUtf8(json.mid(1, json.size() - 2));
  }
  QVariant evaluate(const QString &script) {
    QQmlExpression expression(engine->rootContext(), window, script);
    const auto value = expression.evaluate();
    if (expression.hasError()) {
      QTest::qFail(qPrintable(expression.error().toString()), __FILE__,
                   __LINE__);
      return {};
    }
    return value;
  }
  QVariant page(const QString &script) {
    struct Reply {
      bool ready = false;
      QVariant value;
    };
    const auto result = std::make_shared<Reply>();
    auto frame =
        evaluate(viewExpression + ".mainFrame").value<QWebEngineFrame>();
    if (!frame.isValid()) {
      QTest::qFail("Owned page has no valid main frame", __FILE__, __LINE__);
      return {};
    }
    frame.runJavaScript(script, 0, [result](const QVariant &value) {
      result->value = value;
      result->ready = true;
    });
    QElapsedTimer deadline;
    deadline.start();
    while (!result->ready && deadline.elapsed() < 5000)
      QTest::qWait(10);
    if (!result->ready)
      QTest::qFail("Owned page callback did not arrive", __FILE__, __LINE__);
    return result->value;
  }
  QVariantList requests() {
    QDBusReply<QVariantList> reply = control->call("Requests");
    if (!reply.isValid()) {
      QTest::qFail("Owned portal became unavailable", __FILE__, __LINE__);
      return {};
    }
    return reply.value();
  }
  QVariantMap row(int index) {
    return qdbus_cast<QVariantMap>(requests().at(index));
  }
  void call(const QString &method) {
    const auto reply = control->call(method);
    QVERIFY(reply.type() != QDBusMessage::ErrorMessage);
  }
  void click(const QString &id) {
    const auto point =
        page("(()=>{const r=document.getElementById(" + quote(id) +
             ").getBoundingClientRect();return "
             "[r.x+r.width/2,r.y+r.height/2]})()")
            .toList();
    QCOMPARE(point.size(), 2);
    auto view =
        qobject_cast<QQuickItem *>(evaluate(viewExpression).value<QObject *>());
    auto host = qobject_cast<QQuickWindow *>(
        evaluate(hostExpression).value<QObject *>());
    QVERIFY(view && host);
    QVERIFY(view->isVisible());
    host->requestActivate();
    QTRY_VERIFY(host->isActive());
    view->forceActiveFocus();
    const auto scene =
        view->mapToScene(QPointF(point[0].toDouble(), point[1].toDouble()));
    QDBusInterface compositor(
        "org.ferricbrowser.ParentFixture", "/org/ferricbrowser/ParentFixture",
        "org.ferricbrowser.ParentFixture", QDBusConnection::sessionBus());
    clickedWindow.clear();
    QTRY_VERIFY(([&] {
      QDBusReply<QVariantList> reply = compositor.call("Windows");
      if (!reply.isValid())
        return false;
      for (const auto &item : reply.value()) {
        const auto window = qdbus_cast<QVariantMap>(item);
        if (window["title"].toString() == host->title() &&
            window["pid"].toLongLong() == QCoreApplication::applicationPid() &&
            window["mapped"].toBool())
          clickedWindow = window["id"].toString();
      }
      return !clickedWindow.isEmpty();
    })());
    QDBusReply<bool> sent =
        compositor.call("Click", clickedWindow, scene.x(), scene.y());
    QVERIFY(sent.isValid() && sent.value());
  }
  QVariantMap renderFacts() {
    QVariantMap facts;
    facts["pending"] =
        evaluate("testWindow.browserUi.library_pending").toBool();
    facts["failed"] =
        !evaluate("testWindow.browserUi.library_error").toString().isEmpty();
    auto retry = button("Retry");
    facts["retry_enabled"] = retry && retry->isEnabled();
    facts["caption_found"] = false;
    QList<QQuickItem *> items{window->contentItem()};
    while (!items.isEmpty()) {
      auto item = items.takeLast();
      if (item->property("text").toString() ==
          QString::fromUtf8("Updating history…")) {
        qreal opacity = 1;
        for (auto ancestor = item; ancestor; ancestor = ancestor->parentItem())
          opacity *= ancestor->opacity();
        facts["caption_found"] = true;
        facts["caption_opacity"] = opacity;
        facts["caption_visible"] = item->isVisible();
        break;
      }
      items.append(item->childItems());
    }
    return facts;
  }
  static bool readyFacts(const QVariantMap &facts) {
    return !facts["pending"].toBool() && !facts["failed"].toBool() &&
           facts["retry_enabled"].toBool() && facts["caption_found"].toBool() &&
           facts["caption_opacity"].toDouble() < 0.01;
  }
  void capture(const QString &name) {
    const auto output = qEnvironmentVariable("FERRIC_PORTAL_TEST_ARTIFACTS");
    if (output.isEmpty())
      return;
    QVERIFY(QDir().mkpath(output));
    const bool ready = name.endsWith("-ready");
    QElapsedTimer settling;
    settling.start();
    int stableFrames = 0;
    while (ready && stableFrames < 2 && settling.elapsed() < 5000) {
      const auto beforeFrame = renderFacts();
      if (!readyFacts(beforeFrame)) {
        stableFrames = 0;
        QTest::qWait(10);
        continue;
      }
      QSignalSpy frame(window, &QQuickWindow::frameSwapped);
      window->update();
      while (frame.isEmpty() && settling.elapsed() < 5000)
        QTest::qWait(5);
      if (!frame.isEmpty() && readyFacts(renderFacts()))
        ++stableFrames;
      else
        stableFrames = 0;
    }
    const auto before = renderFacts();
    const auto pixels = window->grabWindow();
    const auto after = renderFacts();
    QFile metadata(QDir(output).filePath(name + ".json"));
    QVERIFY(metadata.open(QIODevice::WriteOnly));
    metadata.write(QJsonDocument::fromVariant(
                       QVariantMap{{"name", name},
                                   {"before", before},
                                   {"after", after},
                                   {"stable_ready_frames", stableFrames}})
                       .toJson());
    QVERIFY(pixels.save(QDir(output).filePath(name + ".png")));
    if (ready) {
      QVERIFY2(stableFrames == 2,
               "Did not observe two presented frames with stable ready "
               "metadata and hidden update caption");
      QVERIFY(readyFacts(before));
      QVERIFY(readyFacts(after));
    }
  }
  QQuickItem *button(const QString &text, const QString &recordId = {}) {
    QList<QQuickItem *> pending{window->contentItem()};
    while (!pending.isEmpty()) {
      auto item = pending.takeLast();
      pending.append(item->childItems());
      if (!item->isVisible() || item->width() <= 0 || item->height() <= 0 ||
          !item->property("down").isValid() ||
          item->property("text").toString() != text)
        continue;
      const auto center =
          item->mapToScene(QPointF(item->width() / 2, item->height() / 2));
      bool inside = window->contentItem()->boundingRect().contains(center);
      bool matches = recordId.isEmpty();
      for (auto ancestor = item; ancestor; ancestor = ancestor->parentItem()) {
        if (ancestor->property("id").toString() == recordId)
          matches = true;
        if (ancestor->clip() &&
            !ancestor->boundingRect().contains(ancestor->mapFromScene(center)))
          inside = false;
      }
      if (inside && matches)
        return item;
    }
    return nullptr;
  }
  void press(QQuickItem *item) {
    QVERIFY(item && item->isEnabled());
    const auto point =
        item->mapToScene(QPointF(item->width() / 2, item->height() / 2));
    QDBusInterface compositor(
        "org.ferricbrowser.ParentFixture", "/org/ferricbrowser/ParentFixture",
        "org.ferricbrowser.ParentFixture", QDBusConnection::sessionBus());
    QDBusReply<bool> sent =
        compositor.call("Click", clickedWindow, point.x(), point.y());
    QVERIFY(sent.isValid() && sent.value());
  }
  void selectOwner(const QString &kind) {
    QTRY_VERIFY_WITH_TIMEOUT(
        evaluate("testWindow.activeWebView().url.toString()===" +
                 quote(sourceUrl) + " && !testWindow.activeWebView().loading")
            .toBool(),
        15000);
    QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.activeWebView().mainFrame")
                                 .value<QWebEngineFrame>()
                                 .isValid(),
                             15000);
    QTRY_VERIFY(page("!!document.getElementById('popup')").toBool());
    if (kind == "secondary" || kind == "private") {
      evaluate("testWindow.browserWindowFactory.createObject(null,{"
               "windowStartupUrl:" +
               quote(sourceUrl) +
               ",windowProfileName:'recovery-owner',windowProfileLabel:'"
               "Recovery owner',windowPrivateProfile:" +
               (kind == "private" ? "true" : "false") + "})");
      QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),
                   2);
      hostExpression = "testWindow.browserWindowRegistry[1].host";
      uiExpression = "testWindow.browserWindowRegistry[1].ui";
      viewExpression = "testWindow.browserWindowRegistry[1].view";
    } else if (kind == "popup") {
      click("popup");
      if (QTest::currentTestFailed())
        return;
      QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(),
                   1);
      hostExpression = "testWindow.popupWindowRegistry[0].host";
      uiExpression = "testWindow.popupWindowRegistry[0].ui";
      viewExpression = "testWindow.popupWindowRegistry[0].view";
    }
    QTRY_VERIFY_WITH_TIMEOUT(
        evaluate(hostExpression + ".profileBootstrapComplete && !" +
                 uiExpression + ".profile_bootstrap_pending && !!" +
                 viewExpression + " && !" + viewExpression + ".loading")
            .toBool(),
        15000);
    QTRY_VERIFY_WITH_TIMEOUT(evaluate(viewExpression +
                                      ".url.toString()===" + quote(sourceUrl) +
                                      " && !" + viewExpression + ".loading")
                                 .toBool(),
                             15000);
    QTRY_VERIFY(evaluate(viewExpression + ".mainFrame")
                    .value<QWebEngineFrame>()
                    .isValid());
    QTRY_VERIFY(page("document.readyState==='complete' && "
                     "!!document.getElementById('file')")
                    .toBool());
    QCOMPARE(evaluate(uiExpression + ".desktop_portal_mode").toString(),
             QString("required"));
  }
private slots:
  void initTestCase() {
    QCOMPARE(qEnvironmentVariable("FERRIC_PORTAL_TEST_BUS"),
             QString("isolated"));
    QCOMPARE(QGuiApplication::platformName(), QString("wayland"));
    QDBusReply<QString> owner =
        QDBusConnection::sessionBus().interface()->serviceOwner(
            "org.freedesktop.portal.Desktop");
    QVERIFY(owner.isValid());
    control = std::make_unique<QDBusInterface>(
        owner.value(), "/org/ferricbrowser/PortalFixture",
        "org.ferricbrowser.PortalFixture", QDBusConnection::sessionBus());
    QVERIFY(control->isValid());
    QVERIFY(directory.isValid());
    upload = directory.filePath(QString::fromUtf8("owned résumé + %2F#?.txt"));
    QFile file(upload);
    QVERIFY(file.open(QIODevice::WriteOnly));
    file.write("Owned upload fixture");
  }
  void init() {
    downloadHits = 0;
    const auto historyBase =
        qEnvironmentVariable("FERRIC_DOWNLOAD_HISTORY_PROFILE");
    const auto historyTag = qEnvironmentVariable("FERRIC_DOWNLOAD_HISTORY_TAG");
    if (!historyBase.isEmpty()) {
      QVERIFY(QFileInfo(QDir(historyBase).filePath(".ferric-owned-fixture"))
                  .isFile());
      QVERIFY(QRegularExpression("^[a-z0-9-]+$").match(historyTag).hasMatch());
    }
    QTest::failOnWarning(
        QRegularExpression("TypeError|ReferenceError|Binding loop|Cannot "
                           "assign|Unable to assign"));
    viewExpression = "testWindow.activeWebView()";
    hostExpression = "testWindow";
    uiExpression = "testWindow.browserUi";
    call("RestoreName");
    const auto configured = control->call("Configure", QVariantMap{});
    QVERIFY(configured.type() != QDBusMessage::ErrorMessage);
    const bool recovering = QByteArray(QTest::currentTestFunction()) ==
                            "recoversAfterServiceReturns";
    if (recovering)
      call("DropName");
    QVERIFY(server.listen(QHostAddress::LocalHost));
    connect(&server, &QTcpServer::newConnection, this, [this] {
      while (auto socket = server.nextPendingConnection()) {
        connect(socket, &QTcpSocket::readyRead, socket, [this, socket] {
          auto data =
              socket->property("request").toByteArray() + socket->readAll();
          socket->setProperty("request", data);
          if (!data.contains("\r\n\r\n"))
            return;
          if (data.split(' ').value(1).split('?').first() == "/download") {
            ++downloadHits;
            socket->write("HTTP/1.1 200 OK\r\nContent-Type: "
                          "application/octet-stream\r\nContent-Disposition: "
                          "attachment; filename=owned.txt\r\nContent-Length: "
                          "8\r\nConnection: close\r\n\r\nfixture!");
            socket->disconnectFromHost();
            return;
          }
          QByteArray body =
              "<!doctype html><title>Owned upload</title>"
              "<a id='popup' target='_blank' href='/fixture.html' "
              "style='position:fixed;left:260px;top:10px'>Open popup</a>"
              "<a id='download' href='/download' "
              "style='position:fixed;left:260px;top:100px'>Save fixture</a>"
              "<input id='file' type='file' "
              "onclick='window.fileClicks=(window.fileClicks||0)+1;window."
              "fileTrust=event.isTrusted' "
              "style='position:fixed;left:20px;top:60px;width:220px;height:"
              "45px'>";
          const auto tag = qgetenv("FERRIC_DOWNLOAD_HISTORY_TAG");
          if (!tag.isEmpty())
            body.replace("href='/download'",
                         QByteArray("href='/download?run=") + tag + "'");
          socket->write("HTTP/1.1 200 OK\r\nContent-Type: text/html; "
                        "charset=utf-8\r\nContent-Length: " +
                        QByteArray::number(body.size()) +
                        "\r\nConnection: close\r\n\r\n" + body);
          socket->disconnectFromHost();
        });
        connect(socket, &QTcpSocket::disconnected, socket,
                &QObject::deleteLater);
      }
    });
    sourceUrl =
        QString("http://127.0.0.1:%1/fixture.html").arg(server.serverPort());
    engine = std::make_unique<QQmlApplicationEngine>();
    const QString configuration =
        R"JSON({"schema_version":3,"desktop":{"portals":"required"}})JSON";
    engine->setInitialProperties(
        {{"temporaryProfile",
          QByteArray(QTest::currentTestFunction()) != "downloadCanRetry"},
         {"storageBasePath",
          historyBase.isEmpty() ? directory.filePath("profile") : historyBase},
         {"softwareRendering", true},
         {"nativeWayland", true},
         {"safeMode", historyBase.isEmpty()},
         {"userscriptsOff", true},
         {"startupConfigJson", configuration},
         {"startupConfigBaseJson", configuration},
         {"startupCliOverridesJson", "{\"schema_version\":3}"},
         {"startupProfileOverridesJson", "{\"schema_version\":3}"},
         {"startupUrl", sourceUrl},
         {"width", 1000},
         {"height", 800}});
    engine->load(QUrl("qrc:/qt/qml/io/github/ferricbrowser/qml/Main.qml"));
    QCOMPARE(engine->rootObjects().size(), 1);
    window = qobject_cast<QQuickWindow *>(engine->rootObjects().first());
    QVERIFY(window);
    engine->rootContext()->setContextProperty("testWindow", window);
    QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.profileBootstrapComplete && "
                                      "!!testWindow.activeWebView() && "
                                      "!testWindow.activeWebView().loading")
                                 .toBool(),
                             15000);
    QTRY_VERIFY_WITH_TIMEOUT(
        evaluate("testWindow.browserUi.portal_capability_"
                 "status('file_chooser')===" +
                 quote(recovering ? "unavailable" : "available"))
            .toBool(),
        10000);
  }
  void cleanup() {
    call("RestoreName");
    engine.reset();
    window = nullptr;
    server.close();
    server.disconnect(this);
    QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
  }
  void downloadOpeningCanBeCancelled_data() {
    QTest::addColumn<QString>("kind");
    for (const auto kind : {"primary", "secondary", "private", "popup"})
      QTest::newRow(kind) << QString::fromLatin1(kind);
  }
  void downloadOpeningCanBeCancelled() {
    QFETCH(QString, kind);
    selectOwner(kind);
    if (QTest::currentTestFailed())
      return;
    const auto configured =
        control->call("Configure", QVariantMap{{"hold", true}});
    QVERIFY(configured.type() != QDBusMessage::ErrorMessage);
    const int index = requests().size();
    click("download");
    if (QTest::currentTestFailed())
      return;
    QTRY_COMPARE_WITH_TIMEOUT(requests().size(), index + 1, 5000);
    QCOMPARE(row(index)["method"].toString(), QString("SaveFile"));
    QVERIFY(
        !evaluate(hostExpression + ".pendingDownloadId").toString().isEmpty());
    QDBusInterface compositor(
        "org.ferricbrowser.ParentFixture", "/org/ferricbrowser/ParentFixture",
        "org.ferricbrowser.ParentFixture", QDBusConnection::sessionBus());
    auto router =
        evaluate(hostExpression + ".browserKeyRouterObject").value<QObject *>();
    QVERIFY(router);
    QSignalSpy routerKeys(router,
                          SIGNAL(keyPressed(QString, int, int, bool, bool)));
    QVERIFY(routerKeys.isValid());
    QDBusReply<bool> sent = compositor.call("Escape", clickedWindow);
    QVERIFY(sent.isValid() && sent.value());
    QTRY_VERIFY_WITH_TIMEOUT(
        routerKeys.count() > 0 ||
            evaluate(hostExpression + ".pendingDownloadId.length===0").toBool(),
        1000);
    qInfo()
        << "Actual browser router classified keys during chooser preparation:"
        << routerKeys.count();
    QTRY_VERIFY_WITH_TIMEOUT(
        evaluate(hostExpression + ".pendingDownloadId.length===0").toBool(),
        1000);
    QCOMPARE(routerKeys.count(), 0);
    QTRY_VERIFY_WITH_TIMEOUT(row(index)["close_count"].toInt() > 0, 1000);
    const auto released =
        control->call("Release", row(index)["path"].toString(), false);
    QVERIFY(released.type() != QDBusMessage::ErrorMessage);
  }
  void downloadCanRetry() {
    selectOwner("primary");
    if (QTest::currentTestFailed())
      return;
    const int index = requests().size();
    click("download");
    if (QTest::currentTestFailed())
      return;
    QTRY_COMPARE_WITH_TIMEOUT(requests().size(), index + 1, 5000);
    QCOMPARE(row(index)["method"].toString(), QString("SaveFile"));
    QCOMPARE(downloadHits, 1);
    const auto firstId = evaluate("testWindow.pendingDownloadId").toString();
    QVERIFY(!firstId.isEmpty());
    QVERIFY(!QUuid(firstId).isNull());
    const auto failed = control->call("Respond", row(index)["path"].toString(),
                                      uint(2), QVariantMap{});
    QVERIFY(failed.type() != QDBusMessage::ErrorMessage);
    QTRY_COMPARE(evaluate("testWindow.pendingDownloadId").toString(),
                 QString());
    QTRY_COMPARE(
        evaluate("testWindow.profileResourceRegistry.downloadOwners.length")
            .toInt(),
        0);
    const auto failedRecord = quote(firstId + "\tinterrupted\t");
    QTRY_VERIFY_WITH_TIMEOUT(
        evaluate("testWindow.browserUi.list_downloads().indexOf(" +
                 failedRecord + ")>=0")
            .toBool(),
        10000);
    QVERIFY(evaluate("testWindow.browserUi.execute_command(':downloads')")
                .toBool());
    evaluate("testWindow.executePendingEngineAction()");
    QTRY_VERIFY(evaluate("testWindow.downloadManagerVisible").toBool());
    QTRY_VERIFY(button("Details", firstId));
    QPointer<QQuickItem> details = button("Details", firstId);
    press(details);
    QTRY_COMPARE(details->property("text").toString(), QString("Hide details"));
    QSignalSpy frames(window, &QQuickWindow::frameSwapped);
    evaluate("testWindow.refreshDownloads()");
    window->update();
    QTRY_VERIFY(frames.count() > 0);
    QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
    QVERIFY2(details, "Refreshing downloads must retain the expanded row and "
                      "its keyboard focus target");
    QCOMPARE(details->property("text").toString(), QString("Hide details"));
    QTRY_VERIFY(button("Retry", firstId));
    QPointer<QQuickItem> retry = button("Retry", firstId);
    evaluate("testWindow.browserUi.refresh_library()");
    QVERIFY(evaluate("testWindow.browserUi.library_pending").toBool());
    // The session catalog retains this validated source independently of
    // the saved-history refresh. Its owned retry must remain usable.
    QVERIFY2(retry->isEnabled(),
             "An owned session retry remains available during history refresh");
    capture("downloads-updating");
    QTRY_VERIFY(!evaluate("testWindow.browserUi.library_pending").toBool());
    QTRY_VERIFY(retry && retry->isEnabled());
    capture("downloads-ready");
    press(retry);
    QTRY_COMPARE_WITH_TIMEOUT(requests().size(), index + 2, 5000);
    QCOMPARE(row(index + 1)["method"].toString(), QString("SaveFile"));
    QCOMPARE(downloadHits, 2);
    const auto retryId = evaluate("testWindow.pendingDownloadId").toString();
    QVERIFY(!retryId.isEmpty() && retryId != firstId);
    QVERIFY(!QUuid(retryId).isNull());
    const auto historyBase =
        qEnvironmentVariable("FERRIC_DOWNLOAD_HISTORY_PROFILE");
    const auto destination =
        historyBase.isEmpty()
            ? directory.filePath("retried résumé + %2F.txt")
            : QDir(historyBase)
                  .filePath(
                      qEnvironmentVariable("FERRIC_DOWNLOAD_HISTORY_TAG") +
                      "-retried résumé + %2F.txt");
    QVERIFY(!QFileInfo::exists(destination));
    const auto accepted = control->call(
        "Respond", row(index + 1)["path"].toString(), uint(0),
        QVariantMap{{"uris", QStringList{QUrl::fromLocalFile(destination)
                                             .toString(QUrl::FullyEncoded)}}});
    QVERIFY(accepted.type() != QDBusMessage::ErrorMessage);
    QTRY_VERIFY_WITH_TIMEOUT(QFileInfo::exists(destination), 10000);
    QFile saved(destination);
    QVERIFY(saved.open(QIODevice::ReadOnly));
    QCOMPARE(saved.readAll(), QByteArray("fixture!"));
    const auto completed = quote(retryId + "\tcompleted\t");
    QTRY_VERIFY_WITH_TIMEOUT(
        evaluate("testWindow.browserUi.list_downloads().indexOf(" + completed +
                 ")>=0")
            .toBool(),
        10000);
    QCOMPARE(evaluate("testWindow.activeWebView().url.toString()").toString(),
             sourceUrl);
  }
  void openingCanBeCancelled_data() {
    QTest::addColumn<QString>("kind");
    QTest::addColumn<bool>("pointer");
    for (const auto kind : {"primary", "secondary", "private", "popup"}) {
      QTest::newRow(kind) << QString::fromLatin1(kind) << false;
      QTest::newRow((QByteArray(kind) + "-pointer").constData())
          << QString::fromLatin1(kind) << true;
    }
  }
  void openingCanBeCancelled() {
    QFETCH(QString, kind);
    QFETCH(bool, pointer);
    selectOwner(kind);
    if (QTest::currentTestFailed())
      return;
    const auto configured =
        control->call("Configure", QVariantMap{{"hold", true}});
    QVERIFY(configured.type() != QDBusMessage::ErrorMessage);
    page(
        "window.addEventListener('keydown',event=>{"
        "if(event.key==='Escape')window.escapeKeys=(window.escapeKeys||0)+1})");
    const int index = requests().size();
    click("file");
    if (QTest::currentTestFailed())
      return;
    QTRY_COMPARE(page("window.fileClicks||0").toInt(), 1);
    QVERIFY(page("window.fileTrust").toBool());
    QTRY_COMPARE_WITH_TIMEOUT(requests().size(), index + 1, 5000);
    QDBusInterface compositor(
        "org.ferricbrowser.ParentFixture", "/org/ferricbrowser/ParentFixture",
        "org.ferricbrowser.ParentFixture", QDBusConnection::sessionBus());
    QDBusReply<bool> sent;
    if (pointer) {
      auto host = qobject_cast<QQuickWindow *>(
          evaluate(hostExpression).value<QObject *>());
      QVERIFY(host);
      QQuickItem *cancel = nullptr;
      QTRY_VERIFY(([&] {
        for (auto item :
             host->findChildren<QQuickItem *>("cancelFileChooserOpening"))
          if (item->isVisible())
            cancel = item;
        return cancel && cancel->width() > 0 && cancel->height() > 0;
      })());
      // Wait until the delayed surface has painted before clicking its button.
      QTest::qWait(250);
      const auto point = cancel->mapToScene(
          QPointF(cancel->width() / 2, cancel->height() / 2));
      sent = compositor.call("Click", clickedWindow, point.x(), point.y());
    } else {
      sent = compositor.call("Escape", clickedWindow);
    }
    QVERIFY(sent.isValid() && sent.value());
    QTRY_VERIFY_WITH_TIMEOUT(
        evaluate(hostExpression + ".pendingFileDialogRequest===null")
                .toBool() ||
            page("window.escapeKeys||0").toInt() > 0,
        1000);
    qInfo() << "Cancellation input delivered; page Escape count"
            << page("window.escapeKeys||0").toInt();
    QTRY_VERIFY_WITH_TIMEOUT(
        evaluate(hostExpression + ".pendingFileDialogRequest===null && !" +
                 uiExpression + ".has_native_prompt_lease()")
            .toBool(),
        1000);
    QTRY_VERIFY_WITH_TIMEOUT(row(index)["close_count"].toInt() > 0, 1000);
    QCOMPARE(evaluate(uiExpression + ".status_text").toString(),
             QString("File selection cancelled"));
    const auto released =
        control->call("Release", row(index)["path"].toString(), false);
    QVERIFY(released.type() != QDBusMessage::ErrorMessage);
  }
  void recoversAfterServiceReturns_data() {
    QTest::addColumn<QString>("kind");
    for (const auto kind : {"primary", "secondary", "private", "popup"})
      QTest::newRow(kind) << QString::fromLatin1(kind);
  }
  void recoversAfterServiceReturns() {
    QFETCH(QString, kind);
    selectOwner(kind);
    if (QTest::currentTestFailed())
      return;
    const int index = requests().size();
    call("RestoreName");
    QDBusReply<bool> restored =
        QDBusConnection::sessionBus().interface()->isServiceRegistered(
            "org.freedesktop.portal.Desktop");
    QVERIFY(restored.isValid() && restored.value());
    click("file");
    if (QTest::currentTestFailed())
      return;
    QTRY_COMPARE(page("window.fileClicks||0").toInt(), 1);
    QVERIFY(page("window.fileTrust").toBool());
    QTRY_VERIFY(evaluate(uiExpression + ".status_text")
                    .toString()
                    .contains("file chooser"));
    qInfo() << "Upload click trusted; portal service restored; UI status"
            << evaluate(uiExpression + ".status_text").toString();
    QTRY_COMPARE_WITH_TIMEOUT(requests().size(), index + 1, 5000);
    QTRY_COMPARE(evaluate(uiExpression + ".status_text").toString(),
                 QString("The system file chooser is open"));
    auto host = qobject_cast<QQuickWindow *>(
        evaluate(hostExpression).value<QObject *>());
    QVERIFY(host);
    const auto request = row(index);
    QDBusInterface parent(
        "org.ferricbrowser.ParentFixture", "/org/ferricbrowser/ParentFixture",
        "org.ferricbrowser.ParentFixture", QDBusConnection::sessionBus());
    QDBusReply<QVariantList> exports = parent.call("Exports");
    QVERIFY(exports.isValid());
    bool matched = false;
    for (const auto &item : exports.value()) {
      const auto exported = qdbus_cast<QVariantMap>(item);
      if (request["parent"].toString() !=
          "wayland:" + exported["handle"].toString())
        continue;
      QCOMPARE(exported["title"].toString(), host->title());
      QCOMPARE(exported["window_id"].toString(), clickedWindow);
      QCOMPARE(exported["pid"].toLongLong(),
               QCoreApplication::applicationPid());
      matched = true;
    }
    QVERIFY(matched);
    const auto reply = control->call(
        "Respond", request["path"].toString(), uint(0),
        QVariantMap{{"uris", QStringList{QUrl::fromLocalFile(upload).toString(
                                 QUrl::FullyEncoded)}}});
    QVERIFY(reply.type() != QDBusMessage::ErrorMessage);
    QTRY_COMPARE(page("document.getElementById('file').files.length").toInt(),
                 1);
    QCOMPARE(page("document.getElementById('file').files[0].name").toString(),
             QFileInfo(upload).fileName());
    QTRY_VERIFY(evaluate(hostExpression +
                         ".pendingFileDialogRequest===null && !" +
                         uiExpression + ".has_native_prompt_lease()")
                    .toBool());
    QCOMPARE(evaluate(uiExpression + ".status_text").toString(),
             QString("File selected"));
  }
};

extern "C" bool cxx_qt_init_crate_ferric_browser_engine_qt();
int main(int argc, char **argv) {
  ferric_browser_enable_software_rendering();
  ferric_browser_register_internal_scheme();
  QQuickWindow::setGraphicsApi(QSGRendererInterface::Software);
  QtWebEngineQuick::initialize();
  cxx_qt_init_crate_ferric_browser_engine_qt();
  QVector<QByteArray> arguments;
  for (int i = 0; i < argc; ++i)
    arguments.append(QByteArray(argv[i]));
  auto application = ferric_browser_gui_application(arguments);
  if (!application || !ferric_browser_gui_preferences_ready())
    return 2;
  BrowserFileChooserTests tests;
  return QTest::qExec(&tests, argc, argv);
}
#include "tst_browser_file_choosers.moc"
