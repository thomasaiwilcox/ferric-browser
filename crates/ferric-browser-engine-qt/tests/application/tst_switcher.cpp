#include "../../src/gui_application.h"
#include <QApplication>
#include <QDir>
#include <QDirIterator>
#include <QDBusInterface>
#include <QDBusReply>
#include <QFile>
#include <QFont>
#include <QGuiApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QKeyEvent>
#include <QJSValue>
#include <QInputMethodEvent>
#include <QQmlApplicationEngine>
#include <QPointer>
#include <QQmlContext>
#include <QQmlExpression>
#include <QQmlIncubationController>
#include <QQuickItem>
#include <QQuickWindow>
#include <QRegularExpression>
#include <QScopeGuard>
#include <QTcpServer>
#include <QTcpSocket>
#include <QTemporaryDir>
#include <QtTest>
#include <QWebEngineLoadingInfo>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <fcntl.h>
#include <algorithm>
#include <memory>
#include <utility>
#include <sqlite3.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <unistd.h>

extern "C" bool cxx_qt_init_crate_ferric_browser_engine_qt();
#include "../../src/url_display.h"

// Observe delivery through the real Wayland client event queue. A successful
// D-Bus request alone does not prove that the browser received the seat event.
class KeyDelivery final : public QObject {
  public:
    explicit KeyDelivery(QQuickWindow *window, int key, QObject *owner)
        : window(window), key(key), owner(owner) {
        qApp->installEventFilter(this);
    }
    bool pressed = false, released = false;
    int presses = 0, repeats = 0;
    quint32 scanCode = 0;
    QPointer<QQuickItem> focusItem;
    QVariant sourceState;
    Qt::KeyboardModifiers modifiers = Qt::NoModifier;
  private:
    QQuickWindow *window;
    int key;
    QObject *owner;
    bool eventFilter(QObject *watched, QEvent *event) override {
        if (watched == window && (event->type() == QEvent::KeyPress
                                 || event->type() == QEvent::KeyRelease)) {
            auto input = static_cast<QKeyEvent *>(event);
            if (input->key() == key) {
                if (event->type() == QEvent::KeyPress) {
                    ++presses;
                    if (input->isAutoRepeat()) ++repeats;
                    pressed = true;
                    focusItem = window->activeFocusItem();
                    sourceState = owner->property("switcherSourceState");
                    scanCode = input->nativeScanCode();
                    modifiers = input->modifiers();
                } else released = true;
            }
        }
        return false;
    }
};

class SwitcherTests final : public QObject {
    Q_OBJECT
    QTemporaryDir directory;
    std::unique_ptr<QQmlApplicationEngine> engine;
    QQuickWindow *window = nullptr;
    QTcpServer server;
    QString configPath, contextsPath;
    int startupPointSize = 12;
    QPointer<QQuickItem> lastKeyFocus;
    QVariant lastKeySourceState;
    QVariant documentResult;
    int documentRequestSerial = 0;
    bool documentReadDone = false;
    QVariant evaluate(const QString &source) {
        QQmlExpression expression(engine->rootContext(), window, source);
        auto result = expression.evaluate();
        if (expression.hasError())
            qFatal("Probe QML: %s", qPrintable(expression.error().toString()));
        return result;
    }
    static QString quoted(const QString &value) {
        auto bytes = QJsonDocument(QJsonArray{value}).toJson(QJsonDocument::Compact);
        return QString::fromUtf8(bytes.mid(1, bytes.size() - 2));
    }
    QString documentUrl(const QString &receiver = "testWindow.activeWebView()") {
        documentReadDone = false;
        const auto serial = ++documentRequestSerial;
        evaluate(receiver + ".runJavaScript('location.href', function(value){"
            "testProbe.acceptDocument(" + QString::number(serial) + ",value)})");
        QElapsedTimer timer;
        timer.start();
        while (!documentReadDone && timer.elapsed() < 5000)
            QTest::qWait(5);
        if (!documentReadDone) qFatal("Owned document callback timed out");
        return documentResult.toString();
    }
    static void write(const QString &path, const QByteArray &contents) {
        QFile file(path);
        QVERIFY(file.open(QIODevice::WriteOnly | QIODevice::Truncate));
        QCOMPARE(file.write(contents), contents.size());
        file.close();
    }
    void submit(const QString &command) {
        window->requestActivate();
        QTRY_VERIFY(window->isActive());
        evaluate("testWindow.browserUi.enter_command()");
        auto input = window->findChild<QQuickItem *>("commandLineInput");
        QVERIFY(input);
        QVERIFY(input->setProperty("text", command));
        input->forceActiveFocus();
        QTRY_VERIFY(input->hasActiveFocus());
        QTest::keyClick(window, Qt::Key_Return);
    }
    void launch(const QString &contexts) {
        const auto base = QFileInfo(configPath).absolutePath();
        const QString config = QStringLiteral(R"JSON({"schema_version":3,"ui":{"font_size_pt":%1}})JSON")
                                   .arg(startupPointSize);
        engine = std::make_unique<QQmlApplicationEngine>();
        engine->rootContext()->setContextProperty("testProbe", this);
        engine->setInitialProperties({{"temporaryProfile", false},
                                      {"storageBasePath", base + "/data"},
                                      {"softwareRendering", true},
                                      {"safeMode", false},
                                      {"userscriptsOff", true},
                                      {"startupConfigJson", config},
                                      {"startupConfigBaseJson", config},
                                      {"startupConfigPath", configPath},
                                      {"startupCliOverridesJson", "{\"schema_version\":3}"},
                                      {"startupProfileOverridesJson", "{\"schema_version\":3}"},
                                      {"startupContextsJson", contexts},
                                      {"startupUrl", "rb://blank"},
                                      {"width", 900},
                                      {"height", 700}});
        engine->load(QUrl("qrc:/qt/qml/io/github/ferricbrowser/qml/Main.qml"));
        QCOMPARE(engine->rootObjects().size(), 1);
        window = qobject_cast<QQuickWindow *>(engine->rootObjects().first());
        QVERIFY(window);
        engine->rootContext()->setContextProperty("testWindow", window);
        QTRY_VERIFY_WITH_TIMEOUT(
            evaluate(
                "testWindow.profileBootstrapComplete && "
                "!testWindow.browserUi.profile_bootstrap_pending && !!testWindow.activeWebView()")
                .toBool(),
            15000);
        window->show();
        window->requestActivate();
        QTRY_VERIFY(window->isActive());
    }
    void closeWindow() {
        if (engine && window) {
            evaluate("testWindow.browserWindowRegistry.slice(1).forEach(function(entry){entry.host."
                     "windowShutdownApproved=true; entry.host.close(); entry.host.destroy()})");
            QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        }
        engine.reset();
        window = nullptr;
        QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        QCoreApplication::processEvents();
    }
    static QQuickItem *visual(QQuickItem *parent, const QString &name) {
        if (parent->objectName() == name && parent->isVisible())
            return parent;
        for (auto child : parent->childItems())
            if (auto found = visual(child, name))
                return found;
        return nullptr;
    }
    QQuickWindow *createChild(const QString &path, bool privateProfile) {
        const auto before = evaluate("testWindow.browserWindowRegistry.length").toInt();
        const auto url = QString("http://127.0.0.1:%1/").arg(server.serverPort()) + path;
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:" +
                 quoted(url) + ",windowProfileName:" + quoted(path) + ",windowPrivateProfile:" +
                 (privateProfile ? "true" : "false") + ",windowEphemeralProfile:false})");
        QElapsedTimer timer;
        timer.start();
        while (evaluate("testWindow.browserWindowRegistry.length").toInt() != before + 1 &&
               timer.elapsed() < 10000)
            QTest::qWait(5);
        if (evaluate("testWindow.browserWindowRegistry.length").toInt() != before + 1)
            qFatal("Owned child did not register");
        const auto entry = "testWindow.browserWindowRegistry[" + QString::number(before) + "]";
        timer.restart();
        while (!evaluate(entry + ".host.profileBootstrapComplete && !" + entry +
                         ".ui.profile_bootstrap_pending && " + entry +
                         ".ui.current_url===" + quoted(url))
                    .toBool() &&
               timer.elapsed() < 15000)
            QTest::qWait(5);
        if (!evaluate(entry + ".host.profileBootstrapComplete && !" + entry +
                      ".ui.profile_bootstrap_pending")
                 .toBool())
            qFatal("Owned child did not open");
        return qobject_cast<QQuickWindow *>(evaluate(entry + ".host").value<QObject *>());
    }
    void submitAt(QQuickWindow *host, const QString &ui, const QString &command) {
        host->requestActivate();
        QTRY_VERIFY(host->isActive());
        evaluate(ui + ".enter_command()");
        auto field = visual(host->contentItem(), "commandLineInput");
        QVERIFY(field);
        QVERIFY(field->setProperty("text", command));
        field->forceActiveFocus();
        QTRY_VERIFY(field->hasActiveFocus());
        QTest::keyClick(host, Qt::Key_Return);
    }
    void captureChrome(QQuickWindow *host, QQuickItem *readable, const QString &name) {
        const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
        if (!artifacts.isEmpty()) {
            const auto path = artifacts + "/" + name + ".png";
            if (qEnvironmentVariable("FERRIC_SWITCHER_COMPOSITOR_KEYS") == "1") {
                QDBusInterface compositor("org.ferricbrowser.ParentFixture",
                    "/org/ferricbrowser/ParentFixture", "org.ferricbrowser.ParentFixture",
                    QDBusConnection::sessionBus());
                QDBusReply<QVariantList> windows = compositor.call("Windows");
                QVERIFY(windows.isValid());
                QString id;
                for (const auto &value : windows.value()) {
                    const auto candidate = qdbus_cast<QVariantMap>(value);
                    if (candidate["title"].toString() == host->title()
                        && candidate["pid"].toLongLong() == QCoreApplication::applicationPid()) {
                        QVERIFY(id.isEmpty());
                        id = candidate["id"].toString();
                    }
                }
                QVERIFY(!id.isEmpty());
                // Read the actual committed client buffer, independently of
                // Qt Quick's in-process software-window capture path.
                auto title = visual(host->contentItem(), "dialogTitleLabel");
                auto textItem = readable;
                QVERIFY(title && textItem);
                QImage image;
                const auto hasText = [&](QQuickItem *item) {
                    if (image.isNull()) return false;
                    const auto scale = image.width() / double(host->width());
                    auto area = item->mapRectToScene(QRectF(0, 0, item->width(), item->height()));
                    auto pixels = QRect(qFloor(area.x() * scale), qFloor(area.y() * scale),
                        qCeil(area.width() * scale), qCeil(area.height() * scale)).intersected(image.rect());
                    int ink = 0;
                    for (int y = pixels.top(); y <= pixels.bottom(); ++y)
                        for (int x = pixels.left(); x <= pixels.right(); ++x) {
                            const auto color = image.pixelColor(x, y);
                            if (color.red() > 180 && color.green() > 180 && color.blue() > 180
                                && ++ink > 100) return true;
                        }
                    return false;
                };
                QTRY_VERIFY2(([&] {
                    host->update();
                    QGuiApplication::sync();
                    QDBusReply<QByteArray> capture = compositor.call("Capture", id);
                    if (!capture.isValid()) return false;
                    image = QImage::fromData(capture.value(), "PNG");
                    return hasText(title) && hasText(textItem);
                })(), "The committed Wayland frame must paint the title and control text");
                QVERIFY(image.save(path));
            } else QVERIFY(host->grabWindow().save(path));
        }
    }
    std::pair<int, int> visibleRows(QQuickItem *list) {
        // Adding a root context property after creation re-evaluates QML
        // bindings. Geometry observation must not restart the layout it tests.
        const auto count = list->property("count").toInt();
        int first = count, last = -1;
        for (int index = 0; index < count; ++index) {
            QQuickItem *row = nullptr;
            if (!QMetaObject::invokeMethod(list, "itemAtIndex", Q_RETURN_ARG(QQuickItem *, row),
                                           Q_ARG(int, index)))
                qFatal("Cannot inspect ListView delegate");
            if (!row) continue;
            const auto y = row->mapToItem(list, QPointF()).y();
            if (y + row->height() > 1 && y < list->height() - 1) {
                first = std::min(first, index);
                last = std::max(last, index);
            }
        }
        return {first, last};
    }
    static QByteArray listGeometry(QQuickItem *list) {
        auto row = qobject_cast<QQuickItem *>(list->property("currentItem").value<QObject *>());
        return QJsonDocument(QJsonObject{
            {"index", list->property("currentIndex").toInt()},
            {"y", list->property("contentY").toDouble()},
            {"origin", list->property("originY").toDouble()},
            {"height", list->height()}, {"visible", list->isVisible()},
            {"enabled", list->isEnabled()}, {"rowHeight", row ? row->height() : -1}
        }).toJson(QJsonDocument::Compact);
    }
    void key(QQuickWindow *host, Qt::Key key, Qt::KeyboardModifiers modifiers = Qt::NoModifier) {
        lastKeyFocus.clear();
        lastKeySourceState.clear();
        if (qEnvironmentVariable("FERRIC_SWITCHER_COMPOSITOR_KEYS") != "1") {
            KeyDelivery delivery(host, key, window);
            QTest::keyClick(host, key, modifiers);
            lastKeyFocus = delivery.focusItem;
            lastKeySourceState = delivery.sourceState;
            QVERIFY(delivery.pressed && delivery.released);
            QCOMPARE(delivery.presses, 1);
            QCOMPARE(delivery.repeats, 0);
            QCOMPARE(delivery.modifiers, modifiers);
            return;
        }
        QString seatWindow;
        QCOMPARE(QGuiApplication::platformName(), QString("wayland"));
        QCOMPARE(qEnvironmentVariable("FERRIC_PORTAL_TEST_BUS"), QString("isolated"));
        QDBusInterface compositor(
            "org.ferricbrowser.ParentFixture", "/org/ferricbrowser/ParentFixture",
            "org.ferricbrowser.ParentFixture", QDBusConnection::sessionBus());
        QVERIFY(compositor.isValid());
        QTRY_VERIFY(([&] {
            QDBusReply<QVariantList> reply = compositor.call("Windows");
            if (!reply.isValid()) return false;
            QStringList matches;
            for (const auto &item : reply.value()) {
                const auto candidate = qdbus_cast<QVariantMap>(item);
                if (candidate["title"].toString() == host->title()
                    && candidate["pid"].toLongLong() == QCoreApplication::applicationPid()
                    && candidate["mapped"].toBool())
                    matches.append(candidate["id"].toString());
            }
            if (matches.size() != 1) return false;
            seatWindow = matches.first();
            return true;
        })());
        KeyDelivery delivery(host, key, window);
        // The seat sends the physical Tab key with Shift; the client reports
        // that gesture as Backtab after applying its keyboard map.
        QDBusReply<bool> sent = compositor.call("Key", seatWindow,
            int(key == Qt::Key_Backtab ? Qt::Key_Tab : key), int(modifiers));
        QVERIFY(sent.isValid() && sent.value());
        QTRY_VERIFY2(delivery.pressed && delivery.released,
            qPrintable(QString("key=%1, pressed=%2, released=%3, focus=%4")
                .arg(key).arg(delivery.pressed).arg(delivery.released)
                .arg(host->activeFocusItem() ? host->activeFocusItem()->objectName() : "none")));
        lastKeyFocus = delivery.focusItem;
        lastKeySourceState = delivery.sourceState;
        QCOMPARE(delivery.presses, 1);
        QCOMPARE(delivery.repeats, 0);
        QVERIFY(delivery.scanCode != 0);
        QCOMPARE(delivery.modifiers, modifiers);
        QTRY_COMPARE(QGuiApplication::queryKeyboardModifiers(), Qt::NoModifier);
    }
    bool journeyRecorded(const QString &url) {
        QDirIterator files(QFileInfo(configPath).absolutePath() + "/data", {"browser.sqlite"},
                           QDir::Files, QDirIterator::Subdirectories);
        if (!files.hasNext()) return false;
        sqlite3 *database = nullptr;
        const auto opened = sqlite3_open_v2(QFile::encodeName(files.next()).constData(), &database,
                                            SQLITE_OPEN_READONLY, nullptr);
        auto close = qScopeGuard([&] { sqlite3_close(database); });
        if (opened != SQLITE_OK) return false;
        sqlite3_stmt *query = nullptr;
        if (sqlite3_prepare_v2(database, "SELECT EXISTS(SELECT 1 FROM journey_nodes WHERE url=?1)",
                               -1, &query, nullptr) != SQLITE_OK) return false;
        auto finish = qScopeGuard([&] { sqlite3_finalize(query); });
        const auto bytes = url.toUtf8();
        if (sqlite3_bind_text(query, 1, bytes.constData(), -1, SQLITE_TRANSIENT) != SQLITE_OK)
            return false;
        return sqlite3_step(query) == SQLITE_ROW && sqlite3_column_int(query, 0) != 0;
    }
    QString journeyStorageState() {
        QJsonArray snapshots;
        const QDir root(QFileInfo(configPath).absolutePath() + "/data");
        QDirIterator files(root.path(), {"browser.sqlite"}, QDir::Files, QDirIterator::Subdirectories);
        while (files.hasNext()) {
            const auto path = files.next();
            sqlite3 *database = nullptr;
            const auto opened = sqlite3_open_v2(QFile::encodeName(path).constData(), &database,
                                                SQLITE_OPEN_READONLY, nullptr);
            auto close = qScopeGuard([&] { sqlite3_close(database); });
            QJsonArray urls;
            if (opened == SQLITE_OK) {
                sqlite3_stmt *query = nullptr;
                if (sqlite3_prepare_v2(database, "SELECT url FROM journey_nodes ORDER BY rowid DESC LIMIT 20",
                                       -1, &query, nullptr) == SQLITE_OK) {
                    auto finish = qScopeGuard([&] { sqlite3_finalize(query); });
                    while (sqlite3_step(query) == SQLITE_ROW)
                        urls.append(QString::fromUtf8(reinterpret_cast<const char *>(sqlite3_column_text(query, 0))));
                }
            }
            snapshots.append(QJsonObject{{"path", root.relativeFilePath(path)}, {"urls", urls}});
        }
        return QString::fromUtf8(QJsonDocument(snapshots).toJson(QJsonDocument::Compact));
    }
    void seedBookmarks(int count) {
        QVERIFY(evaluate("testWindow.browserUi.switcher_query('', 'marks')").toBool());
        QTRY_VERIFY(!evaluate("testWindow.browserUi.library_pending").toBool());
        QDirIterator files(QFileInfo(configPath).absolutePath() + "/data", {"browser.sqlite"},
                           QDir::Files, QDirIterator::Subdirectories);
        QVERIFY(files.hasNext());
        sqlite3 *database = nullptr;
        QCOMPARE(sqlite3_open_v2(QFile::encodeName(files.next()).constData(), &database,
                                 SQLITE_OPEN_READWRITE, nullptr), SQLITE_OK);
        auto close = qScopeGuard([&] { sqlite3_close(database); });
        sqlite3_stmt *insert = nullptr;
        QCOMPARE(sqlite3_prepare_v2(database,
            "INSERT INTO bookmarks(id,url,title,created_at,updated_at) VALUES(?1,?2,?3,1,1)",
            -1, &insert, nullptr), SQLITE_OK);
        auto finish = qScopeGuard([&] { sqlite3_finalize(insert); });
        for (int index = 0; index < count; ++index) {
            const auto id = QString("paging-%1").arg(index).toUtf8();
            const auto url = QByteArray("https://example.test/") + id;
            const auto title = id + " — a saved bookmark with a long readable title";
            QCOMPARE(sqlite3_bind_text(insert, 1, id.constData(), -1, SQLITE_TRANSIENT), SQLITE_OK);
            QCOMPARE(sqlite3_bind_text(insert, 2, url.constData(), -1, SQLITE_TRANSIENT), SQLITE_OK);
            QCOMPARE(sqlite3_bind_text(insert, 3, title.constData(), -1, SQLITE_TRANSIENT), SQLITE_OK);
            QCOMPARE(sqlite3_step(insert), SQLITE_DONE);
            QCOMPARE(sqlite3_reset(insert), SQLITE_OK);
        }
        evaluate("testWindow.browserUi.refresh_library()");
        QTRY_VERIFY(!evaluate("testWindow.browserUi.library_pending").toBool());
    }
    void verifyPages(QQuickWindow *host, QQuickItem *list, QQuickItem *input) {
        // Model publication and focus can precede the first layout after a
        // resize or font change. Measure an actually presented initial page.
        QSignalSpy initialFrame(host, &QQuickWindow::frameSwapped);
        host->update();
        QTRY_VERIFY(!initialFrame.isEmpty());
        for (int direction : {1, -1}) {
            bool complete = false;
            for (int step = 0; step < list->property("count").toInt() * 10 + 10; ++step) {
                QTRY_VERIFY(list->isVisible() && list->isEnabled() && list->height() > 0);
                QVERIFY(host->isActive());
                QCOMPARE(QGuiApplication::focusWindow(), host);
                const auto before = visibleRows(list);
                const auto oldIndex = list->property("currentIndex").toInt();
                const auto oldY = list->property("contentY").toDouble();
                key(host, direction > 0 ? Qt::Key_PageDown : Qt::Key_PageUp);
                if (input != list) QCOMPARE(lastKeyFocus.data(), input);
                // A frame emitted during keyClick's internal event processing
                // can precede the key press. Require one after delivery.
                QSignalSpy frame(host, &QQuickWindow::frameSwapped);
                host->update();
                QTRY_VERIFY(!frame.isEmpty());
                const auto after = visibleRows(list);
                if (before.first <= before.second && after.first <= after.second) {
                    if (!(after.first <= before.second + 1 && after.second >= before.first - 1))
                        qInfo() << "Paging gap after a delivered key" << direction
                                << "from index" << oldIndex << "contentY" << oldY
                                << listGeometry(list);
                    QVERIFY2(after.first <= before.second + 1 && after.second >= before.first - 1,
                        qPrintable(QString("Paging skipped visible rows %1..%2 to %3..%4")
                            .arg(before.first).arg(before.second).arg(after.first).arg(after.second)));
                } else {
                    // The controls above the first library result can occupy
                    // an entire page of their own.
                    QCOMPARE(list->property("currentIndex").toInt(), 0);
                    if (direction > 0 && after.first <= after.second) QCOMPARE(after.first, 0);
                }
                QVERIFY(input->hasActiveFocus());
                if (oldIndex == list->property("currentIndex").toInt()
                    && qAbs(oldY - list->property("contentY").toDouble()) < 1) {
                    if (oldIndex != (direction > 0 ? list->property("count").toInt() - 1 : 0))
                        qInfo() << "Paging made no progress after a delivered key"
                                << "direction" << direction << "contentY" << oldY
                                << listGeometry(list) << "focus at press" << lastKeyFocus
                                << "expected" << input << "source at press"
                                << lastKeySourceState.value<QJSValue>().toVariant();
                    QCOMPARE(oldIndex, direction > 0 ? list->property("count").toInt() - 1 : 0);
                    auto header = qobject_cast<QQuickItem *>(list->property("headerItem").value<QObject *>());
                    if (direction < 0 && header)
                        QVERIFY(header->mapToItem(list, QPointF()).y() >= -1);
                    complete = true;
                    break;
                }
            }
            QVERIFY(complete);
        }
    }
  public slots:
    void acceptDocument(int serial, const QVariant &value) {
        if (serial != documentRequestSerial) return;
        documentResult = value;
        documentReadDone = true;
    }
  private slots:
    void initTestCase() {
        const QDir source(QFileInfo(QString::fromUtf8(__FILE__)).absolutePath() + "/../../qml");
        for (const auto &name : {QString("components/FerricCommandLine.qml"),
                                 QString("components/FerricLibraryEntries.qml"),
                                 QString("components/FerricLibraryManager.qml"),
                                 QString("components/FerricManagerList.qml"),
                                 QString("components/FerricManagerFeedback.qml"),
                                 QString("components/FerricProfileManager.qml"),
                                 QString("components/FerricSessionManager.qml"),
                                 QString("components/FerricDownloadManager.qml"),
                                 QString("components/FerricDownloadWorkspace.qml"),
                                 QString("components/FerricSettings.qml"),
                                 QString("components/FerricSettingRows.qml"),
                                 QString("components/FerricSettingsWorkspace.qml"),
                                 QString("components/FerricStartPage.qml"),
                                 QString("components/FerricRuntimeController.qml"),
                                 QString("components/FerricStartButton.qml"),
                                 QString("components/FerricWrappingButton.qml"),
                                 QString("components/FerricScrollColumn.qml"),
                                 QString("components/FerricJourneyGraph.qml"),
                                 QString("components/FerricPrimaryBrowserWindow.qml"),
                                 QString("components/FerricBrowserWindow.qml"),
                                 QString("components/FerricSwitcher.qml"),
                                 QString("components/FerricSwitcherResults.qml"),
                                 QString("scripts/ListNavigation.js")}) {
            QFile file(source.filePath(name));
            QFile embedded(":/qt/qml/io/github/ferricbrowser/qml/" + name);
            QVERIFY2(file.open(QIODevice::ReadOnly), qPrintable(file.fileName()));
            QVERIFY2(embedded.open(QIODevice::ReadOnly), qPrintable(embedded.fileName()));
            QCOMPARE(embedded.readAll(), file.readAll());
        }
    }
    void init() {
        QTest::failOnWarning(QRegularExpression(
            "qrc:/qt/qml/io/github/ferricbrowser/.*(ReferenceError|TypeError|Binding loop)"));
        QVERIFY(directory.isValid());
        auto base = directory.filePath(QString::fromLatin1(QTest::currentTestFunction()) + "/" +
                                       QString::fromLatin1(QTest::currentDataTag()));
        QVERIFY(QDir().mkpath(base));
        write(base + "/.ferric-owned-fixture", "Owned Switcher privacy regression only.\n");
        configPath = base + "/config.toml";
        contextsPath = base + "/contexts.toml";
        const auto function = QString::fromLatin1(QTest::currentTestFunction());
        startupPointSize = function.endsWith("Paging") || function == "switcherActionFocus"
            || function == "libraryDrafts" || function == "journeyDestination"
            || function == "journeyGraphKeyboard" || function == "managerFormsKeyboard"
            || function == "settingsDrafts" ? 20 : 12;
        write(configPath, "schema_version=3\n[ui]\nfont_size_pt="
                         + QByteArray::number(startupPointSize) + "\n");
        write(contextsPath, "schema_version=3\n");
        QVERIFY(server.listen(QHostAddress::LocalHost, 0));
        connect(&server, &QTcpServer::newConnection, this, [this] {
            while (auto socket = server.nextPendingConnection()) {
                connect(socket, &QTcpSocket::readyRead, socket, [socket] {
                    auto request = socket->property("requestBytes").toByteArray();
                    request += socket->readAll();
                    if (!request.contains("\r\n\r\n")) {
                        socket->setProperty("requestBytes", request);
                        return;
                    }
                    if (socket->property("responseSent").toBool())
                        return;
                    socket->setProperty("responseSent", true);
                    const QByteArray body =
                        "<!doctype html><title>Owned child secret</title><p>Owned fixture</p>";
                    socket->write("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: " +
                                  QByteArray::number(body.size()) +
                                  "\r\nConnection: close\r\n\r\n" + body);
                    socket->disconnectFromHost();
                });
                connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
            }
        });
        launch(R"JSON({"schema_version":3,"contexts":[],"routes":[]})JSON");
        QVERIFY(evaluate("!!testWindow.switcherRegistry && typeof "
                         "testWindow.switcherRegistry.switcherEntries==='function'")
                    .toBool());
    }
    void cleanup() {
        closeWindow();
        server.close();
        server.disconnect(this);
    }
    void commandCompletionKeyboard_data() {
        QTest::addColumn<bool>("secondary");
        QTest::newRow("primary") << false;
        QTest::newRow("secondary") << true;
    }
    void commandCompletionKeyboard() {
        QFETCH(bool, secondary);
        auto host = secondary ? createChild("completion", false) : window;
        QVERIFY(host);
        const QString ui = secondary ? "testWindow.browserWindowRegistry[1].ui"
                                     : "testWindow.browserUi";
        host->requestActivate();
        QTRY_VERIFY(host->isActive());
        evaluate(ui + ".enter_command()");
        auto input = visual(host->contentItem(), "commandLineInput");
        auto list = visual(host->contentItem(), "commandCompletionList");
        QVERIFY(input);
        QTRY_VERIFY(input->hasActiveFocus());
        QTRY_VERIFY((list = visual(host->contentItem(), "commandCompletionList")));
        const auto count = list->property("count").toInt();
        QVERIFY(count > 25);
        key(host, Qt::Key_Backtab, Qt::ShiftModifier);
        QTRY_COMPARE(evaluate(ui + ".completion_selected").toInt(), count - 1);
        auto selectionReady = [this, list, ui] {
            if (list->property("currentIndex").toInt() != evaluate(ui + ".completion_selected").toInt())
                return false;
            auto row = qobject_cast<QQuickItem *>(list->property("currentItem").value<QObject *>());
            if (!row) return false;
            const auto y = row->mapToItem(list, QPointF()).y();
            return y >= -1 && y + row->height() <= list->height() + 1;
        };
        auto selectionGeometry = [this, list, host, ui] {
            const int selected = evaluate(ui + ".completion_selected").toInt();
            QQuickItem *row = nullptr;
            QMetaObject::invokeMethod(list, "itemAtIndex", Qt::DirectConnection,
                Q_RETURN_ARG(QQuickItem *, row), Q_ARG(int, selected));
            QJsonObject state{{"selected", selected}, {"current", list->property("currentIndex").toInt()},
                {"row_ready", bool(row)}, {"row_y", row ? row->mapToItem(list, QPointF()).y() : -999},
                {"row_height", row ? row->height() : -1}, {"list_height", list->height()},
                {"content_y", list->property("contentY").toDouble()},
                {"content_height", list->property("contentHeight").toDouble()},
                {"origin", list->property("originY").toDouble()}, {"visible", list->isVisible()},
                {"popup_height", list->parentItem()->height()}, {"window_height", host->height()},
                {"font", host->property("font").value<QFont>().pointSize()},
                {"mode", evaluate(ui + ".mode").toString()}};
            return QString::fromUtf8(QJsonDocument(state).toJson(QJsonDocument::Compact));
        };
        QTRY_VERIFY2(selectionReady(), qPrintable(selectionGeometry()));
        auto surface = visual(host->contentItem(), "commandSurface");
        QVERIFY(surface);
        const auto labels = surface->property("completionText").toString();
        QVERIFY(surface->setProperty("completionText", labels + " — refreshed metadata"));
        QCOMPARE(list->property("count").toInt(), count);
        QTRY_VERIFY2(selectionReady(), qPrintable(selectionGeometry()));
        key(host, Qt::Key_Tab);
        QTRY_COMPARE(evaluate(ui + ".completion_selected").toInt(), 0);
        for (int index = 0; index < 24; ++index) key(host, Qt::Key_Tab);
        QTRY_COMPARE(evaluate(ui + ".completion_selected").toInt(), 24);
        QTRY_VERIFY2(selectionReady(), qPrintable(selectionGeometry()));
        key(host, Qt::Key_P, Qt::ControlModifier);
        QTRY_COMPARE(evaluate(ui + ".completion_selected").toInt(), 23);
        QTRY_VERIFY2(selectionReady(), qPrintable(selectionGeometry()));
        QVERIFY(!evaluate("testWindow.switcherVisible").toBool());
        QVERIFY(input->hasActiveFocus());
        QCOMPARE(input->property("text").toString(), QString());
        key(host, Qt::Key_Escape);
        QTRY_VERIFY(!input->isVisible());
    }
    void commandCompletionEditing_data() {
        QTest::addColumn<bool>("secondary");
        QTest::addColumn<QString>("gesture");
        for (bool child : {false, true}) {
            for (const auto &gesture : {QString("right"), QString("space"), QString("mouse")}) {
                const auto tag = (child ? "secondary-" : "primary-") + gesture;
                QTest::newRow(qPrintable(tag)) << child << gesture;
            }
        }
    }
    void commandCompletionEditing() {
        QFETCH(bool, secondary);
        QFETCH(QString, gesture);
        auto host = secondary ? createChild("command-editing", false) : window;
        QVERIFY(host);
        const QString ui = secondary ? "testWindow.browserWindowRegistry[1].ui"
                                     : "testWindow.browserUi";
        const QString view = secondary ? "testWindow.browserWindowRegistry[1].view"
                                       : "testWindow.activeWebView()";
        host->requestActivate();
        QTRY_VERIFY(host->isActive());
        const auto originalUrl = evaluate(ui + ".current_url").toString();
        evaluate(ui + ".enter_command()");
        auto input = visual(host->contentItem(), "commandLineInput");
        QVERIFY(input);
        const auto original = gesture == "mouse" ? QString("op --target tab") : QString("op");
        QVERIFY(input->setProperty("text", original));
        QVERIFY(input->setProperty("cursorPosition", 2));
        input->forceActiveFocus();
        QTRY_VERIFY(input->hasActiveFocus());
        QTRY_COMPARE(evaluate(ui + ".completion_end").toInt(), 2);
        const auto values = evaluate(ui + ".completion_values").toString().split('\n');
        QVERIFY(values.size() > 1);
        const auto index = gesture == "mouse" ? 1 : values.indexOf("open");
        QVERIFY(index >= 0);
        evaluate(ui + ".completion_select(" + QString::number(index) + ")");
        QTRY_COMPARE(evaluate(ui + ".completion_selected").toInt(), index);
        if (gesture == "mouse") {
            auto list = visual(host->contentItem(), "commandCompletionList");
            QVERIFY(list);
            QTRY_COMPARE(list->property("currentIndex").toInt(), index);
            QSignalSpy frame(host, &QQuickWindow::frameSwapped);
            host->update();
            QTRY_VERIFY(!frame.isEmpty());
            QTRY_COMPARE(list->property("currentIndex").toInt(), index);
            auto row = qobject_cast<QQuickItem *>(list->property("currentItem").value<QObject *>());
            QVERIFY(row);
            QTest::mouseClick(host, Qt::LeftButton, Qt::NoModifier,
                row->mapToScene(QPointF(row->width() / 2, row->height() / 2)).toPoint());
        } else QTest::keyClick(host, gesture == "space" ? Qt::Key_Space : Qt::Key_Right);
        const auto completed = values.at(index) + (gesture == "mouse" ? " --target tab"
                                                   : gesture == "space" ? " " : "");
        QTRY_COMPARE(input->property("text").toString(), completed);
        QCOMPARE(input->property("cursorPosition").toInt(),
                 values.at(index).size() + (gesture == "space" ? 1 : 0));
        QVERIFY(input->hasActiveFocus());
        QCOMPARE(evaluate(ui + ".mode").toString(), QString("command"));
        QCOMPARE(evaluate(ui + ".current_url").toString(), originalUrl);
        if (gesture == "mouse" || gesture == "right") {
            QTest::keyClick(host, Qt::Key_Z, Qt::ControlModifier);
            QCOMPARE(input->property("text").toString(), original);
            QCOMPARE(input->property("cursorPosition").toInt(), 2);
            QCOMPARE(input->property("selectedText").toString(), QString());
            if (gesture == "mouse") return;
            QTest::keyClick(host, Qt::Key_Z, Qt::ControlModifier | Qt::ShiftModifier);
            QCOMPARE(input->property("text").toString(), QString("open"));
            QTest::keyClick(host, Qt::Key_Space);
        }
        const auto url = QString("http://127.0.0.1:%1/completion-%2")
            .arg(server.serverPort()).arg(gesture);
        for (auto character : url) QTest::keyClick(host, character.toLatin1());
        QCOMPARE(input->property("text").toString(), "open " + url);
        QTest::keyClick(host, Qt::Key_Return);
        QTRY_COMPARE(evaluate(ui + ".current_url").toString(), url);
        QTRY_COMPARE(evaluate(view + ".url.toString()").toString(), url);
        QTRY_VERIFY(!evaluate(view + ".loading").toBool());
    }
    void commandCompletionPreedit_data() {
        QTest::addColumn<int>("key");
        QTest::newRow("right") << int(Qt::Key_Right);
        QTest::newRow("space") << int(Qt::Key_Space);
    }
    void commandCompletionPreedit() {
        QFETCH(int, key);
        evaluate("testWindow.browserUi.enter_command()");
        auto input = visual(window->contentItem(), "commandLineInput");
        QVERIFY(input);
        QVERIFY(input->setProperty("text", "op"));
        QVERIFY(input->setProperty("cursorPosition", 2));
        input->forceActiveFocus();
        QTRY_VERIFY(input->hasActiveFocus());
        const auto values = evaluate("testWindow.browserUi.completion_values").toString().split('\n');
        const auto index = values.indexOf("open");
        QVERIFY(index >= 0);
        evaluate("testWindow.browserUi.completion_select(" + QString::number(index) + ")");
        QInputMethodEvent preedit("composing", {});
        QCoreApplication::sendEvent(input, &preedit);
        QTRY_VERIFY(input->property("inputMethodComposing").toBool());
        QTest::keyClick(window, Qt::Key(key));
        QVERIFY(input->property("text").toString().trimmed() != values.at(index));
        QCOMPARE(evaluate("testWindow.browserUi.mode").toString(), QString("command"));
    }
    void switcherSearchEditing() {
        submit("switcher --scope commands");
        QTRY_VERIFY(evaluate("testWindow.switcherResultsCurrent() && "
                            "testWindow.switcherResults.length > 1").toBool());
        auto input = visual(window->contentItem(), "switcherSearchInput");
        auto list = visual(window->contentItem(), "switcherResultList");
        QVERIFY(input);
        QVERIFY(list);
        QVERIFY(input->setProperty("text", "open"));
        QTRY_VERIFY(evaluate("testWindow.switcherResultsCurrent() && "
                            "testWindow.switcherResultContext.query === 'open' && "
                            "testWindow.switcherResults.length > 1").toBool());
        QVERIFY(input->setProperty("cursorPosition", 2));
        QTest::keyClick(window, Qt::Key_Home);
        QCOMPARE(input->property("cursorPosition").toInt(), 0);
        QCOMPARE(input->property("text").toString(), QString("open"));
        QTest::keyClick(window, Qt::Key_End, Qt::ShiftModifier);
        QCOMPARE(input->property("selectedText").toString(), QString("open"));
        QTest::keyClick(window, Qt::Key_End, Qt::ControlModifier);
        QTRY_COMPARE(list->property("currentIndex").toInt(), list->property("count").toInt() - 1);
        QCOMPARE(input->property("text").toString(), QString("open"));
        QTest::keyClick(window, Qt::Key_Home, Qt::ControlModifier);
        QTRY_COMPARE(list->property("currentIndex").toInt(), 0);
        QCOMPARE(input->property("text").toString(), QString("open"));
        QTest::keyClick(window, Qt::Key_N, Qt::ControlModifier);
        QTRY_COMPARE(list->property("currentIndex").toInt(), 1);
        QCOMPARE(input->property("text").toString(), QString("open"));
        QTest::keyClick(window, Qt::Key_P, Qt::ControlModifier);
        QTRY_COMPARE(list->property("currentIndex").toInt(), 0);
        QCOMPARE(input->property("text").toString(), QString("open"));
        QVERIFY(input->hasActiveFocus());
    }
    void normalCtrlPUsesConfiguredBinding() {
        evaluate("testWindow.restoreModeFocus()");
        QTRY_VERIFY(evaluate("testWindow.activeWebView().activeFocus").toBool());
        QVERIFY(!evaluate("testWindow.tabsModel.get(0).pinned").toBool());
        key(window, Qt::Key_P, Qt::ControlModifier);
        QTRY_VERIFY(evaluate("testWindow.tabsModel.get(0).pinned").toBool());
        QVERIFY(!evaluate("testWindow.switcherVisible").toBool());
        key(window, Qt::Key_P, Qt::ControlModifier);
        QTRY_VERIFY(!evaluate("testWindow.tabsModel.get(0).pinned").toBool());
        key(window, Qt::Key_Space, Qt::ControlModifier);
        QTRY_VERIFY(evaluate("testWindow.switcherVisible").toBool());
    }
    void switcherPaging_data() {
        QTest::addColumn<bool>("secondary");
        QTest::newRow("primary") << false;
        QTest::newRow("secondary") << true;
    }
    void switcherDeferredPaging_data() {
        QTest::addColumn<bool>("secondary");
        QTest::newRow("primary") << false;
        QTest::newRow("secondary") << true;
    }
    void switcherDeferredPaging() {
        QFETCH(bool, secondary);
        auto host = secondary ? createChild("deferred-paging", false) : window;
        const QString ui = secondary ? "testWindow.browserWindowRegistry[1].ui"
                                     : "testWindow.browserUi";
        host->resize(390, 600);
        submitAt(host, ui, "switcher --scope commands");
        QTRY_VERIFY(evaluate("testWindow.switcherResultsCurrent() && "
                            "testWindow.switcherResults.length > 50 && "
                            "testWindow.switcherBatchIndex === testWindow.switcherBatchEntries.length && "
                            "!testWindow.switcherSourceState.pending").toBool());
        auto list = visual(host->contentItem(), "switcherResultList");
        auto input = visual(host->contentItem(), "switcherSearchInput");
        QVERIFY(list && input);
        QVERIFY(list->setProperty("cacheBuffer", 0));
        QVERIFY(list->setProperty("currentIndex", 50));
        QVERIFY(QMetaObject::invokeMethod(list, "positionViewAtIndex", Q_ARG(int, 50), Q_ARG(int, 0)));
        QSignalSpy frame(host, &QQuickWindow::frameSwapped);
        host->update();
        QTRY_VERIFY(!frame.isEmpty());
        // Evict the initial page, then leave background delegate incubation
        // paused. Navigation must also work through uncached results.
        QQmlIncubationController paused;
        auto original = engine->incubationController();
        engine->setIncubationController(&paused);
        auto restore = qScopeGuard([&] { engine->setIncubationController(original); });
        QVERIFY(list->setProperty("currentIndex", 0));
        QVERIFY(QMetaObject::invokeMethod(list, "positionViewAtBeginning"));
        input->forceActiveFocus();
        verifyPages(host, list, input);
    }
    void switcherActionFocus_data() {
        QTest::addColumn<bool>("secondary");
        QTest::newRow("primary") << false;
        QTest::newRow("secondary") << true;
    }
    void switcherActionFocus() {
        QFETCH(bool, secondary);
        auto host = secondary ? createChild("action-focus", false) : window;
        const QString ui = secondary ? "testWindow.browserWindowRegistry[1].ui"
                                     : "testWindow.browserUi";
        host->resize(390, 600);
        submitAt(host, ui, "switcher --scope commands");
        QTRY_VERIFY(evaluate("testWindow.switcherResultsCurrent() && "
                            "testWindow.switcherResults.length > 20 && "
                            "testWindow.switcherBatchIndex === testWindow.switcherBatchEntries.length && "
                            "!testWindow.switcherSourceState.pending").toBool());
        auto list = visual(host->contentItem(), "switcherResultList");
        auto input = visual(host->contentItem(), "switcherSearchInput");
        QVERIFY(list && input);
        QTRY_VERIFY(input->hasActiveFocus());
        for (int step = 0; step < 7; ++step) key(host, Qt::Key_Down);
        QTRY_COMPARE(list->property("currentIndex").toInt(), 7);
        QSignalSpy frame(host, &QQuickWindow::frameSwapped);
        host->update();
        QTRY_VERIFY(!frame.isEmpty());
        QPointer<QQuickItem> row = qobject_cast<QQuickItem *>(list->property("currentItem").value<QObject *>());
        QVERIFY(row);
        key(host, Qt::Key_Tab);
        QTRY_VERIFY(host->activeFocusItem()
                    && host->activeFocusItem()->objectName() == "switcherActionButton");
        QCOMPARE(list->property("currentIndex").toInt(), 7);
        auto focused = host->activeFocusItem();
        while (focused && focused != row) focused = focused->parentItem();
        QVERIFY(focused == row);
        key(host, Qt::Key_Backtab, Qt::ShiftModifier);
        QTRY_VERIFY(input->hasActiveFocus());
        QCOMPARE(list->property("currentIndex").toInt(), 7);
    }
    void switcherPaging() {
        QFETCH(bool, secondary);
        auto host = secondary ? createChild("paging", false) : window;
        const QString ui = secondary ? "testWindow.browserWindowRegistry[1].ui"
                                     : "testWindow.browserUi";
        host->resize(390, 600);
        submitAt(host, ui, "switcher --scope commands");
        QTRY_VERIFY(evaluate("testWindow.switcherResultsCurrent() && "
                            "testWindow.switcherResults.length > 20").toBool());
        auto list = visual(host->contentItem(), "switcherResultList");
        auto input = visual(host->contentItem(), "switcherSearchInput");
        QVERIFY(list);
        QVERIFY(input);
        QTRY_VERIFY(input->hasActiveFocus());
        verifyPages(host, list, input);
    }
    void managerFormsKeyboard_data() {
        QTest::addColumn<bool>("secondary");
        QTest::newRow("primary") << false;
        QTest::newRow("secondary") << true;
    }
    void managerFormsKeyboard() {
        QFETCH(bool, secondary);
        auto host = secondary ? createChild("manager-forms", false) : window;
        QVERIFY(host);
        host->resize(390, 700);
        const QString ui = secondary ? "testWindow.browserWindowRegistry[1].ui" : "testWindow.browserUi";
        auto type = [host](const QByteArray &text) {
            for (const auto character : text) QTest::keyClick(host, character);
        };
        auto repeatReturn = [host] {
            QKeyEvent press(QEvent::KeyPress, Qt::Key_Return, Qt::NoModifier, "\r", true);
            QKeyEvent release(QEvent::KeyRelease, Qt::Key_Return, Qt::NoModifier, "\r", true);
            QCoreApplication::sendEvent(host, &press);
            QCoreApplication::sendEvent(host, &release);
        };
        submitAt(host, ui, "profiles");
        QTRY_VERIFY(!evaluate(ui + ".profile_values_pending").toBool());
        auto name = visual(host->contentItem(), "profileNameInput");
        auto label = visual(host->contentItem(), "profileLabelInput");
        QVERIFY(name && label);
        QTRY_COMPARE(host->activeFocusItem(), name);
        QSignalSpy ready(host, &QQuickWindow::frameSwapped);
        host->update();
        QTRY_VERIFY(!ready.isEmpty());
        type("keyboard-profile");
        repeatReturn();
        QCOMPARE(host->activeFocusItem(), name);
        key(host, Qt::Key_Return);
        QCOMPARE(lastKeyFocus.data(), name);
        QTRY_COMPARE(host->activeFocusItem(), label);
        type("keyboard profile");
        const auto beforeCreate = evaluate(ui + ".profile_action_id").toString();
        repeatReturn();
        QCOMPARE(evaluate(ui + ".profile_action_id").toString(), beforeCreate);
        key(host, Qt::Key_Return);
        QCOMPARE(lastKeyFocus.data(), label);
        QTRY_COMPARE(evaluate(ui + ".profile_action_state").toString(), QString("succeeded"));
        QTRY_VERIFY(evaluate(ui + ".profile_values").toString().contains("keyboard-profile\tkeyboard profile\t"));
        QTRY_COMPARE(name->property("text").toString(), QString());
        QCOMPARE(label->property("text").toString(), QString());
        QSignalSpy createdFrame(host, &QQuickWindow::frameSwapped);
        host->update();
        QTRY_VERIFY(!createdFrame.isEmpty());
        QCOMPARE(host->activeFocusItem(), name);
        if (secondary) QVERIFY(evaluate("testWindow.browserUi.profile_action_id").toString().isEmpty());

        auto atRename = [host] {
            auto item = host->activeFocusItem();
            if (!item || item->objectName() != "profileBeginRenameButton") return false;
            for (; item; item = item->parentItem())
                if (item->property("name").toString() == "keyboard-profile") return true;
            return false;
        };
        for (bool cancel : {true, false}) {
            for (int step = 0; step < 40 && !atRename(); ++step) key(host, Qt::Key_Tab);
            QVERIFY(atRename());
            key(host, Qt::Key_Return);
            auto rename = visual(host->contentItem(), "profileRenameInput");
            QVERIFY(rename);
            QTRY_COMPARE(host->activeFocusItem(), rename);
            if (cancel) {
                key(host, Qt::Key_Tab);
                QCOMPARE(host->activeFocusItem()->objectName(), QString("profileRenameButton"));
                key(host, Qt::Key_Tab);
                QCOMPARE(host->activeFocusItem()->property("text").toString(), QString("Cancel rename"));
                key(host, Qt::Key_Return);
            } else {
                key(host, Qt::Key_A, Qt::ControlModifier);
                type("renamed keyboard");
                const auto beforeRename = evaluate(ui + ".profile_action_id").toString();
                repeatReturn();
                QCOMPARE(evaluate(ui + ".profile_action_id").toString(), beforeRename);
                key(host, Qt::Key_Return);
                QCOMPARE(lastKeyFocus.data(), rename);
                QTRY_COMPARE(evaluate(ui + ".profile_action_state").toString(), QString("succeeded"));
                QTRY_VERIFY(evaluate(ui + ".profile_values").toString().contains("keyboard-profile\trenamed keyboard\t"));
            }
            QTRY_VERIFY(!rename->isVisible());
            QSignalSpy renamedFrame(host, &QQuickWindow::frameSwapped);
            host->update();
            QTRY_VERIFY(!renamedFrame.isEmpty());
            QVERIFY(host->activeFocusItem() && host->activeFocusItem()->isVisible()
                && host->activeFocusItem()->isEnabled()
                && host->activeFocusItem()->property("activeFocusOnTab").toBool());
        }
        for (int step = 0; step < 40 && (!host->activeFocusItem()
                || host->activeFocusItem()->objectName() != "profileCloseButton"); ++step)
            key(host, Qt::Key_Backtab, Qt::ShiftModifier);
        QVERIFY(host->activeFocusItem() && host->activeFocusItem()->objectName() == "profileCloseButton");
        key(host, Qt::Key_Return);
        submitAt(host, ui, "sessions");
        auto session = visual(host->contentItem(), "sessionNameInput");
        QVERIFY(session);
        QTRY_COMPARE(host->activeFocusItem(), session);
        QSignalSpy sessionFrame(host, &QQuickWindow::frameSwapped);
        host->update();
        QTRY_VERIFY(!sessionFrame.isEmpty());
        type("keyboard-session");
        QCOMPARE(session->property("text").toString(), QString("keyboard-session"));
        const auto beforeSave = evaluate(ui + ".session_save_id").toString();
        repeatReturn();
        QCOMPARE(evaluate(ui + ".session_save_id").toString(), beforeSave);
        key(host, Qt::Key_Return);
        QCOMPARE(lastKeyFocus.data(), session);
        QVERIFY(evaluate(ui + ".session_save_id").toString() != beforeSave);
        const auto saveState = [&] {
            return evaluate("JSON.stringify({id:" + ui + ".session_save_id,state:" + ui
                + ".session_save_state,message:" + ui + ".session_save_message,detail:" + ui
                + ".session_save_detail,names:" + ui + ".session_names_values,pending:" + ui
                + ".session_names_pending})").toString();
        };
        QTRY_VERIFY2(evaluate(ui + ".session_save_state").toString() == "succeeded", qPrintable(saveState()));
        QTRY_VERIFY2(evaluate(ui + ".session_names_values.indexOf('keyboard-session')>=0").toBool(), qPrintable(saveState()));
        QTRY_COMPARE(session->property("text").toString(), QString());
        QSignalSpy savedFrame(host, &QQuickWindow::frameSwapped);
        host->update();
        QTRY_VERIFY(!savedFrame.isEmpty());
        QCOMPARE(host->activeFocusItem(), session);
        if (secondary) QVERIFY(evaluate("testWindow.browserUi.session_save_id").toString().isEmpty());
        const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
        if (!artifacts.isEmpty()) QVERIFY(host->grabWindow().save(artifacts + "/manager-forms-"
            + (secondary ? "secondary" : "primary") + ".png"));
    }
    void favouriteEditorKeyboard_data() {
        QTest::addColumn<bool>("secondary");
        QTest::addColumn<bool>("privateProfile");
        QTest::newRow("primary") << false << false;
        QTest::newRow("secondary") << true << false;
        QTest::newRow("private") << true << true;
    }
    void favouriteEditorKeyboard() {
        QFETCH(bool, secondary);
        QFETCH(bool, privateProfile);
        auto host = secondary ? createChild(privateProfile ? "favourite-private" : "favourite-editor", privateProfile) : window;
        QVERIFY(host);
        host->resize(390, 700);
        const QString owner = secondary ? "testWindow.browserWindowRegistry[1]" : "testWindow";
        const QString ui = secondary ? owner + ".ui" : owner + ".browserUi";
        const QString hostExpression = secondary ? owner + ".host" : owner;
        const QString destination = QString("http://127.0.0.1:%1/favourite-saved").arg(server.serverPort());
        const auto type = [host](const QByteArray &value) { for (const auto character : value) QTest::keyClick(host, character); };
        const auto repeatReturn = [host] {
            QKeyEvent press(QEvent::KeyPress, Qt::Key_Return, Qt::NoModifier, "\r", true);
            QKeyEvent release(QEvent::KeyRelease, Qt::Key_Return, Qt::NoModifier, "\r", true);
            QCoreApplication::sendEvent(host, &press);
            QCoreApplication::sendEvent(host, &release);
        };
        submitAt(host, ui, "start-page-view favourites");
        QTRY_COMPARE(evaluate(ui + ".current_url").toString(), QString("rb://start"));
        QTRY_VERIFY(evaluate(ui + ".start_page_ready && !" + ui + ".start_page_busy").toBool());
        QTRY_VERIFY(visual(host->contentItem(), "startSiteTile1"));
        QTRY_COMPARE(host->activeFocusItem(), visual(host->contentItem(), "startSiteTile1"));
        QSignalSpy startFrame(host, &QQuickWindow::frameSwapped);
        host->update();QTRY_VERIFY(!startFrame.isEmpty());
        auto startPage = evaluate(hostExpression + ".activeWebView().startPage").value<QObject *>();
        QVERIFY(startPage);
        auto editor = startPage->findChild<QObject *>("ferricFavouriteEditor");
        QVERIFY(editor);
        key(host, Qt::Key_Return);
        QTRY_VERIFY(editor->property("visible").toBool());
        auto url = visual(host->contentItem(), "favouriteUrlInput");
        auto name = visual(host->contentItem(), "favouriteNameInput");
        QVERIFY(url && name);
        QTRY_COMPARE(host->activeFocusItem(), url);
        type(privateProfile ? QByteArray("not-a-url") : destination.toUtf8());
        repeatReturn();
        QCOMPARE(host->activeFocusItem(), url);
        QVERIFY(evaluate(ui + ".favourite_action_id").toString().isEmpty());
        key(host, Qt::Key_Return);
        QTRY_COMPARE(host->activeFocusItem(), name);
        QVERIFY(evaluate(ui + ".favourite_action_id").toString().isEmpty());
        type("Owned favourite");
        repeatReturn();
        QVERIFY(evaluate(ui + ".favourite_action_id").toString().isEmpty());
        key(host, Qt::Key_Return);
        if (privateProfile) {
            QTRY_COMPARE(evaluate(ui + ".favourite_action_state").toString(), QString("failed"));
            QVERIFY(editor->property("visible").toBool());
            QCOMPARE(host->activeFocusItem(), name);
            QCOMPARE(name->property("text").toString(), QString("Owned favourite"));
            key(host, Qt::Key_Backtab, Qt::ShiftModifier);
            QTRY_COMPARE(host->activeFocusItem(), url);
            key(host, Qt::Key_A, Qt::ControlModifier);type(destination.toUtf8());
            key(host, Qt::Key_Return);QTRY_COMPARE(host->activeFocusItem(), name);
            key(host, Qt::Key_Return);
        }
        QTRY_COMPARE(evaluate(ui + ".favourite_action_state").toString(), QString("succeeded"));
        QTRY_VERIFY(!editor->property("visible").toBool());
        QTRY_COMPARE(host->activeFocusItem(), visual(host->contentItem(), "startSiteTile1"));

        const auto page = hostExpression + ".activeWebView().startPage";
        const auto bookmarkId = evaluate(page + ".favouriteTiles[0].id").toString();
        QVERIFY(!bookmarkId.isEmpty());
        for (const auto chooseBookmark : {false, true}) {
            const auto slot = chooseBookmark ? 1 : 2;
            QSignalSpy ready(host, &QQuickWindow::frameSwapped);
            host->update();QTRY_VERIFY(!ready.isEmpty());
            key(host, chooseBookmark ? Qt::Key_Up : Qt::Key_Down);
            QTRY_COMPARE(host->activeFocusItem(), visual(host->contentItem(), "startSiteTile" + QString::number(slot)));
            key(host, Qt::Key_Return);QTRY_VERIFY(editor->property("visible").toBool());
            QTRY_COMPARE(host->activeFocusItem(), url);
            if (chooseBookmark) {
                auto picker = visual(host->contentItem(), "favouriteBookmarkPicker");
                QVERIFY(picker);
                key(host, Qt::Key_Backtab, Qt::ShiftModifier);QTRY_COMPARE(host->activeFocusItem(), picker);
                key(host, Qt::Key_Down);QTRY_COMPARE(picker->property("currentIndex").toInt(), 1);
                QTRY_COMPARE(startPage->property("editBookmarkId").toString(), bookmarkId);
                QCOMPARE(url->property("text").toString(), destination);
                key(host, Qt::Key_Tab);QTRY_COMPARE(host->activeFocusItem(), url);
            } else type(destination.toUtf8());
            key(host, Qt::Key_Return);QTRY_COMPARE(host->activeFocusItem(), name);
            key(host, Qt::Key_A, Qt::ControlModifier);
            const QString label = chooseBookmark ? "Chosen bookmark name" : "Renamed existing favourite";
            type(label.toUtf8());
            const auto before = evaluate(ui + ".favourite_action_id").toString();
            key(host, Qt::Key_Return);
            QTRY_COMPARE(evaluate(ui + ".favourite_action_state").toString(), QString("succeeded"));
            QVERIFY(evaluate(ui + ".favourite_action_id").toString() != before);
            QTRY_VERIFY(!editor->property("visible").toBool());
            QTRY_COMPARE(host->activeFocusItem(), visual(host->contentItem(), "startSiteTile" + QString::number(slot)));
            const auto tile = page + ".favouriteTiles[" + QString::number(slot - 1) + "]";
            QCOMPARE(evaluate(tile + ".id").toString(), bookmarkId);
            QCOMPARE(evaluate(tile + ".title").toString(), label);
            QCOMPARE(evaluate(page + ".bookmarks.length").toInt(), 1);
            QCOMPARE(evaluate(page + ".bookmarks[0].title").toString(), label);
            QVERIFY(evaluate(page + ".favouriteTiles[" + QString::number(2 - slot) + "] === null").toBool());
        }

        const auto openEdit = [&] {
            auto options = visual(host->contentItem(), "startSiteOptions1");
            QVERIFY(options);
            QSignalSpy presented(host, &QQuickWindow::frameSwapped);
            host->update();QTRY_VERIFY(!presented.isEmpty());
            QVERIFY(options->isEnabled() && options->width() > 0 && options->height() > 0);
            key(host, Qt::Key_Tab);QTRY_COMPARE(host->activeFocusItem(), options);
            key(host, Qt::Key_Space);
            QQmlExpression menuExpression(qmlContext(options), options, "tileMenu");
            auto menu = menuExpression.evaluate().value<QObject *>();
            QVERIFY2(!menuExpression.hasError(), qPrintable(menuExpression.error().toString()));
            QVERIFY(menu);QTRY_VERIFY(menu->property("visible").toBool());
            const auto currentText = [menu] {
                QQuickItem *item = nullptr;
                QMetaObject::invokeMethod(menu, "itemAt", Q_RETURN_ARG(QQuickItem *, item),
                    Q_ARG(int, menu->property("currentIndex").toInt()));
                return item ? item->property("text").toString() : QString();
            };
            for (int step = 0; step < 8 && currentText() != "Edit bookmark"; ++step) key(host, Qt::Key_Down);
            QCOMPARE(currentText(), QString("Edit bookmark"));
            key(host, Qt::Key_Return);
            QTRY_VERIFY(editor->property("visible").toBool());
            QTRY_COMPARE(host->activeFocusItem(), url);
            key(host, Qt::Key_Return);QTRY_COMPARE(host->activeFocusItem(), name);
        };
        if (!privateProfile) {
            openEdit();
            if (QTest::currentTestFailed()) return;
            key(host, Qt::Key_A, Qt::ControlModifier);type("Submitted favourite");
            QList<sqlite3 *> databases;
            const auto closeDatabases = qScopeGuard([&] { for (auto database : databases) { sqlite3_exec(database, "ROLLBACK", nullptr, nullptr, nullptr); sqlite3_close(database); } });
            QDirIterator files(QFileInfo(configPath).absolutePath() + "/data", {"browser.sqlite"}, QDir::Files, QDirIterator::Subdirectories);
            while (files.hasNext()) {
                sqlite3 *database = nullptr;
                QCOMPARE(sqlite3_open_v2(QFile::encodeName(files.next()).constData(), &database, SQLITE_OPEN_READWRITE, nullptr), SQLITE_OK);
                databases.push_back(database);sqlite3_busy_timeout(database, 2000);
                QCOMPARE(sqlite3_exec(database, "BEGIN IMMEDIATE", nullptr, nullptr, nullptr), SQLITE_OK);
            }
            QVERIFY(!databases.isEmpty());
            const auto revision = evaluate(ui + ".start_page_revision").toInt();
            // Queue an older read and submit this real field in one event-loop
            // turn. The held SQLite writer makes their completion order visible.
            evaluate("(function(){" + ui + ".refresh_start_page_metadata();" + hostExpression + ".activeFocusItem.accepted()})()");
            const auto pendingId = evaluate(ui + ".favourite_action_id").toString();
            QCOMPARE(evaluate(ui + ".favourite_action_state").toString(), QString("pending"));
            QTRY_VERIFY_WITH_TIMEOUT(evaluate(ui + ".start_page_revision").toInt() > revision, 1500);
            QCOMPARE(evaluate(ui + ".favourite_action_state").toString(), QString("pending"));
            QVERIFY(editor->property("visible").toBool());
            repeatReturn();QCOMPARE(evaluate(ui + ".favourite_action_id").toString(), pendingId);
            key(host, Qt::Key_A, Qt::ControlModifier);type("Newer favourite");
            for (auto database : databases) QCOMPARE(sqlite3_exec(database, "ROLLBACK", nullptr, nullptr, nullptr), SQLITE_OK);
            QTRY_COMPARE(evaluate(ui + ".favourite_action_state").toString(), QString("succeeded"));
            QVERIFY(editor->property("visible").toBool());
            QCOMPARE(name->property("text").toString(), QString("Newer favourite"));
            QVERIFY(visual(host->contentItem(), "favouriteNewerEditNotice"));
            QTRY_COMPARE(host->activeFocusItem(), name);
            key(host, Qt::Key_Return);
            QTRY_COMPARE(evaluate(ui + ".favourite_action_state").toString(), QString("succeeded"));
            QVERIFY(evaluate(ui + ".favourite_action_id").toString() != pendingId);
            QTRY_VERIFY(!editor->property("visible").toBool());
            QTRY_COMPARE(host->activeFocusItem(), visual(host->contentItem(), "startSiteTile1"));
            QTRY_VERIFY(evaluate(ui + ".start_page_titles.indexOf('Newer favourite') >= 0").toBool());

            openEdit();
            if (QTest::currentTestFailed()) return;
            key(host, Qt::Key_A, Qt::ControlModifier);type("Retry favourite");
            for (auto database : databases) QCOMPARE(sqlite3_exec(database,
                "CREATE TRIGGER owned_favourite_failure BEFORE UPDATE ON bookmarks BEGIN SELECT RAISE(ABORT,'Owned favourite write failure'); END", nullptr, nullptr, nullptr), SQLITE_OK);
            key(host, Qt::Key_Return);
            QTRY_COMPARE(evaluate(ui + ".favourite_action_state").toString(), QString("failed"));
            const auto failedId = evaluate(ui + ".favourite_action_id").toString();
            QVERIFY(evaluate(ui + ".favourite_action_detail").toString().contains("Owned favourite write failure"));
            QVERIFY(editor->property("visible").toBool());
            QCOMPARE(name->property("text").toString(), QString("Retry favourite"));
            auto detailsButton = visual(host->contentItem(), "managerFeedbackDetailsButton");
            QVERIFY(detailsButton);
            for (int step = 0; step < 12 && host->activeFocusItem() != detailsButton; ++step) key(host, Qt::Key_Tab);
            QTRY_COMPARE(host->activeFocusItem(), detailsButton);
            QTRY_VERIFY(detailsButton->width() > 0 && detailsButton->height() > 0);
            key(host, Qt::Key_Return);
            QTRY_VERIFY(visual(host->contentItem(), "managerFeedbackDetails"));
            key(host, Qt::Key_Tab);
            auto details = visual(host->contentItem(), "managerFeedbackDetails");
            QTRY_COMPARE(host->activeFocusItem(), details);
            key(host, Qt::Key_End, Qt::ControlModifier);
            QTRY_COMPARE(details->property("cursorPosition").toInt(), int(details->property("text").toString().size()));
            captureChrome(host, details, "favourite-failure-" + QString(secondary ? "secondary" : "primary"));
            for (auto database : databases) QCOMPARE(sqlite3_exec(database, "DROP TRIGGER owned_favourite_failure", nullptr, nullptr, nullptr), SQLITE_OK);
            for (int step = 0; step < 12 && host->activeFocusItem() != name; ++step) key(host, Qt::Key_Backtab, Qt::ShiftModifier);
            QTRY_COMPARE(host->activeFocusItem(), name);key(host, Qt::Key_Return);
            QTRY_COMPARE(evaluate(ui + ".favourite_action_state").toString(), QString("succeeded"));
            QVERIFY(evaluate(ui + ".favourite_action_id").toString() != failedId);
            QTRY_VERIFY(!editor->property("visible").toBool());
        }
        if (secondary) QVERIFY(evaluate("testWindow.browserUi.favourite_action_id").toString().isEmpty());
        QTRY_COMPARE(host->activeFocusItem(), visual(host->contentItem(), "startSiteTile1"));
        key(host, Qt::Key_Return);
        QTRY_COMPARE(evaluate(ui + ".current_url").toString(), destination);
        QTRY_COMPARE(documentUrl(hostExpression + ".activeWebView()"), destination);
    }
    void favouriteRequestsKeyboard_data() {
        QTest::addColumn<bool>("secondary");
        QTest::addColumn<bool>("privateProfile");
        QTest::newRow("primary") << false << false;
        QTest::newRow("secondary") << true << false;
        QTest::newRow("private") << true << true;
    }
    void favouriteRequestsKeyboard() {
        QFETCH(bool, secondary);
        QFETCH(bool, privateProfile);
        auto host = secondary ? createChild(privateProfile ? "requests-private" : "requests-secondary", privateProfile) : window;
        QVERIFY(host);host->resize(390, 700);
        const QString owner = secondary ? "testWindow.browserWindowRegistry[1]" : "testWindow";
        const QString ui = secondary ? owner + ".ui" : owner + ".browserUi";
        const QString hostExpression = secondary ? owner + ".host" : owner;
        const QString page = hostExpression + ".activeWebView().startPage";
        const auto origin = QString("http://127.0.0.1:%1/").arg(server.serverPort());
        const QStringList addresses{origin + "queued-a", origin + "queued-b", origin + "draft-c"};
        const auto type = [host](const QString &value) { for (const auto character : value.toUtf8()) QTest::keyClick(host, character); };
        const auto present = [host] { QSignalSpy frame(host, &QQuickWindow::frameSwapped);host->update();QTRY_VERIFY(!frame.isEmpty()); };
        submitAt(host, ui, "start-page-view favourites");
        QTRY_COMPARE(evaluate(ui + ".start_page_view").toString(), QString("favourites"));
        QTRY_VERIFY(evaluate(ui + ".start_page_ready && !" + ui + ".start_page_busy").toBool());
        for (int index = 0; index < 3; ++index) {
            QVERIFY2(evaluate(ui + ".save_favourite(" + QString::number(index + 1) + "," + quoted(addresses[index])
                + "," + quoted("Original " + QString(QChar('A' + index))) + ",\"\")").toBool(), qPrintable(evaluate(ui + ".start_page_error").toString()));
            QTRY_COMPARE(evaluate(ui + ".favourite_action_state").toString(), QString("succeeded"));
        }
        if (!privateProfile) {
            QTRY_VERIFY(!evaluate(ui + ".library_pending").toBool());
            evaluate(ui + ".refresh_library()");
            QTRY_VERIFY(!evaluate(ui + ".library_pending").toBool());
        }
        const auto firstId = evaluate(page + ".favouriteTiles[0].id").toString();
        const auto secondId = evaluate(page + ".favouriteTiles[1].id").toString();
        const auto currentId = evaluate(page + ".favouriteTiles[2].id").toString();
        QVERIFY(!firstId.isEmpty() && !secondId.isEmpty() && !currentId.isEmpty());
        QTRY_COMPARE(host->activeFocusItem(), visual(host->contentItem(), "startSiteTile1"));
        key(host, Qt::Key_Down);key(host, Qt::Key_Down);
        QTRY_COMPARE(host->activeFocusItem(), visual(host->contentItem(), "startSiteTile3"));
        present();key(host, Qt::Key_Tab);
        auto options = visual(host->contentItem(), "startSiteOptions3");
        QTRY_COMPARE(host->activeFocusItem(), options);key(host, Qt::Key_Space);
        QQmlExpression menuExpression(qmlContext(options), options, "tileMenu");
        auto menu = menuExpression.evaluate().value<QObject *>();
        QVERIFY(menu && !menuExpression.hasError());QTRY_VERIFY(menu->property("visible").toBool());
        const auto currentText = [menu] {
            QQuickItem *item = nullptr;
            QMetaObject::invokeMethod(menu, "itemAt", Q_RETURN_ARG(QQuickItem *, item), Q_ARG(int, menu->property("currentIndex").toInt()));
            return item ? item->property("text").toString() : QString();
        };
        for (int step = 0; step < 8 && currentText() != "Edit bookmark"; ++step) key(host, Qt::Key_Down);
        QCOMPARE(currentText(), QString("Edit bookmark"));key(host, Qt::Key_Return);
        auto startPage = evaluate(page).value<QObject *>();QVERIFY(startPage);
        auto editor = startPage->findChild<QObject *>("ferricFavouriteEditor");QVERIFY(editor);
        QTRY_VERIFY(editor->property("visible").toBool());
        auto url = visual(host->contentItem(), "favouriteUrlInput");
        auto name = visual(host->contentItem(), "favouriteNameInput");QVERIFY(url && name);
        QTRY_COMPARE(host->activeFocusItem(), url);key(host, Qt::Key_Return);
        QTRY_COMPARE(host->activeFocusItem(), name);key(host, Qt::Key_A, Qt::ControlModifier);type("Submitted C");
        const auto capture = [&](const QString &id) {
            QVERIFY2(evaluate(ui + ".start_page_pick_bookmark(" + quoted(id) + ")").toBool(),
                qPrintable(evaluate(ui + ".status_text").toString()));
            evaluate("testWindow.completeBrowserKeyAction(" + ui + "," + hostExpression + ")");
        };
        key(host, Qt::Key_A, Qt::ControlModifier);
        capture(secondId);if (QTest::currentTestFailed()) return;
        QTRY_COMPARE(evaluate(page + ".queuedFavouriteCaptures.length").toInt(), 1);
        QCOMPARE(evaluate(page + ".queuedFavouriteCaptures[0].bookmark").toString(), secondId);
        QCOMPARE(url->property("text").toString(), addresses[2]);
        QCOMPARE(name->property("text").toString(), QString("Submitted C"));
        QCOMPARE(name->property("selectionStart").toInt(), 0);
        QCOMPARE(name->property("selectionEnd").toInt(), int(QString("Submitted C").size()));
        QCOMPARE(startPage->property("editingAction").toString(), QString("edit"));
        QVERIFY(startPage->property("editBookmarkId").toString().isEmpty());
        QTRY_COMPARE(host->activeFocusItem(), name);
        QList<sqlite3 *> databases;
        const auto closeDatabases = qScopeGuard([&] { for (auto database : databases) { sqlite3_exec(database, "ROLLBACK", nullptr, nullptr, nullptr);sqlite3_close(database); } });
        if (!privateProfile) {
            QDirIterator files(QFileInfo(configPath).absolutePath() + "/data", {"browser.sqlite"}, QDir::Files, QDirIterator::Subdirectories);
            while (files.hasNext()) {
                sqlite3 *database = nullptr;
                QCOMPARE(sqlite3_open_v2(QFile::encodeName(files.next()).constData(), &database, SQLITE_OPEN_READWRITE, nullptr), SQLITE_OK);
                databases.push_back(database);sqlite3_busy_timeout(database, 2000);
                QCOMPARE(sqlite3_exec(database, "BEGIN IMMEDIATE", nullptr, nullptr, nullptr), SQLITE_OK);
            }
            QVERIFY(!databases.isEmpty());
            key(host, Qt::Key_Return);
            QTRY_COMPARE(evaluate(ui + ".favourite_action_state").toString(), QString("pending"));
        }
        capture(firstId);if (QTest::currentTestFailed()) return;
        QTRY_COMPARE_WITH_TIMEOUT(evaluate(page + ".queuedFavouriteCaptures.length").toInt(), 2, 1500);
        QCOMPARE(name->property("text").toString(), QString("Submitted C"));
        QTRY_COMPARE(host->activeFocusItem(), name);
        if (!privateProfile) {
            key(host, Qt::Key_A, Qt::ControlModifier);type("Newer C");
            for (auto database : databases) QCOMPARE(sqlite3_exec(database, "ROLLBACK", nullptr, nullptr, nullptr), SQLITE_OK);
        } else key(host, Qt::Key_Return);
        QTRY_COMPARE(evaluate(ui + ".favourite_action_state").toString(), QString("succeeded"));
        const auto cancel = [&] {
            auto button = visual(host->contentItem(), "cancelFavouriteEdit");QVERIFY(button);
            for (int step = 0; step < 32 && host->activeFocusItem() != button; ++step) key(host, Qt::Key_Tab);
            QTRY_COMPARE(host->activeFocusItem(), button);key(host, Qt::Key_Return);
            QTRY_VERIFY(!editor->property("visible").toBool());present();
        };
        if (!privateProfile) {
            QVERIFY(editor->property("visible").toBool());
            QCOMPARE(name->property("text").toString(), QString("Newer C"));
            auto review = visual(host->contentItem(), "reviewQueuedFavourite");QVERIFY(review && !review->isEnabled());
            cancel();if (QTest::currentTestFailed()) return;
        } else { QTRY_VERIFY(!editor->property("visible").toBool());present(); }
        auto review = visual(host->contentItem(), "reviewQueuedFavourite");QVERIFY(review);
        QTRY_COMPARE(host->activeFocusItem(), review);key(host, Qt::Key_Return);
        QTRY_VERIFY(editor->property("visible").toBool());
        QTRY_COMPARE(host->activeFocusItem(), url);
        QCOMPARE(url->property("text").toString(), addresses[1]);
        QCOMPARE(startPage->property("editBookmarkId").toString(), secondId);
        QCOMPARE(evaluate(page + ".queuedFavouriteCaptures.length").toInt(), 1);
        key(host, Qt::Key_Return);key(host, Qt::Key_A, Qt::ControlModifier);type("Reviewed B");
        const auto updated = origin + "queued-b-updated";
        QVERIFY(evaluate(ui + ".start_page_action('edit',2," + quoted(updated) + ",'Original B')").toBool());
        QTRY_COMPARE(evaluate(ui + ".favourite_action_state").toString(), QString("succeeded"));
        QTRY_VERIFY(startPage->property("favouriteBookmarkStale").toBool());
        auto currentAddress = visual(host->contentItem(), "updatedFavouriteAddress");QVERIFY(currentAddress);
        QCOMPARE(currentAddress->property("text").toString(), updated);
        QVERIFY(currentAddress->property("readOnly").toBool());
        QCOMPARE(url->property("text").toString(), addresses[1]);
        QCOMPARE(name->property("text").toString(), QString("Reviewed B"));
        QVERIFY(!visual(host->contentItem(), "saveFavouriteEdit")->isEnabled());
        const auto staleReceipt = evaluate(ui + ".favourite_action_id").toString();
        // Exercise the storage identity guard independently of the form's
        // stale-address decision, using exactly the address it still displays.
        evaluate(ui + ".save_favourite(4," + quoted(addresses[1]) + ",'Must not overwrite'," + quoted(secondId) + ")");
        QTRY_COMPARE(evaluate(ui + ".favourite_action_state").toString(), QString("failed"));
        QVERIFY(evaluate(ui + ".favourite_action_id").toString() != staleReceipt);
        QVERIFY(!evaluate(page + ".bookmarks.some(function(bookmark){return bookmark.title==='Must not overwrite'})").toBool());
        key(host, Qt::Key_Return);
        auto updatedChoice = visual(host->contentItem(), "useUpdatedFavouriteAddress");QVERIFY(updatedChoice);
        QTRY_COMPARE(host->activeFocusItem(), updatedChoice);
        captureChrome(host, updatedChoice, "favourite-address-choice-" + QString(privateProfile ? "private" : secondary ? "secondary" : "primary"));
        key(host, Qt::Key_Return);QTRY_COMPARE(host->activeFocusItem(), url);
        QCOMPARE(url->property("text").toString(), updated);
        QCOMPARE(name->property("text").toString(), QString("Reviewed B"));
        key(host, Qt::Key_Return);QTRY_COMPARE(host->activeFocusItem(), name);key(host, Qt::Key_Return);
        QTRY_COMPARE(evaluate(ui + ".favourite_action_state").toString(), QString("succeeded"));
        QTRY_VERIFY(!editor->property("visible").toBool());present();
        QTRY_COMPARE(host->activeFocusItem(), review);key(host, Qt::Key_Return);
        QTRY_VERIFY(editor->property("visible").toBool());QTRY_COMPARE(host->activeFocusItem(), url);
        QCOMPARE(url->property("text").toString(), addresses[0]);
        QCOMPARE(startPage->property("editBookmarkId").toString(), firstId);
        QCOMPARE(evaluate(page + ".queuedFavouriteCaptures.length").toInt(), 0);
        cancel();if (QTest::currentTestFailed()) return;
        QVERIFY(!startPage->property("closeNeedsConfirmation").toBool());
        QCOMPARE(evaluate(page + ".bookmarks.length").toInt(), 3);
        QCOMPARE(evaluate(page + ".bookmarks.find(function(bookmark){return bookmark.id===" + quoted(firstId) + "}).title").toString(), QString("Original A"));
        QCOMPARE(evaluate(page + ".bookmarks.find(function(bookmark){return bookmark.id===" + quoted(secondId) + "}).title").toString(), QString("Reviewed B"));
        QCOMPARE(evaluate(page + ".bookmarks.find(function(bookmark){return bookmark.id===" + quoted(currentId) + "}).title").toString(), QString("Submitted C"));
        if (secondary) QVERIFY(evaluate("testWindow.browserUi.favourite_action_id").toString().isEmpty());
    }
    void settingsDrafts_data() {
        QTest::addColumn<bool>("secondary");
        QTest::newRow("primary") << false;
        QTest::newRow("secondary") << true;
    }
    void settingsDrafts() {
        QFETCH(bool, secondary);
        auto host = secondary ? createChild("settings-drafts", false) : window;
        QVERIFY(host);
        host->resize(390, 700);
        const QString ui = secondary ? "testWindow.browserWindowRegistry[1].ui" : "testWindow.browserUi";
        const auto type = [host](const QByteArray &text) {
            for (const auto character : text) QTest::keyClick(host, character);
        };
        submitAt(host, ui, "settings");
        QTRY_VERIFY(visual(host->contentItem(), "settingsSearchInput")
            && visual(host->contentItem(), "settingsList"));
        auto search = visual(host->contentItem(), "settingsSearchInput");
        auto list = visual(host->contentItem(), "settingsList");
        QVERIFY(search && list);
        const auto field = [list] {
            QQuickItem *row = nullptr;
            if (!QMetaObject::invokeMethod(list, "itemAtIndex", Q_RETURN_ARG(QQuickItem *, row),
                                           Q_ARG(int, 0))) return static_cast<QQuickItem *>(nullptr);
            if (!row) return static_cast<QQuickItem *>(nullptr);
            const auto data = row->property("rowData");
            const auto object = data.value<QJSValue>();
            const auto key = object.isObject() ? object.property("key").toString()
                : data.toMap().value("key").toString();
            return key == "ui.font_size_pt" ? visual(row, "settingTextEditor") : nullptr;
        };
        QTRY_COMPARE(host->activeFocusItem(), search);
        QSignalSpy ready(host, &QQuickWindow::frameSwapped);
        host->update();
        QTRY_VERIFY(!ready.isEmpty());
        key(host, Qt::Key_A, Qt::ControlModifier);
        type("ui.font_size_pt");
        QCOMPARE(search->property("text").toString(), QString("ui.font_size_pt"));
        QTRY_COMPARE(list->property("count").toInt(), 1);
        QTRY_VERIFY(field());
        QPointer<QQuickItem> editor = field();
        QVERIFY(editor);
        for (int step = 0; step < 6 && host->activeFocusItem() != editor; ++step) key(host, Qt::Key_Tab);
        QTRY_COMPARE(host->activeFocusItem(), editor.data());
        key(host, Qt::Key_A, Qt::ControlModifier);
        type("21");
        QCOMPARE(editor->property("text").toString(), QString("21"));
        for (int step = 0; step < 4 && host->activeFocusItem() != search; ++step)
            key(host, Qt::Key_Backtab, Qt::ShiftModifier);
        QTRY_COMPARE(host->activeFocusItem(), search);
        key(host, Qt::Key_A, Qt::ControlModifier);
        type("missing");
        QTRY_COMPARE(list->property("count").toInt(), 0);
        key(host, Qt::Key_A, Qt::ControlModifier);
        type("ui.font_size_pt");
        QCOMPARE(search->property("text").toString(), QString("ui.font_size_pt"));
        QTRY_COMPARE(list->property("count").toInt(), 1);
        QTRY_VERIFY(field());
        editor = field();
        QCOMPARE(editor->property("text").toString(), QString("21"));
        QSignalSpy filteredFrame(host, &QQuickWindow::frameSwapped);
        host->update();
        QTRY_VERIFY(!filteredFrame.isEmpty());
        for (int step = 0; step < 6 && host->activeFocusItem() != editor; ++step) key(host, Qt::Key_Tab);
        QTRY_COMPARE(host->activeFocusItem(), editor.data());
        QKeyEvent repeatedPress(QEvent::KeyPress, Qt::Key_Return, Qt::NoModifier, "\r", true);
        QKeyEvent repeatedRelease(QEvent::KeyRelease, Qt::Key_Return, Qt::NoModifier, "\r", true);
        QCoreApplication::sendEvent(host, &repeatedPress);
        QCoreApplication::sendEvent(host, &repeatedRelease);
        QCOMPARE(evaluate(ui + ".chrome_font_size_pt").toDouble(), 20.0);
        const auto submitWhenReady = [&](const QString &draft) {
            QElapsedTimer ready;
            ready.start();
            do {
                key(host, Qt::Key_Return);
                if (evaluate(ui + ".runtime_setting_error").toString()
                        != "Settings are being reloaded. Try again in a moment.") return;
                // Startup and changes to the watched override path can both
                // reserve configuration. Check the visible rejection and
                // retained edit before issuing a fresh keyboard retry.
                QCOMPARE(evaluate(ui + ".setting_action_state").toString(), QString("failed"));
                QVERIFY(editor);
                QCOMPARE(editor->property("text").toString(), draft);
                auto message = visual(host->contentItem(), "managerFeedbackMessage");
                QVERIFY(message);
                QCOMPARE(message->property("text").toString(), evaluate(ui + ".setting_action_message").toString());
                qInfo() << "Owned Settings submission waits for the watched configuration reload";
                QTest::qWait(20);
            } while (ready.elapsed() < 5000);
        };
        submitWhenReady("21");
        QTRY_COMPARE(evaluate(ui + ".chrome_font_size_pt").toDouble(), 21.0);
        QTRY_VERIFY(editor && editor->property("text").toString().toDouble() == 21.0);
        auto notice = [&] { return visual(host->contentItem(), "settingDraftNotice"); };
        QTRY_VERIFY(!notice());
        QTRY_COMPARE(host->activeFocusItem(), editor.data());
        if (secondary) QTRY_COMPARE(evaluate("testWindow.browserUi.chrome_font_size_pt").toDouble(), 21.0);

        key(host, Qt::Key_A, Qt::ControlModifier);
        type("24");
        auto refresh = visual(host->contentItem(), "settingsRefreshButton");
        QVERIFY(refresh);
        for (int step = 0; step < 8 && host->activeFocusItem() != refresh; ++step) key(host, Qt::Key_Tab);
        QTRY_COMPARE(host->activeFocusItem(), refresh);
        key(host, Qt::Key_Return);
        QSignalSpy refreshed(host, &QQuickWindow::frameSwapped);
        host->update();
        QTRY_VERIFY(!refreshed.isEmpty());
        QVERIFY(editor);
        QCOMPARE(editor->property("text").toString(), QString("24"));
        QCOMPARE(host->activeFocusItem(), refresh);
        key(host, Qt::Key_Backtab, Qt::ShiftModifier);
        QTRY_VERIFY(host->activeFocusItem() && host->activeFocusItem()->objectName() == "settingDiscardButton");
        key(host, Qt::Key_Return);
        QTRY_COMPARE(host->activeFocusItem(), editor.data());
        QCOMPARE(editor->property("text").toString().toDouble(), 21.0);
        QVERIFY(!notice());
        key(host, Qt::Key_A, Qt::ControlModifier);
        type("24");
        QTRY_VERIFY(notice());
        QCOMPARE(editor->property("text").toString(), QString("24"));
        QSignalSpy draftedFrame(host, &QQuickWindow::frameSwapped);
        host->update();
        QTRY_VERIFY(!draftedFrame.isEmpty());
        captureChrome(host, editor.data(), "settings-drafts-" + QString(secondary ? "secondary" : "primary"));
        auto close = visual(host->contentItem(), "settingsCloseButton");
        QVERIFY(close);
        for (int step = 0; step < 8 && host->activeFocusItem() != close; ++step) key(host, Qt::Key_Tab);
        QTRY_COMPARE(host->activeFocusItem(), close);
        key(host, Qt::Key_Return);
        QTRY_VERIFY(!list->isVisible());
        submitAt(host, ui, "settings");
        QTRY_VERIFY(list->isVisible());
        QTRY_COMPARE(host->activeFocusItem(), search);
        key(host, Qt::Key_A, Qt::ControlModifier);
        type("ui.font_size_pt");
        QTRY_COMPARE(list->property("count").toInt(), 1);
        QTRY_VERIFY(field());
        QCOMPARE(field()->property("text").toString(), QString("24"));

        // Reject a real durable write in this fixture's profile, then retry
        // the preserved edit through the same keyboard control.
        QTRY_COMPARE(evaluate(ui + ".setting_action_state").toString(), QString("succeeded"));
        QVERIFY(evaluate(ui + ".setting_action_message").toString().contains("all regular profiles in this setup"));
        const auto savedId = evaluate(ui + ".setting_action_id").toString();
        QVERIFY(!savedId.isEmpty());
        QDirIterator overrides(QFileInfo(configPath).absolutePath() + "/data",
            {"runtime-overrides.toml"}, QDir::Files, QDirIterator::Subdirectories);
        QVERIFY(overrides.hasNext());
        const auto overridePath = overrides.next();
        QVERIFY(!overrides.hasNext());
        QVERIFY(QFile::remove(overridePath));
        QVERIFY(QDir().mkdir(overridePath));
        editor = field();
        for (int step = 0; step < 6 && host->activeFocusItem() != editor; ++step) key(host, Qt::Key_Tab);
        QTRY_COMPARE(host->activeFocusItem(), editor.data());
        submitWhenReady("24");
        QTRY_COMPARE(evaluate(ui + ".setting_action_state").toString(), QString("failed"));
        const auto failedId = evaluate(ui + ".setting_action_id").toString();
        QVERIFY(failedId != savedId);
        QVERIFY(!evaluate(ui + ".setting_action_detail").toString().isEmpty());
        QVERIFY2(evaluate(ui + ".setting_action_detail").toString().contains("runtime-overrides.toml"),
            qPrintable(evaluate(ui + ".setting_action_detail").toString()));
        QCOMPARE(evaluate(ui + ".chrome_font_size_pt").toDouble(), 21.0);
        QCOMPARE(editor->property("text").toString(), QString("24"));
        QVERIFY(notice());
        auto feedback = visual(host->contentItem(), "managerFeedbackMessage");
        QVERIFY(feedback);
        QCOMPARE(feedback->property("text").toString(), evaluate(ui + ".setting_action_message").toString());
        auto detailsButton = visual(host->contentItem(), "managerFeedbackDetailsButton");
        QVERIFY(detailsButton);
        for (int step = 0; step < 10 && host->activeFocusItem() != detailsButton; ++step) key(host, Qt::Key_Tab);
        QTRY_COMPARE(host->activeFocusItem(), detailsButton);
        key(host, Qt::Key_Return);
        QTRY_VERIFY(visual(host->contentItem(), "managerFeedbackDetails"));
        auto details = visual(host->contentItem(), "managerFeedbackDetails");
        QCOMPARE(details->property("text").toString(), evaluate(ui + ".setting_action_detail").toString());
        key(host, Qt::Key_Tab);
        QTRY_COMPARE(host->activeFocusItem(), details);
        key(host, Qt::Key_End, Qt::ControlModifier);
        QTRY_COMPARE(details->property("cursorPosition").toInt(), int(details->property("text").toString().size()));
        auto feedbackViewport = visual(host->contentItem(), "settingsFeedbackViewport");
        QVERIFY(feedbackViewport);
        QTRY_VERIFY(([&] {
            const auto caret = details->mapRectToItem(feedbackViewport, details->property("cursorRectangle").toRectF());
            return caret.top() >= 0 && caret.bottom() <= feedbackViewport->height();
        })());
        captureChrome(host, details, "settings-save-failure-" + QString(secondary ? "secondary" : "primary"));
        QVERIFY(QDir().rmdir(overridePath));
        for (int step = 0; step < 12 && host->activeFocusItem() != editor; ++step)
            key(host, Qt::Key_Backtab, Qt::ShiftModifier);
        QTRY_COMPARE(host->activeFocusItem(), editor.data());
        submitWhenReady("24");
        QTRY_VERIFY2(evaluate(ui + ".setting_action_state").toString() == "succeeded",
            qPrintable(evaluate(ui + ".setting_action_state + ': ' + " + ui + ".setting_action_detail + ' / ' + "
                                + ui + ".runtime_setting_error").toString()));
        QVERIFY(evaluate(ui + ".setting_action_id").toString() != failedId);
        QCOMPARE(evaluate(ui + ".chrome_font_size_pt").toDouble(), 24.0);
        QTRY_VERIFY(!notice());
        QTRY_VERIFY(QFileInfo(overridePath).isFile());
        if (secondary) QTRY_COMPARE(evaluate("testWindow.browserUi.chrome_font_size_pt").toDouble(), 24.0);
    }
    void downloadPaging_data() {
        QTest::addColumn<bool>("secondary");
        QTest::newRow("primary") << false;
        QTest::newRow("secondary") << true;
    }
    void downloadPaging() {
        QFETCH(bool, secondary);
        auto host = secondary ? createChild("download-keyboard", false) : window;
        QVERIFY(host);
        host->resize(390, 700);
        const QString ui = secondary ? "testWindow.browserWindowRegistry[1].ui" : "testWindow.browserUi";
        // The catalog reads real durable rows and checks real owned files.
        // Seed only this test's temporary profiles, never its visual model.
        QDirIterator files(QFileInfo(configPath).absolutePath() + "/data", {"browser.sqlite"},
                           QDir::Files, QDirIterator::Subdirectories);
        int databases = 0;
        while (files.hasNext()) {
            sqlite3 *database = nullptr;
            QCOMPARE(sqlite3_open_v2(QFile::encodeName(files.next()).constData(), &database,
                                     SQLITE_OPEN_READWRITE, nullptr), SQLITE_OK);
            auto close = qScopeGuard([&] { sqlite3_close(database); });
            sqlite3_busy_timeout(database, 3000);
            sqlite3_stmt *insert = nullptr;
            QCOMPARE(sqlite3_prepare_v2(database,
                "INSERT INTO downloads(id,source_url,destination,state,bytes_received,created_at,completed_at) "
                "VALUES(?1,?2,?3,'completed',4,?4,?4)", -1, &insert, nullptr), SQLITE_OK);
            auto finish = qScopeGuard([&] { sqlite3_finalize(insert); });
            for (int index = 0; index < 30; ++index) {
                const auto id = QString("paging-download-%1").arg(index).toUtf8();
                const auto url = QByteArray("https://example.test/") + id;
                const auto path = QFileInfo(configPath).absolutePath()
                    + QString("/Research <final> report %1 — δ.zip").arg(index);
                write(path, "data");
                const auto destination = path.toUtf8();
                QCOMPARE(sqlite3_bind_text(insert, 1, id.constData(), -1, SQLITE_TRANSIENT), SQLITE_OK);
                QCOMPARE(sqlite3_bind_text(insert, 2, url.constData(), -1, SQLITE_TRANSIENT), SQLITE_OK);
                QCOMPARE(sqlite3_bind_text(insert, 3, destination.constData(), -1, SQLITE_TRANSIENT), SQLITE_OK);
                QCOMPARE(sqlite3_bind_int(insert, 4, index + 1), SQLITE_OK);
                QCOMPARE(sqlite3_step(insert), SQLITE_DONE);
                QCOMPARE(sqlite3_reset(insert), SQLITE_OK);
            }
            ++databases;
        }
        QVERIFY(databases > 0);
        evaluate(ui + ".refresh_download_history()");
        QTRY_COMPARE(evaluate(ui + ".download_history_status()").toString(), QString("ready"));
        submitAt(host, ui, "downloads");
        QTRY_VERIFY(visual(host->contentItem(), "downloadList"));
        auto list = visual(host->contentItem(), "downloadList");
        QTRY_COMPARE(list->property("count").toInt(), 30);
        QTRY_VERIFY(list->hasActiveFocus());
        key(host, Qt::Key_J);
        QCOMPARE(list->property("currentIndex").toInt(), 1);
        key(host, Qt::Key_K);
        QCOMPARE(list->property("currentIndex").toInt(), 0);
        verifyPages(host, list, list);
        key(host, Qt::Key_End);
        QCOMPARE(list->property("currentIndex").toInt(), 29);
        auto row = qobject_cast<QQuickItem *>(list->property("currentItem").value<QObject *>());
        QVERIFY(row);
        key(host, Qt::Key_Tab);
        auto open = visual(row, "downloadOpenButton");
        QVERIFY(open && open->isEnabled());
        QTRY_COMPARE(host->activeFocusItem(), open);
        QVERIFY(open->mapToItem(list, QPointF()).y() >= -1);
        QVERIFY(open->mapToItem(list, QPointF(0, open->height())).y() <= list->height() + 1);
        key(host, Qt::Key_Backtab, Qt::ShiftModifier);
        QTRY_VERIFY(list->hasActiveFocus());
        key(host, Qt::Key_Return);
        QVERIFY(row->property("detailsExpanded").toBool());
        auto details = visual(row, "downloadDetails");
        QVERIFY(details);
        QVERIFY(details->property("text").toString().contains(row->property("destination").toString()));
        key(host, Qt::Key_Tab);
        QTRY_COMPARE(host->activeFocusItem(), open);
        key(host, Qt::Key_Tab);
        QTRY_COMPARE(host->activeFocusItem(), visual(row, "downloadFolderButton"));
        key(host, Qt::Key_Tab);
        QTRY_COMPARE(host->activeFocusItem(), visual(row, "downloadDetailsButton"));
        key(host, Qt::Key_Return);
        QVERIFY(!row->property("detailsExpanded").toBool());
        for (int step = 0; step < 4; ++step) key(host, Qt::Key_Backtab, Qt::ShiftModifier);
        auto close = visual(host->contentItem(), "downloadCloseButton");
        QVERIFY(close);
        QTRY_COMPARE(host->activeFocusItem(), close);
        QSignalSpy toolbarFrame(host, &QQuickWindow::frameSwapped);
        host->update();
        QTRY_VERIFY(!toolbarFrame.isEmpty());
        QVERIFY(close->mapToItem(list, QPointF()).y() >= -1);
        QVERIFY(close->mapToItem(list, QPointF(0, close->height())).y() <= list->height() + 1);
        QQuickItem *first = nullptr;
        QTRY_VERIFY2(QMetaObject::invokeMethod(list, "itemAtIndex", Q_RETURN_ARG(QQuickItem *, first),
                                             Q_ARG(int, 0)) && first
            && first->mapToItem(list, QPointF()).y() < list->height(),
            qPrintable(QString::fromUtf8(listGeometry(list)) + QString(" first=%1 firstY=%2 headerY=%3")
                .arg(first != nullptr).arg(first ? first->mapToItem(list, QPointF()).y() : -1)
                .arg(qobject_cast<QQuickItem *>(list->property("headerItem").value<QObject *>())->y())));
        QSignalSpy rowsFrame(host, &QQuickWindow::frameSwapped);
        host->update();
        QTRY_VERIFY(!rowsFrame.isEmpty());
        QCOMPARE(list->property("currentIndex").toInt(), 29);
        captureChrome(host, visual(first, "downloadFileName"), "downloads-keyboard-"
            + QString(secondary ? "secondary" : "primary"));
        key(host, Qt::Key_Return);
        QTRY_VERIFY(!list->isVisible());
        submitAt(host, ui, "downloads");
        QTRY_VERIFY(list->isVisible() && list->hasActiveFocus());
    }
    void libraryPaging() {
        seedBookmarks(30);
        window->resize(390, 600);
        submit("bookmark-list");
        QTRY_COMPARE(evaluate("testWindow.libraryEntries.count").toInt(), 30);
        auto list = visual(window->contentItem(), "libraryEntryList");
        QVERIFY(list);
        list->forceActiveFocus();
        QTRY_VERIFY(list->hasActiveFocus());
        QTRY_VERIFY(!evaluate("testWindow.browserUi.library_pending").toBool());
        key(window, Qt::Key_Backtab, Qt::ShiftModifier);
        QTRY_VERIFY(window->activeFocusItem()
            && window->activeFocusItem()->objectName() == "libraryCloseButton");
        key(window, Qt::Key_Backtab, Qt::ShiftModifier);
        QTRY_VERIFY(window->activeFocusItem()
            && window->activeFocusItem()->objectName() == "libraryRefreshButton");
        key(window, Qt::Key_Backtab, Qt::ShiftModifier);
        QTRY_VERIFY(list->hasActiveFocus());
        verifyPages(window, list, list);
        for (int index = 1; index <= 7; ++index) {
            key(window, Qt::Key_Down);
            QCOMPARE(list->property("currentIndex").toInt(), index);
        }
        key(window, Qt::Key_Tab);
        auto row = qobject_cast<QQuickItem *>(list->property("currentItem").value<QObject *>());
        auto focused = window->activeFocusItem();
        QVERIFY(focused);
        QCOMPARE(focused->property("text").toString(), QString("Open"));
        while (focused && focused != row) focused = focused->parentItem();
        QVERIFY(row && focused == row);
        key(window, Qt::Key_Backtab, Qt::ShiftModifier);
        QCOMPARE(list->property("currentIndex").toInt(), 7);
        QVERIFY(window->activeFocusItem() == list || window->activeFocusItem() == row);
        key(window, Qt::Key_Down);
        QCOMPARE(list->property("currentIndex").toInt(), 8);
        key(window, Qt::Key_Backtab, Qt::ShiftModifier);
        QTRY_VERIFY(window->activeFocusItem()
            && window->activeFocusItem()->objectName() == "libraryCloseButton");
        key(window, Qt::Key_Tab);
        QTRY_VERIFY(window->activeFocusItem()
            && window->activeFocusItem()->property("text").toString() == "Open");
        QCOMPARE(list->property("currentIndex").toInt(), 8);
        row = qobject_cast<QQuickItem *>(list->property("currentItem").value<QObject *>());
        focused = window->activeFocusItem();
        while (focused && focused != row) focused = focused->parentItem();
        QVERIFY(row && focused == row);
        key(window, Qt::Key_Backtab, Qt::ShiftModifier);
        QTRY_VERIFY(list->hasActiveFocus());
        QCOMPARE(list->property("currentIndex").toInt(), 8);
    }
    void libraryDrafts() {
        seedBookmarks(110);
        window->resize(390, 700);
        submit("bookmark-list");
        QTRY_COMPARE(evaluate("testWindow.libraryTotalEntries").toInt(), 110);
        auto list = visual(window->contentItem(), "libraryEntryList");
        QVERIFY(list);
        QTRY_COMPARE(list->property("count").toInt(), 100);
        QVERIFY(list->setProperty("cacheBuffer", 0));
        for (int index = 1; index <= 7; ++index) {
            key(window, Qt::Key_Down);
            QCOMPARE(list->property("currentIndex").toInt(), index);
        }
        const auto target = evaluate("testWindow.libraryEntries.get(7).entryId").toString();
        key(window, Qt::Key_Tab);
        key(window, Qt::Key_Tab);
        key(window, Qt::Key_Tab);
        QVERIFY(window->activeFocusItem());
        QCOMPARE(window->activeFocusItem()->objectName(), QString("libraryEntryEditButton"));
        key(window, Qt::Key_Return);
        auto row = qobject_cast<QQuickItem *>(list->property("currentItem").value<QObject *>());
        QVERIFY(row);
        auto input = visual(row, "libraryEntryEdit");
        QVERIFY(input && input->hasActiveFocus());
        key(window, Qt::Key_A, Qt::ControlModifier);
        const QByteArray draft = "an unsaved bookmark title";
        // Text entry uses Qt events; navigation and action gestures use the
        // compositor seat when this journey runs in the Wayland gate.
        for (const auto character : draft) QTest::keyClick(window, character);
        QCOMPARE(input->property("text").toString(), QString::fromLatin1(draft));
        key(window, Qt::Key_Backtab, Qt::ShiftModifier);
        key(window, Qt::Key_End);
        QSignalSpy frame(window, &QQuickWindow::frameSwapped);
        window->update();
        QTRY_VERIFY(!frame.isEmpty());
        QTRY_VERIFY(([&] {
            QQuickItem *oldRow = nullptr;
            return QMetaObject::invokeMethod(list, "itemAtIndex", Q_RETURN_ARG(QQuickItem *, oldRow),
                                              Q_ARG(int, 7)) && !oldRow;
        })());
        auto next = visual(window->contentItem(), "libraryNextPageButton");
        QVERIFY(next && next->isEnabled());
        for (int step = 0; step < 20 && window->activeFocusItem() != next; ++step)
            key(window, Qt::Key_Backtab, Qt::ShiftModifier);
        QCOMPARE(window->activeFocusItem(), next);
        key(window, Qt::Key_Return);
        QCOMPARE(lastKeyFocus.data(), next);
        QTRY_COMPARE(evaluate("testWindow.libraryPage").toInt(), 1);
        QTRY_COMPARE(list->property("count").toInt(), 10);
        auto previous = visual(window->contentItem(), "libraryPreviousPageButton");
        QVERIFY(previous && previous->isEnabled());
        for (int step = 0; step < 20 && window->activeFocusItem() != previous; ++step)
            key(window, Qt::Key_Backtab, Qt::ShiftModifier);
        QCOMPARE(window->activeFocusItem(), previous);
        QSignalSpy previousClicks(previous, SIGNAL(clicked()));
        QSignalSpy closeClicks(visual(window->contentItem(), "libraryCloseButton"), SIGNAL(clicked()));
        key(window, Qt::Key_Return);
        QCOMPARE(lastKeyFocus.data(), previous);
        QCOMPARE(previousClicks.size(), 1);
        QCOMPARE(closeClicks.size(), 0);
        QTRY_COMPARE(evaluate("testWindow.libraryPage").toInt(), 0);
        QTRY_VERIFY2(list->property("count").toInt() == 100, qPrintable(evaluate(
            "JSON.stringify({page:testWindow.libraryPage,visible:testWindow.libraryManagerVisible,"
            "kind:testWindow.browserUi.library_kind,total:testWindow.libraryTotalEntries,"
            "rows:testWindow.libraryEntries.count,values:testWindow.browserUi.library_values.length,"
            "status:testWindow.browserUi.status_text})").toString()));
        for (int step = 0; step < 20 && window->activeFocusItem() != list
                && window->activeFocusItem() != qobject_cast<QQuickItem *>(
                    list->property("currentItem").value<QObject *>()); ++step)
            key(window, Qt::Key_Backtab, Qt::ShiftModifier);
        QVERIFY(window->activeFocusItem() == list || window->activeFocusItem()
            == qobject_cast<QQuickItem *>(list->property("currentItem").value<QObject *>()));
        key(window, Qt::Key_Home);
        for (int index = 1; index <= 7; ++index) key(window, Qt::Key_Down);
        QCOMPARE(evaluate("testWindow.libraryEntries.get(7).entryId").toString(), target);
        row = qobject_cast<QQuickItem *>(list->property("currentItem").value<QObject *>());
        QVERIFY(row);
        input = visual(row, "libraryEntryEdit");
        QVERIFY(input);
        QCOMPARE(input->property("text").toString(), QString::fromLatin1(draft));
        QVERIFY(!evaluate("testWindow.browserUi.library_values").toString().contains(QString::fromLatin1(draft)));
        key(window, Qt::Key_Tab);
        QTRY_COMPARE(window->activeFocusItem(), input);
        key(window, Qt::Key_Return);
        QTRY_COMPARE(evaluate("testWindow.browserUi.mark_action_state").toString(), QString("succeeded"));
        QTRY_VERIFY(evaluate("testWindow.browserUi.library_values").toString().contains(
            target + "\t" + QString::fromLatin1(draft) + "\t"));
    }
    void journeyDestination_data() {
        QTest::addColumn<QString>("target");
        QTest::addColumn<int>("destinationIndex");
        QTest::newRow("new-tab") << QString("tab") << 1;
        QTest::newRow("new-window") << QString("window") << 2;
    }
    void journeyDestination() {
        QFETCH(QString, target);
        QFETCH(int, destinationIndex);
        const auto origin = QString("http://127.0.0.1:%1").arg(server.serverPort());
        const auto firstUrl = origin + "/journey-first";
        const auto secondUrl = origin + "/journey-second";
        QTRY_VERIFY(evaluate("(()=>{const v=testWindow.activeWebView();return !!v && v.ownsTab()"
            " && v.contentInitialized && v.mainFrame.isValid && !v.loading"
            " && String(v.nativeUrl)==='rb://blank'})()").toBool());
        QSignalSpy initialFrame(window, &QQuickWindow::frameSwapped);
        window->update();
        QTRY_VERIFY(!initialFrame.isEmpty());
        for (const auto &url : {firstUrl, secondUrl}) {
            auto view = evaluate("testWindow.activeWebView()").value<QObject *>();
            QVERIFY(view);
            QSignalSpy loads(view, SIGNAL(loadingChanged(QWebEngineLoadingInfo)));
            QVERIFY(loads.isValid());
            submit("open " + url);
            QTRY_COMPARE(evaluate("testWindow.browserUi.current_url").toString(), url);
            QTRY_COMPARE(evaluate("testWindow.activeWebView().url.toString()").toString(), url);
            const auto loadDetails = [&] {
                QStringList events;
                for (const auto &arguments : loads) {
                    const auto info = arguments.at(0).value<QWebEngineLoadingInfo>();
                    events.append(QString("%1: status=%2 error=%3 %4").arg(info.url().toString())
                        .arg(int(info.status())).arg(info.errorCode()).arg(info.errorString()));
                }
                return "Expected commit for " + url + "; events: " + events.join("; ")
                    + "; same active view: " + QString::number(
                        view == evaluate("testWindow.activeWebView()").value<QObject *>())
                    + "; document: " + documentUrl()
                    + "; view: " + evaluate("JSON.stringify({url:testWindow.activeWebView().url.toString(),"
                        "native:testWindow.activeWebView().nativeUrl,loading:testWindow.activeWebView().loading,"
                        "state:testWindow.browserUi.load_state})").toString();
            };
            // The URL setter and loading flag can precede the document. Read
            // the actual page before leaving it to seed the next visit.
            QTRY_VERIFY2(documentUrl() == url, qPrintable(loadDetails()));
            QTRY_VERIFY(!evaluate("testWindow.activeWebView().loading").toBool());
            QTRY_VERIFY2(journeyRecorded(url), qPrintable(loadDetails()));
        }
        window->resize(390, 700);
        submit("journey");
        QTRY_COMPARE(evaluate("testWindow.browserUi.library_kind").toString(), QString("journey"));
        QTRY_VERIFY(!evaluate("testWindow.browserUi.library_pending").toBool());
        const auto firstIndex = "(()=>{for(let i=0;i<testWindow.libraryEntries.count;++i)"
            "if(testWindow.libraryEntries.get(i).secondary.indexOf(" + quoted(firstUrl)
            + ")==0)return i;return -1})()";
        QTRY_VERIFY2(evaluate(firstIndex).toInt() >= 0, qPrintable("Expected " + firstUrl
            + "; journey rows: " + evaluate("testWindow.browserUi.library_values").toString()));
        const auto index = evaluate(firstIndex).toInt();
        auto list = visual(window->contentItem(), "libraryEntryList");
        QVERIFY(list);
        list->forceActiveFocus();
        key(window, Qt::Key_Home);
        for (int step = 0; step < index; ++step) key(window, Qt::Key_Down);
        QCOMPARE(list->property("currentIndex").toInt(), index);
        key(window, Qt::Key_Tab);
        auto selector = window->activeFocusItem();
        QVERIFY(selector);
        QCOMPARE(selector->objectName(), QString("libraryJourneyDestination"));
        for (int step = 0; step < destinationIndex; ++step) key(window, Qt::Key_Down);
        QTRY_COMPARE(selector->property("currentValue").toString(), target);
        key(window, Qt::Key_Backtab, Qt::ShiftModifier);
        QVERIFY(list->hasActiveFocus());
        const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
        if (!artifacts.isEmpty()) {
            QSignalSpy frame(window, &QQuickWindow::frameSwapped);
            window->update();
            QTRY_VERIFY(!frame.isEmpty());
            QVERIFY(window->grabWindow().save(artifacts + "/journey-" + target + "-destination.png"));
        }
        key(window, Qt::Key_Return);
        QTRY_VERIFY(!evaluate("testWindow.libraryManagerVisible").toBool());
        if (target == "tab") {
            QTRY_COMPARE(evaluate("testWindow.browserUi.tab_count").toInt(), 2);
            QTRY_COMPARE(evaluate("testWindow.browserUi.current_url").toString(), firstUrl);
            QTRY_COMPARE(evaluate("testWindow.activeWebView().url.toString()").toString(), firstUrl);
            QTRY_COMPARE(evaluate("testWindow.activeWebView().title").toString(), QString("Owned child secret"));
            QTRY_VERIFY(!evaluate("testWindow.activeWebView().loading").toBool());
            QTRY_COMPARE(documentUrl(), firstUrl);
            QCOMPARE(evaluate("testWindow.tabsModel.get(0).url").toString(), secondUrl);
            QCOMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 1);
        } else {
            QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
            const QString child = "testWindow.browserWindowRegistry[1]";
            QTRY_VERIFY(evaluate(child + ".host.profileBootstrapComplete && !" + child
                                 + ".ui.profile_bootstrap_pending").toBool());
            QTRY_COMPARE(evaluate(child + ".ui.current_url").toString(), firstUrl);
            QTRY_VERIFY(evaluate("(()=>{const v=" + child + ".host.activeWebView();return !!v"
                " && v.url.toString()===" + quoted(firstUrl)
                + " && v.title==='Owned child secret' && !v.loading})()").toBool());
            QTRY_COMPARE(documentUrl(child + ".host.activeWebView()"), firstUrl);
            QCOMPARE(evaluate("testWindow.browserUi.tab_count").toInt(), 1);
            QCOMPARE(evaluate("testWindow.browserUi.current_url").toString(), secondUrl);
            QCOMPARE(evaluate("testWindow.activeWebView().url.toString()").toString(), secondUrl);
        }
    }
    void journeyGraphKeyboard() {
        auto view = evaluate("testWindow.activeWebView()").value<QObject *>();
        QVERIFY(view);
        QSignalSpy loads(view, SIGNAL(loadingChanged(QWebEngineLoadingInfo)));
        QVERIFY(loads.isValid());
        const auto loadEvents = [&] {
            QJsonArray events;
            for (const auto &arguments : loads) {
                const auto info = arguments.at(0).value<QWebEngineLoadingInfo>();
                events.append(QJsonObject{{"status", int(info.status())}, {"url", info.url().toString()}});
            }
            return QString::fromUtf8(QJsonDocument(events).toJson(QJsonDocument::Compact));
        };
        const auto origin = QString("http://127.0.0.1:%1").arg(server.serverPort());
        for (int index = 0; index < 15; ++index) {
            const auto url = origin + "/graph-page-" + QString::number(index);
            submit("open " + url);
            QTRY_COMPARE(documentUrl(), url);
            QTRY_VERIFY(!evaluate("testWindow.activeWebView().loading").toBool());
            QTRY_VERIFY2(journeyRecorded(url), qPrintable("Unrecorded visit " + url + " "
                + journeyStorageState() + " " + evaluate("JSON.stringify({url:testWindow.browserUi.current_url,"
                    "state:testWindow.browserUi.load_state,status:testWindow.browserUi.status_text})").toString()
                + " events=" + loadEvents()));
        }
        QVERIFY2(!journeyRecorded("about:blank"), "Native blank startup must not persist an engine placeholder visit");
        window->resize(390, 700);
        submit("journey");
        QTRY_VERIFY(!evaluate("testWindow.browserUi.library_pending").toBool());
        QTRY_VERIFY(evaluate("testWindow.libraryGraphEntries.count").toInt() >= 14);
        auto outline = visual(window->contentItem(), "libraryEntryList");
        QVERIFY(outline);
        QTRY_VERIFY(window->activeFocusItem() && (window->activeFocusItem() == outline
            || window->activeFocusItem() == qobject_cast<QQuickItem *>(
                outline->property("currentItem").value<QObject *>())));
        QSignalSpy outlineFrame(window, &QQuickWindow::frameSwapped);
        window->update();
        QTRY_VERIFY(!outlineFrame.isEmpty());
        auto toggle = visual(window->contentItem(), "libraryGraphToggleButton");
        QVERIFY(toggle);
        for (int step = 0; step < 20 && window->activeFocusItem() != toggle; ++step)
            key(window, Qt::Key_Backtab, Qt::ShiftModifier);
        QCOMPARE(window->activeFocusItem(), toggle);
        key(window, Qt::Key_Return);
        QCOMPARE(lastKeyFocus.data(), toggle);
        QTRY_VERIFY(evaluate("testWindow.libraryJourneyGraphMode").toBool());
        auto graph = visual(window->contentItem(), "journeyGraphList");
        QVERIFY(graph);
        QTRY_VERIFY(graph->hasActiveFocus());
        QSignalSpy initialFrame(window, &QQuickWindow::frameSwapped);
        window->update();
        QTRY_VERIFY(!initialFrame.isEmpty());
        auto header = qobject_cast<QQuickItem *>(graph->property("headerItem").value<QObject *>());
        QVERIFY(header);
        key(window, Qt::Key_Tab);
        QTRY_VERIFY(window->activeFocusItem()
            && window->activeFocusItem()->objectName() == "libraryGraphToggleButton");
        QCOMPARE(window->activeFocusItem()->property("text").toString(), QString("Outline"));
        key(window, Qt::Key_Backtab, Qt::ShiftModifier);
        QTRY_COMPARE(window->activeFocusItem(), graph);
        const auto atEnd = [&] {
            QQuickItem *last = nullptr;
            if (!QMetaObject::invokeMethod(graph, "itemAtIndex", Q_RETURN_ARG(QQuickItem *, last),
                    Q_ARG(int, graph->property("count").toInt() - 1)) || !last) return false;
            const auto bottom = last->mapToItem(graph, QPointF(0, last->height())).y();
            return bottom > 0 && bottom <= graph->height() + 1;
        };
        auto map = visual(window->contentItem(), "journeyMapViewport");
        QVERIFY(map);
        for (int step = 0; step < 20 && window->activeFocusItem() != map; ++step)
            key(window, Qt::Key_Tab);
        QCOMPARE(window->activeFocusItem(), map);
        QSignalSpy mapFrame(window, &QQuickWindow::frameSwapped);
        window->update();
        QTRY_VERIFY(!mapFrame.isEmpty());
        QVERIFY(map->height() <= graph->height() + 1);
        QVERIFY(map->property("contentHeight").toDouble() > map->height());
        const auto outerY = graph->property("contentY").toDouble();
        key(window, Qt::Key_Right);
        QTRY_VERIFY(map->property("contentX").toDouble() > 0);
        key(window, Qt::Key_PageDown);
        QCOMPARE(lastKeyFocus.data(), map);
        QTRY_VERIFY(map->property("contentY").toDouble() > 0);
        QVERIFY(map->property("contentY").toDouble() <= map->height() + 1);
        key(window, Qt::Key_End);
        QTRY_VERIFY(qAbs(map->property("contentY").toDouble()
            - (map->property("contentHeight").toDouble() - map->height())) < 1);
        key(window, Qt::Key_Home);
        QTRY_COMPARE(map->property("contentY").toDouble(), 0.0);
        QCOMPARE(graph->property("contentY").toDouble(), outerY);
        for (int step = 0; step < 20 && window->activeFocusItem() != graph; ++step)
            key(window, Qt::Key_Backtab, Qt::ShiftModifier);
        QCOMPARE(window->activeFocusItem(), graph);
        for (int direction : {1, -1}) {
            bool complete = false;
            for (int step = 0; step < graph->property("count").toInt() * 10 + 30; ++step) {
                const auto before = visibleRows(graph);
                const auto oldY = graph->property("contentY").toDouble();
                key(window, direction > 0 ? Qt::Key_PageDown : Qt::Key_PageUp);
                QCOMPARE(lastKeyFocus.data(), graph);
                QSignalSpy frame(window, &QQuickWindow::frameSwapped);
                window->update();
                QTRY_VERIFY(!frame.isEmpty());
                const auto after = visibleRows(graph);
                if (before.first <= before.second && after.first <= after.second) {
                    QVERIFY2(after.first <= before.second + 1 && after.second >= before.first - 1,
                             "Relationship paging skips no visible rows");
                }
                QVERIFY(graph->hasActiveFocus());
                if (qAbs(graph->property("contentY").toDouble() - oldY) < 1) {
                    if (direction > 0) {
                        if (!atEnd()) {
                            QQuickItem *last = nullptr;
                            QMetaObject::invokeMethod(graph, "itemAtIndex", Q_RETURN_ARG(QQuickItem *, last),
                                Q_ARG(int, graph->property("count").toInt() - 1));
                            qInfo() << "Relationship paging stopped before the final row"
                                << listGeometry(graph) << "count" << graph->property("count")
                                << "contentHeight" << graph->property("contentHeight")
                                << "header" << header->y() << header->height()
                                << "visible rows" << after << "last row bottom"
                                << (last ? last->mapToItem(graph, QPointF(0, last->height())).y() : -1);
                            const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
                            if (!artifacts.isEmpty())
                                window->grabWindow().save(artifacts + "/journey-paging-stopped.png");
                        }
                        QVERIFY(atEnd());
                    }
                    else QVERIFY(header->mapToItem(graph, QPointF()).y() >= -1);
                    complete = true;
                    break;
                }
            }
            QVERIFY(complete);
        }
        key(window, Qt::Key_End);
        QTRY_VERIFY(atEnd());
        const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
        if (!artifacts.isEmpty()) {
            QSignalSpy frame(window, &QQuickWindow::frameSwapped);
            window->update();
            QTRY_VERIFY(!frame.isEmpty());
            QVERIFY(window->grabWindow().save(artifacts + "/journey-relationships-end.png"));
        }
        key(window, Qt::Key_Home);
        QTRY_VERIFY(header->mapToItem(graph, QPointF()).y() >= -1);
        key(window, Qt::Key_G, Qt::ShiftModifier);
        QTRY_VERIFY(atEnd());
        key(window, Qt::Key_G);
        QTRY_VERIFY(header->mapToItem(graph, QPointF()).y() >= -1);
        if (!artifacts.isEmpty()) {
            QSignalSpy frame(window, &QQuickWindow::frameSwapped);
            window->update();
            QTRY_VERIFY(!frame.isEmpty());
            QVERIFY(window->grabWindow().save(artifacts + "/journey-relationships-controls.png"));
        }
    }
    void heldEnterRequiresFreshConfirmation() {
        const auto origin = QString("http://127.0.0.1:%1").arg(server.serverPort());
        for (const auto &name : {QString("alpha"), QString("beta"), QString("gamma")}) {
            submit((name == "alpha" ? "open " : "tab-open ") + origin + "/" + name);
            QTRY_COMPARE(evaluate("testWindow.browserUi.current_url").toString(),
                         origin + "/" + name);
            QTRY_COMPARE(evaluate("testWindow.activeWebView().url.toString()").toString(),
                         origin + "/" + name);
        }
        submit("switcher --scope tabs alpha");
        QTRY_VERIFY(
            evaluate("testWindow.switcherResultsCurrent() && testWindow.switcherResults.length===1")
                .toBool());
        auto input = visual(window->contentItem(), "switcherSearchInput");
        QVERIFY(input);
        QTRY_VERIFY(input->hasActiveFocus());
        QVERIFY(input->setProperty("text", "beta"));
        QKeyEvent initialPress(QEvent::KeyPress, Qt::Key_Return, Qt::NoModifier, QString(), false,
                               1);
        QCoreApplication::sendEvent(window, &initialPress);
        QKeyEvent searchingRepeat(QEvent::KeyPress, Qt::Key_Return, Qt::NoModifier, QString(), true,
                                  1);
        QCoreApplication::sendEvent(window, &searchingRepeat);
        QCOMPARE(evaluate("testWindow.browserUi.current_url").toString(), origin + "/gamma");
        QTRY_VERIFY(evaluate("testWindow.switcherResultsCurrent() && "
                             "testWindow.switcherResultContext.query==='beta' && "
                             "testWindow.switcherResults.length===1")
                        .toBool());
        QKeyEvent readyRepeat(QEvent::KeyPress, Qt::Key_Return, Qt::NoModifier, QString(), true, 1);
        QCoreApplication::sendEvent(window, &readyRepeat);
        const QJsonObject facts{{"repeat", true},
                                {"before", origin + "/gamma"},
                                {"after", evaluate("testWindow.browserUi.current_url").toString()},
                                {"visible", evaluate("testWindow.switcherVisible").toBool()}};
        qInfo().noquote() << QJsonDocument(facts).toJson(QJsonDocument::Compact);
        const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
        if (!artifacts.isEmpty())
            write(artifacts + "/held-enter.json", QJsonDocument(facts).toJson());
        QCOMPARE(evaluate("testWindow.browserUi.current_url").toString(), origin + "/gamma");
        QVERIFY(evaluate("testWindow.switcherVisible").toBool());
        QKeyEvent heldRelease(QEvent::KeyRelease, Qt::Key_Return, Qt::NoModifier, QString(), false,
                              1);
        QCoreApplication::sendEvent(window, &heldRelease);
        QTest::keyClick(window, Qt::Key_Return);
        QTRY_COMPARE(evaluate("testWindow.browserUi.current_url").toString(), origin + "/beta");
        QTRY_VERIFY(!evaluate("testWindow.switcherVisible").toBool());
    }
    void savedLibraryPendingIsNotEmpty() {
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("String(testWindow.browserUi.permission_decision('https:"
                                           "//owned-switcher.test','notifications'))")
                                      .toString(),
                                  QString("ask"), 15000);
        QVERIFY(evaluate("testWindow.browserUi.switcher_query('', 'marks')").toBool());
        QTRY_VERIFY(!evaluate("testWindow.browserUi.library_pending").toBool());
        QDirIterator files(QFileInfo(configPath).absolutePath() + "/data", {"browser.sqlite"},
                           QDir::Files, QDirIterator::Subdirectories);
        QVERIFY(files.hasNext());
        const auto metadata = files.next();
        QVERIFY(!files.hasNext());
        struct WriteLock {
            sqlite3 *db = nullptr;
            ~WriteLock() {
                if (db) {
                    sqlite3_exec(db, "ROLLBACK", nullptr, nullptr, nullptr);
                    sqlite3_close(db);
                }
            }
        } held;
        QCOMPARE(sqlite3_open_v2(QFile::encodeName(metadata).constData(), &held.db,
                                 SQLITE_OPEN_READWRITE, nullptr),
                 SQLITE_OK);
        QCOMPARE(sqlite3_exec(held.db, "BEGIN IMMEDIATE", nullptr, nullptr, nullptr), SQLITE_OK);
        QCOMPARE(sqlite3_exec(held.db,
                              "INSERT INTO bookmarks(id,url,title,created_at,updated_at) "
                              "VALUES('held-switcher-mark','https://owned-switcher.test/"
                              "heldmark','heldmark',1,1)",
                              nullptr, nullptr, nullptr),
                 SQLITE_OK);
        QVERIFY(evaluate("testWindow.browserUi.remember_permission('https://"
                         "owned-switcher.test','notifications','deny','site')")
                    .toBool());
        evaluate("testWindow.browserUi.refresh_library()");
        QTRY_VERIFY(evaluate("testWindow.browserUi.library_pending").toBool());
        submit("switcher --scope marks heldmark");
        QTRY_VERIFY(
            evaluate(
                "testWindow.switcherResultsCurrent() && testWindow.switcherSourceState.pending")
                .toBool());
        QCOMPARE(evaluate("testWindow.switcherResults.length").toInt(), 0);
        QVERIFY(evaluate("testWindow.browserUi.library_pending").toBool());
        auto status = visual(window->contentItem(), "switcherSourceStatus");
        QVERIFY(status);
        auto message = visual(status, "managerFeedbackMessage");
        QVERIFY(message);
        QCOMPARE(message->property("text").toString(),
                 QString("Loading bookmarks and quickmarks…"));
        QVERIFY(!visual(window->contentItem(), "switcherEmptyState"));
        QSignalSpy frame(window, &QQuickWindow::frameSwapped);
        window->requestUpdate();
        QTRY_VERIFY(!frame.isEmpty());
        const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
        if (!artifacts.isEmpty())
            QVERIFY(window->grabWindow().save(artifacts + "/switcher-library-pending.png"));
        QCOMPARE(sqlite3_exec(held.db, "COMMIT", nullptr, nullptr, nullptr), SQLITE_OK);
        sqlite3_close(held.db);
        held.db = nullptr;
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("!testWindow.switcherSourceState.pending && "
                                          "testWindow.switcherResults.some(function(row){return "
                                          "row.kind==='bookmark' && row.label==='heldmark'})")
                                     .toBool(),
                                 15000);
        QVERIFY(!evaluate("testWindow.switcherSourceState.failed").toBool());
    }

    void savedLibraryFailureWaitsForExplicitRetry() {
        QVERIFY(evaluate("testWindow.browserUi.switcher_query('', 'marks')").toBool());
        QTRY_VERIFY(!evaluate("testWindow.browserUi.library_pending").toBool());
        QDirIterator files(QFileInfo(configPath).absolutePath() + "/data", {"browser.sqlite"},
                           QDir::Files, QDirIterator::Subdirectories);
        QVERIFY(files.hasNext());
        const auto metadata = files.next();
        QVERIFY(!files.hasNext());
        struct Connection {
            sqlite3 *db = nullptr;
            ~Connection() {
                if (db)
                    sqlite3_close(db);
            }
        } database;
        QCOMPARE(sqlite3_open_v2(QFile::encodeName(metadata).constData(), &database.db,
                                 SQLITE_OPEN_READWRITE, nullptr),
                 SQLITE_OK);
        sqlite3_busy_timeout(database.db, 3000);
        // A real malformed stored value makes the actual catalog read fail;
        // the fixture never assigns the published readiness properties.
        QCOMPARE(sqlite3_exec(database.db,
                              "INSERT INTO bookmarks(id,url,title,created_at,updated_at) "
                              "VALUES('invalid-switcher-mark','https://owned-switcher.test/"
                              "recovered',zeroblob(4),1,1)",
                              nullptr, nullptr, nullptr),
                 SQLITE_OK);
        evaluate("testWindow.browserUi.refresh_library()");
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("!testWindow.browserUi.library_pending && "
                                          "testWindow.browserUi.library_error.length>0")
                                     .toBool(),
                                 15000);
        submit("switcher --scope marks recovered");
        QTRY_VERIFY(
            evaluate("testWindow.switcherResultsCurrent() && testWindow.switcherSourceState.failed "
                     "&& !testWindow.switcherSourceState.pending")
                .toBool());
        const auto detail = evaluate("testWindow.switcherSourceState.detail").toString();
        QVERIFY(!detail.isEmpty());
        QVERIFY(!evaluate("testWindow.switcherSourceState.notice").toString().contains(detail));
        auto input = visual(window->contentItem(), "switcherSearchInput");
        QVERIFY(input);
        QVERIFY(input->setProperty("text", "recovered bookmark"));
        QTRY_VERIFY(evaluate("testWindow.switcherResultsCurrent() && "
                             "testWindow.switcherResultContext.query==='recovered bookmark'")
                        .toBool());
        QVERIFY(
            evaluate(
                "testWindow.switcherSourceState.failed && !testWindow.switcherSourceState.pending")
                .toBool());
        QCOMPARE(evaluate("testWindow.switcherSourceState.detail").toString(), detail);
        // Repair the owned data. Typing still must not silently retry I/O.
        QCOMPARE(
            sqlite3_exec(
                database.db,
                "UPDATE bookmarks SET title='recovered bookmark' WHERE id='invalid-switcher-mark'",
                nullptr, nullptr, nullptr),
            SQLITE_OK);
        evaluate("testWindow.refreshSwitcher()");
        QTRY_VERIFY(
            evaluate("testWindow.switcherResultsCurrent() && "
                     "testWindow.switcherBatchIndex>=testWindow.switcherBatchEntries.length")
                .toBool());
        QVERIFY(evaluate("testWindow.switcherSourceState.failed").toBool());
        auto refresh = visual(window->contentItem(), "switcherSourceRefresh");
        QVERIFY(refresh);
        refresh->forceActiveFocus(Qt::TabFocusReason);
        QTRY_VERIFY(refresh->hasActiveFocus());
        QTest::keyClick(window, Qt::Key_Return);
        QTRY_VERIFY_WITH_TIMEOUT(
            evaluate(
                "!testWindow.switcherSourceState.pending && !testWindow.switcherSourceState.failed "
                "&& testWindow.switcherResults.some(function(row){return "
                "row.id==='invalid-switcher-mark'})")
                .toBool(),
            15000);
        QCOMPARE(evaluate("testWindow.switcherSourceState.detail").toString(), QString());
        // A failure in a saved source must not make a live-only scope fail.
        evaluate("testWindow.setSwitcherScope('tabs')");
        QTRY_VERIFY(
            evaluate(
                "testWindow.switcherResultsCurrent() && !testWindow.switcherSourceState.failed && "
                "!testWindow.switcherSourceState.pending")
                .toBool());
    }

    void invokingWindowOwnsTheCombinedLimit() {
        QCOMPARE(evaluate("testWindow.browserUi.feature_switcher_max_results").toInt(), 100);
        auto child = createChild("limit-owner", false);
        QVERIFY(child);
        const QString ui = "testWindow.browserWindowRegistry[1].ui";
        for (int i = 0; i < 10; ++i)
            submitAt(child, ui, "tab-open rb://blank");
        QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 11);
        // Tab-count publication precedes native view activation. Let the last
        // tab finish taking focus before driving another command-line gesture.
        QTRY_VERIFY(evaluate("(()=>{const v=testWindow.browserWindowRegistry[1].host.activeWebView();"
                             "return !!v && v.ownsTab() && !v.loading && v.mainFrame.isValid"
                             " && String(v.nativeUrl)==='rb://blank'})()").toBool());
        // This fixture needs an accepted scoped setting before inspecting
        // the combined limit. Startup can still own the reload reservation;
        // honor its explicit retry response without retrying other failures.
        QElapsedTimer settingReady;
        settingReady.start();
        do {
            submitAt(child, ui, "set --temp switcher.max_results=10");
            if (evaluate(ui + ".status_text").toString()
                    != "Settings are being reloaded. Try again in a moment.") break;
            QTest::qWait(20);
        } while (settingReady.elapsed() < 5000);
        QTRY_VERIFY2(evaluate(ui + ".feature_switcher_max_results").toInt() == 10,
                     qPrintable(evaluate("JSON.stringify({mode:" + ui + ".mode,status:" + ui
                         + ".status_text,limit:" + ui + ".feature_switcher_max_results})").toString()));
        QCOMPARE(evaluate("testWindow.browserUi.feature_switcher_max_results").toInt(), 100);
        submitAt(child, ui, "switcher --scope tabs");
        QTRY_VERIFY(
            evaluate(
                "testWindow.switcherResultsCurrent() && testWindow.switcherBatchEntries.length===2 "
                "&& testWindow.switcherBatchIndex===2")
                .toBool());
        QCOMPARE(evaluate("testWindow.switcherHost").value<QObject *>(),
                 static_cast<QObject *>(child));
        QCOMPARE(evaluate("testWindow.switcherMaxResults()").toInt(), 10);
        // Each source has its own backend cap. The invoking window's ten-item
        // cap must also constrain their combined results and visible count.
        QCOMPARE(evaluate("testWindow.switcherResults.length").toInt(), 10);
        auto count = visual(child->contentItem(), "switcherResultCount");
        QVERIFY(count);
        QCOMPARE(count->property("text").toString(), QString("10 results"));
    }
    void rootSwitcherHonorsPrivacy_data() {
        QTest::addColumn<QString>("kind");
        for (const auto &kind : {"regular", "private", "ephemeral"})
            QTest::newRow(kind) << QString::fromLatin1(kind);
    }
    void rootSwitcherHonorsPrivacy() {
        QFETCH(QString, kind);
        const bool privateChild = kind == "private";
        const bool ephemeralChild = kind == "ephemeral";
        const bool transientChild = privateChild || ephemeralChild;
        QVERIFY(!evaluate("testWindow.temporaryProfile || testWindow.browserProfile.offTheRecord")
                     .toBool());
        const auto url = QString("http://127.0.0.1:%1/child-secret").arg(server.serverPort());
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:" +
                 quoted(url) + ",windowProfileName:'privacy-fixture',windowPrivateProfile:" +
                 (privateChild ? "true" : "false") +
                 ",windowEphemeralProfile:" + (ephemeralChild ? "true" : "false") +
                 ",windowEphemeralInvocationToken:'owned-switcher-ephemeral'})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
        QTRY_VERIFY_WITH_TIMEOUT(
            evaluate("testWindow.browserWindowRegistry[1].host.profileBootstrapComplete && "
                     "!testWindow.browserWindowRegistry[1].ui.profile_bootstrap_pending")
                .toBool(),
            15000);
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry[1].ui.current_url").toString(),
                     url);
        QCOMPARE(evaluate("testWindow.browserWindowRegistry[1].profile.offTheRecord").toBool(),
                 transientChild);
        const auto child =
            evaluate("String(testWindow.browserWindowRegistry[1].ui.window_token)").toString();
        submit("switcher --scope tabs child-secret");
        QTRY_VERIFY(
            evaluate("testWindow.switcherVisible && testWindow.switcherBatchQuery==='child-secret' "
                     "&& testWindow.switcherBatchEntries.length>0 && "
                     "testWindow.switcherBatchIndex>=testWindow.switcherBatchEntries.length")
                .toBool());
        QCOMPARE(evaluate("testWindow.switcherBatchEntries.length").toInt(),
                 transientChild ? 1 : 2);
        const auto rows = evaluate("JSON.stringify(testWindow.switcherResults)").toString();
        const bool found =
            evaluate("testWindow.switcherResults.some(function(row){return row.owner_token===" +
                     quoted(child) + "})")
                .toBool();
        const QJsonObject facts{{"kind", kind},
                                {"childToken", child},
                                {"childReturned", found},
                                {"rows", QJsonDocument::fromJson(rows.toUtf8()).array()}};
        qInfo().noquote() << QJsonDocument(facts).toJson(QJsonDocument::Compact);
        const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
        if (!artifacts.isEmpty()) {
            QSignalSpy frame(window, &QQuickWindow::frameSwapped);
            window->requestUpdate();
            QTRY_VERIFY(!frame.isEmpty());
            QVERIFY(window->grabWindow().save(
                artifacts + "/" + QString::fromLatin1(QTest::currentDataTag()) + ".png"));
            write(artifacts + "/" + QString::fromLatin1(QTest::currentDataTag()) + ".json",
                  QJsonDocument(facts).toJson());
        }
        QCOMPARE(found, !transientChild);
    }
    void privateInvokerStaysInItsSessionAndHost() {
        const auto normal =
            QString("http://127.0.0.1:%1/child-secret-normal").arg(server.serverPort());
        submit("open " + normal);
        QTRY_COMPARE(evaluate("testWindow.browserUi.current_url").toString(), normal);
        auto privateHost = createChild("child-secret-a", true);
        QVERIFY(privateHost);
        auto independent = createChild("child-secret-b", true);
        QVERIFY(independent);
        QVERIFY(evaluate("testWindow.browserWindowRegistry[1].profile!==testWindow."
                         "browserWindowRegistry[2].profile")
                    .toBool());
        submitAt(privateHost, "testWindow.browserWindowRegistry[1].ui",
                 "set --temp ui.font_size_pt=20");
        QTRY_COMPARE(privateHost->property("font").value<QFont>().pointSize(), 20);
        privateHost->resize(640, 480);
        submitAt(privateHost, "testWindow.browserWindowRegistry[1].ui",
                 "switcher --scope tabs child-secret");
        QTRY_VERIFY(
            evaluate(
                "testWindow.switcherVisible && testWindow.switcherBatchQuery==='child-secret' && "
                "testWindow.switcherBatchEntries.length===1 && testWindow.switcherBatchIndex===1")
                .toBool());
        QCOMPARE(evaluate("testWindow.switcherHost").value<QObject *>(),
                 static_cast<QObject *>(privateHost));
        QVERIFY(evaluate("testWindow.switcherUi===testWindow.browserWindowRegistry[1].ui && "
                         "!testWindow.modalInputScope.activeSurface && "
                         "!!testWindow.switcherHost.modalInputScope.activeSurface")
                    .toBool());
        const auto rows = evaluate("JSON.stringify(testWindow.switcherResults)").toString();
        QVERIFY(evaluate("testWindow.switcherResults.length>0 && "
                         "testWindow.switcherResults.every(function(row){return "
                         "row.owner_token===testWindow.switcherOwnerToken})")
                    .toBool());
        auto field = visual(privateHost->contentItem(), "switcherSearchInput");
        QVERIFY(field);
        QTRY_VERIFY(field->hasActiveFocus());
        auto panel = visual(privateHost->contentItem(), "ferricModalPanel");
        QVERIFY(panel);
        QSignalSpy frame(privateHost, &QQuickWindow::frameSwapped);
        privateHost->requestUpdate();
        QTRY_VERIFY(!frame.isEmpty());
        QVERIFY(QRectF(0, 0, privateHost->width(), privateHost->height())
                    .adjusted(-1, -1, 1, 1)
                    .contains(panel->mapRectToScene(panel->boundingRect())));
        const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
        if (!artifacts.isEmpty()) {
            QVERIFY(privateHost->grabWindow().save(artifacts + "/private-invoker-large.png"));
            write(
                artifacts + "/private-invoker.json",
                QJsonDocument(QJsonObject{{"rows", QJsonDocument::fromJson(rows.toUtf8()).array()},
                                          {"hostedInInvoker", true},
                                          {"sourceCount", 1}})
                    .toJson());
        }
        window->requestActivate();
        QTRY_VERIFY(window->isActive());
        QTest::keyClick(window, Qt::Key_Escape);
        QVERIFY(evaluate("testWindow.switcherVisible").toBool());
        privateHost->requestActivate();
        QTRY_VERIFY(privateHost->isActive());
        QTest::keyClick(privateHost, Qt::Key_Escape);
        QTRY_VERIFY(!evaluate("testWindow.switcherVisible").toBool());
        QTRY_VERIFY(
            evaluate("testWindow.browserWindowRegistry[1].host.activeView.activeFocus").toBool());
        QCOMPARE(evaluate("testWindow.browserUi.current_url").toString(), normal);
    }
    void retiredResultDoesNotUsePrimaryOwner() {
        auto child = createChild("child-secret-retired", false);
        QVERIFY(child);
        submit("switcher --scope tabs child-secret-retired");
        QTRY_VERIFY(
            evaluate(
                "testWindow.switcherResults.some(function(row){return "
                "row.owner_token===String(testWindow.browserWindowRegistry[1].ui.window_token)})")
                .toBool());
        const auto captured =
            evaluate(
                "JSON.stringify(testWindow.switcherResults.find(function(row){return "
                "row.owner_token===String(testWindow.browserWindowRegistry[1].ui.window_token)}))")
                .toString();
        const auto before = evaluate("testWindow.browserUi.current_url").toString();
        evaluate("testWindow.browserWindowRegistry[1].host.windowShutdownApproved=true;testWindow."
                 "browserWindowRegistry[1].host.close()");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 1);
        // Retain the captured action across actual owner retirement, as an
        // already-pressed result can outlive its source window.
        evaluate("testWindow.switcherResults=[JSON.parse(" + quoted(captured) + ")]");
        evaluate("testWindow.activateSwitcher(0)");
        QCOMPARE(evaluate("testWindow.browserUi.current_url").toString(), before);
        QVERIFY(evaluate("testWindow.browserUi.status_text.indexOf('no longer available')>=0")
                    .toBool());
        QVERIFY(evaluate("testWindow.switcherVisible").toBool());
    }
};
int main(int argc, char **argv) {
    ferric_browser_enable_software_rendering();
    ferric_browser_register_internal_scheme();
    QQuickWindow::setGraphicsApi(QSGRendererInterface::Software);
    QtWebEngineQuick::initialize();
    cxx_qt_init_crate_ferric_browser_engine_qt();
    QVector<QByteArray> args;
    for (int i = 0; i < argc; ++i)
        args.append(QByteArray(argv[i]));
    auto app = ferric_browser_gui_application(args);
    if (!app || !ferric_browser_gui_preferences_ready())
        return 1;
    SwitcherTests tests;
    return QTest::qExec(&tests, argc, argv);
}
#include "tst_switcher.moc"
