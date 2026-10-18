#include "../../src/native_file_chooser.h"
#include "../../src/gui_application.h"
#include <QAbstractItemView>
#include <QApplication>
#include <QDialogButtonBox>
#include <QFile>
#include <QFileSystemModel>
#include <QPushButton>
#include <QLineEdit>
#include <QQmlApplicationEngine>
#include <QQmlComponent>
#include <QQuickWindow>
#include <QQmlProperty>
#include <QTemporaryDir>
#include <QtTest>

namespace {
QFileDialog *picker() {
    for (auto candidate : QApplication::topLevelWidgets())
        if (auto dialog = qobject_cast<QFileDialog *>(candidate))
            if (dialog->objectName() == "chooserUnderTest") return dialog;
    return nullptr;
}
QAbstractItemView *fileView() {
    if (auto dialog = picker())
        for (auto view : dialog->findChildren<QAbstractItemView *>())
            if (view->isVisible() && qobject_cast<QFileSystemModel *>(view->model())) return view;
    return nullptr;
}
}

class FileChooserBackendTests : public QObject {
    Q_OBJECT
    QTemporaryDir directory;
    QString first, second;
    QQuickWindow host;
    void open(FerricNativeFileChooser &chooser, int mode) {
        chooser.setObjectName("chooserUnderTest");
        chooser.setParentWindow(&host);
        chooser.setFileMode(mode);
        chooser.setOptions(QFileDialog::DontUseNativeDialog);
        chooser.setCurrentFolder(QUrl::fromLocalFile(directory.path()));
        chooser.open();
        QVERIFY(picker());
        QCOMPARE(picker()->windowHandle()->transientParent(), &host);
    }
    void choose(const QString &path, Qt::KeyboardModifiers modifiers = Qt::NoModifier) {
        QTRY_VERIFY(fileView());
        auto view = fileView();
        auto model = qobject_cast<QFileSystemModel *>(view->model()); QVERIFY(model);
        const auto index = model->index(path);
        QTRY_VERIFY(index.isValid());
        view->scrollTo(index);
        QTRY_VERIFY(!view->visualRect(index).isEmpty());
        QTest::mouseClick(view->viewport(), Qt::LeftButton, modifiers, view->visualRect(index).center());
    }
    void acceptByPointer() {
        QVERIFY(picker());
        auto box = picker()->findChild<QDialogButtonBox *>(); QVERIFY(box);
        QPushButton *button = nullptr;
        for (auto candidate : box->buttons())
            if (box->buttonRole(candidate) == QDialogButtonBox::AcceptRole)
                button = qobject_cast<QPushButton *>(candidate);
        QVERIFY(button && button->isEnabled());
        QTest::mouseClick(button, Qt::LeftButton);
    }
private slots:
    void init() {
        QTest::failOnWarning(QRegularExpression("TypeError|ReferenceError|Binding loop"));
    }
    void initTestCase() {
        QVERIFY(qobject_cast<QApplication *>(QCoreApplication::instance()));
        QVERIFY(directory.isValid());
        first = directory.filePath(QString::fromUtf8("résumé + 100% #?.txt"));
        second = directory.filePath("zeta.txt");
        for (const auto &path : {first, second}) {
            QFile file(path); QVERIFY(file.open(QIODevice::WriteOnly)); file.write("Disposable fixture");
        }
        host.resize(800, 600); host.show();
    }
    void cleanup() { QTRY_VERIFY(picker() == nullptr); }
    void successiveRequestsHaveNoRetainedSelection() {
        for (const auto &path : {first, second}) {
            FerricNativeFileChooser chooser;
            open(chooser, FerricNativeFileChooser::OpenFile);
            if (QTest::currentTestFailed()) return;
            QSignalSpy accepted(&chooser, &FerricNativeFileChooser::accepted);
            QVERIFY(!chooser.selectedFiles().contains(QUrl::fromLocalFile(first)));
            choose(path); if (QTest::currentTestFailed()) return;
            acceptByPointer(); if (QTest::currentTestFailed()) return;
            QTRY_COMPARE(accepted.size(), 1);
            QCOMPARE(chooser.selectedFiles(), QList<QUrl>{QUrl::fromLocalFile(path)});
            QVERIFY(!chooser.visible()); QVERIFY(picker() == nullptr);
        }
    }
    void multipleFilesSelectedByPointer() {
        FerricNativeFileChooser chooser;
        open(chooser, FerricNativeFileChooser::OpenFiles);
        if (QTest::currentTestFailed()) return;
        QSignalSpy accepted(&chooser, &FerricNativeFileChooser::accepted);
        choose(first); if (QTest::currentTestFailed()) return;
        choose(second, Qt::ControlModifier); if (QTest::currentTestFailed()) return;
        acceptByPointer(); if (QTest::currentTestFailed()) return;
        QTRY_COMPARE(accepted.size(), 1);
        QCOMPARE(chooser.selectedFiles().size(), 2);
        QVERIFY(chooser.selectedFiles().contains(QUrl::fromLocalFile(first)));
        QVERIFY(chooser.selectedFiles().contains(QUrl::fromLocalFile(second)));
    }
    void saveSuggestionPreservesLiteralCharacters() {
        FerricNativeFileChooser chooser;
        const auto path = directory.filePath(QString::fromUtf8("new résumé + %2F#?.txt"));
        chooser.setSelectedFile(QUrl::fromLocalFile(path));
        open(chooser, FerricNativeFileChooser::SaveFile);
        if (QTest::currentTestFailed()) return;
        QSignalSpy accepted(&chooser, &FerricNativeFileChooser::accepted);
        acceptByPointer(); if (QTest::currentTestFailed()) return;
        QTRY_COMPARE(accepted.size(), 1);
        QCOMPARE(chooser.selectedFile(), QUrl::fromLocalFile(path));
    }
    void localUrlPolicyMatchesBrowserConversions() {
        FerricNativeFileChooser chooser;
        auto local = QUrl::fromLocalFile(first);
        local.setHost("localhost");
        chooser.setSelectedFile(local);
        QCOMPARE(chooser.selectedFile(), QUrl::fromLocalFile(first));
        const auto accepted = chooser.selectedFile();
        for (const auto &invalid : {QUrl("https://example.test/file"), QUrl("file://server/tmp/file"),
                                   QUrl("file:relative.txt"), QUrl("file:///tmp/file?query"),
                                   QUrl("file:///tmp/file#fragment")}) {
            chooser.setSelectedFile(invalid);
            QCOMPARE(chooser.selectedFile(), accepted);
        }
    }
    void saveChooserAcceptsAndCancelsByKeyboard() {
        FerricNativeFileChooser chooser;
        open(chooser, FerricNativeFileChooser::SaveFile);
        if (QTest::currentTestFailed()) return;
        auto filename=picker()->findChild<QLineEdit *>("fileNameEdit"); QVERIFY(filename);
        QVERIFY(filename->isVisible());
        filename->setFocus();
        QTest::keyClick(filename,Qt::Key_A,Qt::ControlModifier);
        QTest::keyClicks(filename,"keyboard-save.txt");
        QSignalSpy accepted(&chooser,&FerricNativeFileChooser::accepted);
        QSignalSpy rejected(&chooser,&FerricNativeFileChooser::rejected);
        QTest::keyClick(filename,Qt::Key_Return);
        QTRY_COMPARE(accepted.size(),1);
        QCOMPARE(chooser.selectedFile(),QUrl::fromLocalFile(directory.filePath("keyboard-save.txt")));
        chooser.setSelectedFile({});
        chooser.open(); QVERIFY(picker());
        QTest::keyClick(picker(),Qt::Key_Escape);
        QTRY_COMPARE(rejected.size(),1);
        QCOMPARE(accepted.size(),1);
        QVERIFY(!chooser.visible());
    }
    void completionReturnsActivationToOwner_data() {
        QTest::addColumn<bool>("accept");
        QTest::newRow("accept") << true;
        QTest::newRow("cancel") << false;
    }
    void completionReturnsActivationToOwner() {
        QFETCH(bool, accept);
        host.requestActivate(); QTRY_VERIFY(host.isActive());
        FerricNativeFileChooser chooser;
        open(chooser, FerricNativeFileChooser::OpenFile);
        if (QTest::currentTestFailed()) return;
        picker()->activateWindow(); QTRY_VERIFY(picker()->isActiveWindow());
        QSignalSpy finished(&chooser, accept ? &FerricNativeFileChooser::accepted : &FerricNativeFileChooser::rejected);
        if (accept) {
            choose(first); if (QTest::currentTestFailed()) return;
            acceptByPointer();
        } else {
            QTest::keyClick(picker(), Qt::Key_Escape);
        }
        QTRY_COMPARE(finished.size(), 1);
        QTRY_VERIFY(host.isActive());
    }
    void closingInBackgroundDoesNotActivateOwner() {
        FerricNativeFileChooser chooser;
        open(chooser, FerricNativeFileChooser::OpenFile);
        if (QTest::currentTestFailed()) return;
        QQuickWindow other;
        other.resize(640, 480); other.show(); other.requestActivate();
        QTRY_VERIFY(other.isActive());
        QSignalSpy rejected(&chooser, &FerricNativeFileChooser::rejected);
        chooser.close();
        QTRY_COMPARE(rejected.size(), 1);
        QVERIFY(other.isActive());
        QVERIFY(!host.isActive());
    }
    void configuredAppearance_data() {
        QTest::addColumn<bool>("dark");
        QTest::addColumn<int>("points");
        QTest::newRow("dark") << true << 10;
        QTest::newRow("dark-large") << true << 20;
        QTest::newRow("light") << false << 10;
        QTest::newRow("light-large") << false << 20;
    }
    void configuredAppearance() {
        QFETCH(bool, dark);
        QFETCH(int, points);
        QQmlApplicationEngine engine;
        QQmlComponent component(&engine);
        component.setData(
            "import QtQuick; import QtQuick.Controls\n"
            "ApplicationWindow {\n"
            "width: 640; height: 480; visible: true\n"
            "property bool darkTheme: true\n"
            "font.family: \"monospace\"\n"
            "palette.window: darkTheme ? \"#1e1e2e\" : \"#eff1f5\"\n"
            "palette.windowText: darkTheme ? \"#cdd6f4\" : \"#4c4f69\"\n"
            "palette.base: darkTheme ? \"#313244\" : \"#ffffff\"\n"
            "palette.text: darkTheme ? \"#cdd6f4\" : \"#4c4f69\"\n"
            "palette.button: darkTheme ? \"#313244\" : \"#ffffff\"\n"
            "palette.buttonText: darkTheme ? \"#cdd6f4\" : \"#4c4f69\"\n"
            "palette.highlight: \"#89b4fa\"\n"
            "palette.highlightedText: \"#11111b\"\n"
            "palette.disabled.buttonText: \"#808080\"\n"
            "palette.inactive.highlight: \"#717f9a\"\n"
            "}\n", QUrl());
        std::unique_ptr<QObject> owner(component.create());
        QVERIFY2(owner, qPrintable(component.errorString()));
        QVERIFY(owner->setProperty("darkTheme", dark));
        QVERIFY(QQmlProperty::write(owner.get(), "font.pointSize", points));
        // Set group overrides after the theme binding has resolved its roles.
        QVERIFY(QQmlProperty::write(owner.get(), "palette.disabled.buttonText", QColor("#808080")));
        QVERIFY(QQmlProperty::write(owner.get(), "palette.inactive.highlight", QColor("#717f9a")));
        QCOMPARE(QQmlProperty::read(owner.get(), "palette.disabled.buttonText").value<QColor>(), QColor("#808080"));
        auto window = qobject_cast<QWindow *>(owner.get()); QVERIFY(window);
        FerricNativeFileChooser chooser;
        chooser.setObjectName("chooserUnderTest");
        chooser.setParentWindow(window);
        chooser.setOptions(QFileDialog::DontUseNativeDialog);
        chooser.setCurrentFolder(QUrl::fromLocalFile(directory.path()));
        chooser.open(); QVERIFY(picker());
        QCOMPARE(picker()->font().pointSize(), points);
        for (const auto group : {QPalette::Active, QPalette::Inactive, QPalette::Disabled}) {
            QCOMPARE(picker()->palette().color(group, QPalette::Window), QColor(dark ? "#1e1e2e" : "#eff1f5"));
            QCOMPARE(picker()->palette().color(group, QPalette::WindowText), QColor(dark ? "#cdd6f4" : "#4c4f69"));
            QCOMPARE(picker()->palette().color(group, QPalette::Base), QColor(dark ? "#313244" : "#ffffff"));
        }
        QCOMPARE(picker()->palette().color(QPalette::Disabled, QPalette::ButtonText), QColor("#808080"));
        QCOMPARE(picker()->palette().color(QPalette::Inactive, QPalette::Highlight), QColor("#717f9a"));
        QTRY_VERIFY(fileView() && fileView()->model()->rowCount(fileView()->rootIndex()) >= 2);
        const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
        if (!artifacts.isEmpty()) {
            QVERIFY(QDir().mkpath(artifacts));
            QVERIFY(picker()->grab().save(artifacts + QString("/chooser-%1-%2pt.png").arg(dark ? "dark" : "light").arg(points)));
        }
        chooser.close();
        QTRY_VERIFY(picker() == nullptr);
    }
    void cancellationResolvesOnceAndDestroysBackend() {
        FerricNativeFileChooser chooser;
        open(chooser, FerricNativeFileChooser::OpenFile);
        QSignalSpy rejected(&chooser, &FerricNativeFileChooser::rejected);
        QPointer<QFileDialog> before = picker();
        chooser.close(); chooser.close();
        QTRY_COMPARE(rejected.size(), 1);
        QVERIFY(before.isNull()); QVERIFY(!chooser.visible());
        QVERIFY(chooser.selectedFiles().isEmpty());
    }
    void completionCanDestroyController() {
        auto chooser = std::make_unique<FerricNativeFileChooser>();
        open(*chooser, FerricNativeFileChooser::OpenFile);
        bool accepted = false;
        connect(chooser.get(), &FerricNativeFileChooser::accepted, this, [&] {
            accepted = true; chooser.reset();
        });
        choose(first); if (QTest::currentTestFailed()) return;
        acceptByPointer(); if (QTest::currentTestFailed()) return;
        QTRY_VERIFY(accepted);
        QVERIFY(!chooser); QVERIFY(picker() == nullptr);
    }
    void ownerAndEngineTeardownDoNotLeaveChoosers() {
        const auto before = QApplication::topLevelWidgets().size();
        for (int iteration = 0; iteration < 8; ++iteration) {
            auto engine = std::make_unique<QQmlApplicationEngine>();
            QQmlComponent component(engine.get());
            component.setData("import QtQuick; Window { visible:true; width:640; height:480 }", QUrl());
            std::unique_ptr<QObject> owner(component.create());
            QVERIFY2(owner, qPrintable(component.errorString()));
            auto window = qobject_cast<QWindow *>(owner.get()); QVERIFY(window);
            FerricNativeFileChooser chooser;
            chooser.setParentWindow(window);
            chooser.setObjectName("chooserUnderTest");
            chooser.setOptions(QFileDialog::DontUseNativeDialog);
            chooser.open(); QVERIFY(chooser.visible());
            QSignalSpy rejected(&chooser, &FerricNativeFileChooser::rejected);
            owner.reset(); engine.reset();
            QTRY_COMPARE(rejected.size(), 1);
            QVERIFY(!chooser.visible());
            QCOMPARE(QApplication::topLevelWidgets().size(), before);
        }
    }
};

int main(int argc, char **argv) {
    QVector<QByteArray> arguments;
    for (int index = 0; index < argc; ++index) arguments.append(QByteArray(argv[index]));
    auto application = ferric_browser_gui_application(arguments);
    FileChooserBackendTests tests;
    return QTest::qExec(&tests, argc, argv);
}
#include "tst_file_chooser_backend.moc"
