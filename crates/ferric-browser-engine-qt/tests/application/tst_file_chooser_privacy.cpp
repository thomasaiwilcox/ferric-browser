#include "../../src/gui_application.h"
#include "../../src/native_file_chooser.h"
#include <QApplication>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QProcess>
#include <QQuickWindow>
#include <QSettings>
#include <QTemporaryDir>
#include <QTextStream>
#include <QtTest>

namespace {
QString hostPreferenceFile;
QByteArray hostPreferenceContents;
QFileDialog *picker() {
    for (auto widget : QApplication::topLevelWidgets())
        if (auto dialog = qobject_cast<QFileDialog *>(widget)) return dialog;
    return nullptr;
}
}

class FileChooserPrivacyTests : public QObject {
    Q_OBJECT
    QTemporaryDir privateRoot{QDir::tempPath() + "/private-history-sentinel-XXXXXX"};
    QQuickWindow owner;
    void prepare(FerricNativeFileChooser &chooser, QWindow *host = nullptr) {
        chooser.setParentWindow(host ? host : &owner);
        chooser.setOptions(QFileDialog::DontUseNativeDialog);
        chooser.setCurrentFolder(QUrl::fromLocalFile(privateRoot.path()));
        chooser.open();
        QVERIFY(picker());
        picker()->setHistory({privateRoot.filePath("private-prior-folder")});
        picker()->setSidebarUrls({QUrl::fromLocalFile(privateRoot.path())});
    }
    void noPrivatePreference() {
        QSettings state(QSettings::UserScope, "QtProject");
        state.sync();
        QCOMPARE(state.status(), QSettings::NoError);
        for (const auto &key : state.allKeys()) {
            const auto value = state.value(key);
            QVERIFY2(!value.toString().contains("private-history-sentinel"), qPrintable(key));
            for (const auto &entry : value.toStringList())
                QVERIFY2(!entry.contains("private-history-sentinel"), qPrintable(key));
        }
        QFile file(state.fileName());
        if (file.exists()) {
            QVERIFY(file.open(QIODevice::ReadOnly));
            QVERIFY(!file.readAll().contains("private-history-sentinel"));
        }
    }
private slots:
    void initTestCase() {
        QVERIFY(privateRoot.isValid());
        QVERIFY(ferric_browser_gui_preferences_ready());
        owner.resize(800, 600); owner.show();
    }
    void cleanup() { QTRY_VERIFY(picker() == nullptr); }
    void cancellationDoesNotSaveVisitedFolders() {
        FerricNativeFileChooser chooser; prepare(chooser);
        QSignalSpy rejected(&chooser, &FerricNativeFileChooser::rejected);
        chooser.close(); QTRY_COMPARE(rejected.size(), 1);
        noPrivatePreference();
    }
    void acceptanceDoesNotSaveVisitedFolders() {
        const auto path = privateRoot.filePath("private-file.txt");
        QFile file(path); QVERIFY(file.open(QIODevice::WriteOnly)); file.write("Fixture"); file.close();
        FerricNativeFileChooser chooser;
        chooser.setSelectedFile(QUrl::fromLocalFile(path));
        prepare(chooser);
        QSignalSpy accepted(&chooser, &FerricNativeFileChooser::accepted);
        chooser.accept(); QTRY_COMPARE(accepted.size(), 1);
        QCOMPARE(chooser.selectedFile(), QUrl::fromLocalFile(path));
        // The selected folder remains available to this request's owner in
        // memory, while Qt's saved preferences contain no browsing location.
        QCOMPARE(chooser.currentFolder(), QUrl::fromLocalFile(privateRoot.path()));
        noPrivatePreference();
    }
    void controllerDestructionDoesNotSaveVisitedFolders() {
        auto chooser = std::make_unique<FerricNativeFileChooser>(); prepare(*chooser);
        chooser.reset(); QVERIFY(picker() == nullptr);
        noPrivatePreference();
    }
    void ownerDestructionDoesNotSaveVisitedFolders() {
        auto host = std::make_unique<QQuickWindow>(); host->resize(640, 480); host->show();
        FerricNativeFileChooser chooser; prepare(chooser, host.get());
        QSignalSpy rejected(&chooser, &FerricNativeFileChooser::rejected);
        host.reset(); QTRY_COMPARE(rejected.size(), 1);
        noPrivatePreference();
    }
    void nextRequestDoesNotInheritPriorFolders() {
        { FerricNativeFileChooser chooser; prepare(chooser); chooser.close(); QTRY_VERIFY(picker() == nullptr); }
        FerricNativeFileChooser next;
        next.setParentWindow(&owner);
        next.setOptions(QFileDialog::DontUseNativeDialog);
        next.open(); QVERIFY(picker());
        QCOMPARE(picker()->directory().absolutePath(), QDir::homePath());
        QVERIFY(!picker()->history().join(' ').contains("private-history-sentinel"));
        for (const auto &url : picker()->sidebarUrls())
            QVERIFY(!url.toString().contains("private-history-sentinel"));
        next.close(); QTRY_VERIFY(picker() == nullptr);
        noPrivatePreference();
    }
    void hardTerminationDoesNotRetainPrivateFolders() {
        QProcess child;
        child.start(QCoreApplication::applicationFilePath(), {"--privacy-probe-active", privateRoot.path()});
        QVERIFY(child.waitForStarted(5000));
        QByteArray output;
        QElapsedTimer deadline; deadline.start();
        while (!output.contains("ACTIVE_READY\n") && deadline.elapsed() < 10000) {
            child.waitForReadyRead(200);
            output += child.readAllStandardOutput();
            if (child.state() == QProcess::NotRunning) break;
        }
        QVERIFY2(output.contains("ACTIVE_READY\n"), child.readAllStandardError().constData());
        QString root;
        for (const auto &line : output.split('\n'))
            if (line.startsWith("PREFERENCE_ROOT ")) root = QString::fromUtf8(line.mid(16));
        QVERIFY(root.startsWith(QDir::tempPath() + "/ferric-ui-"));
        QVERIFY(QFileInfo(root).isDir());
        // The child first closed one chooser, then opened another. A real
        // preference file must exist; absence of output alone is insufficient.
        QVERIFY(QFileInfo::exists(root + "/QtProject.conf"));
        child.kill(); QVERIFY(child.waitForFinished(5000));
        QCOMPARE(child.exitStatus(), QProcess::CrashExit);
        bool containsPrivatePath = false;
        QDirIterator files(root, QDir::Files | QDir::Hidden | QDir::System | QDir::NoDotAndDotDot,
                           QDirIterator::Subdirectories);
        while (files.hasNext()) {
            QFile file(files.next());
            if (!file.open(QIODevice::ReadOnly)) containsPrivatePath = true;
            else if (file.readAll().contains("private-history-sentinel")) containsPrivatePath = true;
        }
        // This directory belongs to our terminated fixture, not the browser
        // under development or a desktop application.
        QVERIFY(QDir(root).removeRecursively());
        QVERIFY(!containsPrivatePath);
    }
    void hostPreferencesRemainUntouched() {
        QFile file(hostPreferenceFile); QVERIFY(file.open(QIODevice::ReadOnly));
        QCOMPARE(file.readAll(), hostPreferenceContents);
    }
};

int main(int argc, char **argv) {
    const bool probe = argc >= 3 && QByteArray(argv[1]) == "--privacy-probe-active";
    std::unique_ptr<QTemporaryDir> host;
    if (!probe) {
        host = std::make_unique<QTemporaryDir>(); if (!host->isValid()) return 2;
        for (const auto &category : {"config", "cache", "data"}) {
            const auto path = host->filePath(category);
            if (!QDir().mkpath(path)) return 2;
            qputenv((QByteArray("XDG_") + QByteArray(category).toUpper() + "_HOME").constData(), path.toUtf8());
        }
        hostPreferenceFile = host->filePath("config/QtProject.conf");
        hostPreferenceContents = "[FileDialog]\nhistory=file:///host-preferences-sentinel\nlastVisited=file:///host-preferences-sentinel\n";
        QFile file(hostPreferenceFile); if (!file.open(QIODevice::WriteOnly)) return 2;
        if (file.write(hostPreferenceContents) != hostPreferenceContents.size()) return 2;
    }
    QVector<QByteArray> arguments;
    for (int index = 0; index < argc; ++index) arguments.append(QByteArray(argv[index]));
    auto application = ferric_browser_gui_application(arguments); if (!application) return 2;
    QSettings preferences(QSettings::UserScope, "QtProject");
    const auto root = QFileInfo(preferences.fileName()).absolutePath();
    if (probe) {
        QQuickWindow owner; owner.resize(640, 480); owner.show();
        {
            FerricNativeFileChooser prior; prior.setParentWindow(&owner);
            prior.setOptions(QFileDialog::DontUseNativeDialog);
            prior.setCurrentFolder(QUrl::fromLocalFile(QString::fromLocal8Bit(argv[2])));
            prior.open(); prior.close();
            QElapsedTimer deadline; deadline.start();
            while (picker() && deadline.elapsed() < 5000) QCoreApplication::processEvents();
            if (picker()) return 3;
        }
        FerricNativeFileChooser chooser; chooser.setParentWindow(&owner);
        chooser.setOptions(QFileDialog::DontUseNativeDialog);
        chooser.setCurrentFolder(QUrl::fromLocalFile(QString::fromLocal8Bit(argv[2])));
        chooser.open(); if (!chooser.visible()) return 3;
        QTextStream(stdout) << "PREFERENCE_ROOT " << root << "\nACTIVE_READY" << Qt::endl;
        return application->exec();
    }
    int code;
    { FileChooserPrivacyTests tests; code = QTest::qExec(&tests, argc, argv); }
    application.reset();
    if (QFileInfo::exists(root)) { qCritical() << "Disposable UI preference directory was retained"; return 4; }
    QFile file(hostPreferenceFile);
    if (!file.open(QIODevice::ReadOnly) || file.readAll() != hostPreferenceContents) return 5;
    return code;
}
#include "tst_file_chooser_privacy.moc"
