#include "tst_application.h"
#include "owned_renderer_process.h"
#include <QFile>
#include <QFileInfo>
#include <QPointer>
#include <QRegularExpression>
#include <QGuiApplication>
#include <QApplication>
#include <QAbstractItemView>
#include <QFileDialog>
#include <QFileSystemModel>
#include <QDialogButtonBox>
#include <QPushButton>
#include <QDir>
#include <QFont>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QUuid>
#include <QQmlComponent>
#include <QQmlContext>
#include <QQmlProperty>
#include <QQuickItem>
#include <QUrl>
#include <QtTest>
#include <csignal>
#include <unistd.h>

namespace {
QObject *visibleEngineFileChooser(bool folder = false) {
    for (auto candidate : QGuiApplication::allWindows()) {
        for (auto dialog : candidate->findChildren<QObject *>(folder ? "engineFolderChooser" : "engineFileChooser"))
            if (dialog->property("visible").toBool()) return dialog;
    }
    return nullptr;
}
QFileDialog *visiblePicker(bool folder = false) {
    for (auto candidate : QApplication::topLevelWidgets())
        if (auto dialog = qobject_cast<QFileDialog *>(candidate))
            if (dialog->isVisible() && dialog->objectName() == (folder ? "engineFolderChooser" : "engineFileChooser")) return dialog;
    return nullptr;
}
QAbstractItemView *visiblePickerList(bool folder = false) {
    auto picker = visiblePicker(folder);
    if (!picker) return nullptr;
    for (auto view : picker->findChildren<QAbstractItemView *>())
        if (view->isVisible() && qobject_cast<QFileSystemModel *>(view->model())) return view;
    return nullptr;
}
void selectPickerFile(const QUrl &file, Qt::KeyboardModifiers modifiers = Qt::NoModifier) {
    QTRY_VERIFY_WITH_TIMEOUT(visiblePickerList()!=nullptr,5000);
    auto list=visiblePickerList();
    auto model=qobject_cast<QFileSystemModel *>(list->model()); QVERIFY(model);
    auto row=model->index(file.toLocalFile()); QTRY_VERIFY(row.isValid());
    list->scrollTo(row); QTRY_VERIFY(!list->visualRect(row).isEmpty());
    visiblePicker()->activateWindow();
    QTest::mouseClick(list->viewport(),Qt::LeftButton,modifiers,list->visualRect(row).center());
    QTRY_VERIFY(visiblePicker()->selectedUrls().contains(file));
}
void acceptPickerByPointer(bool folder = false) {
    auto picker=visiblePicker(folder); QVERIFY(picker);
    auto buttons=picker->findChild<QDialogButtonBox *>(); QVERIFY(buttons);
    QPushButton *openButton=nullptr;
    for (auto button : buttons->buttons())
        if (buttons->buttonRole(button)==QDialogButtonBox::AcceptRole)
            openButton=qobject_cast<QPushButton *>(button);
    QVERIFY(openButton); QTRY_VERIFY(openButton->isEnabled());
    QTest::mouseClick(openButton,Qt::LeftButton);
}
}

void ApplicationTests::productPolishFileChooser_data() {
    QTest::addColumn<QString>("kind");
    QTest::addColumn<QString>("ending");
    for (const auto &kind : {"primary","secondary","private","popup"}) {
        for (const auto &ending : {"navigation","renderer-loss","owner-close","backend-failure"}) {
            if (QString::fromLatin1(kind)=="primary" && QString::fromLatin1(ending)=="owner-close") continue;
            QTest::newRow((QByteArray(kind)+"-"+ending).constData())
                << QString::fromLatin1(kind) << QString::fromLatin1(ending);
        }
    }
}

void ApplicationTests::productPolishFileChooser() {
    QFETCH(QString,kind);
    QFETCH(QString,ending);
    QTest::failOnWarning(QRegularExpression("TypeError|ReferenceError|Binding loop"));
    QString owner = "testWindow.browserWindowRegistry[0]";
    const auto url = text("current_url");
    if (kind == "secondary" || kind == "private") {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"+quoted(url)
            +",windowProfileName:'file-owner',windowProfileLabel:'File owner',windowPrivateProfile:"
            +(kind=="private" ? "true" : "false")+"})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        owner = "testWindow.browserWindowRegistry[1]";
    } else if (kind == "popup") {
        const auto point=page("(()=>{const r=document.getElementById('blank').getBoundingClientRect();return [r.x+r.width/2,r.y+r.height/2]})()").toList();
        auto opener=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>()); QVERIFY(opener);
        QTest::mouseClick(window,Qt::LeftButton,Qt::NoModifier,
            opener->mapToScene(QPointF(point[0].toDouble(),point[1].toDouble())).toPoint());
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(),1);
        owner = "testWindow.popupWindowRegistry[0]";
    }
    const auto hostExpression=owner+".host";
    const auto ui=owner+".ui";
    pageReceiver=owner+".view";
    QTRY_VERIFY_WITH_TIMEOUT(evaluate(hostExpression+".profileBootstrapComplete && !"+ui
        +".profile_bootstrap_pending && !!"+pageReceiver+" && !"+pageReceiver+".loading").toBool(),15000);
    QTRY_VERIFY_WITH_TIMEOUT(page("document.readyState==='complete' && !!document.getElementById('button')").toBool(),15000);
    auto host=qobject_cast<QQuickWindow *>(evaluate(hostExpression).value<QObject *>()); QVERIFY(host);
    auto view=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>()); QVERIFY(view);
    const auto initialMode=evaluate(ui+".mode").toString();
    page(R"JS((()=>{const input=document.createElement('input');input.type='file';input.id='owned-file';
        input.style='position:fixed;left:20px;top:20px;width:300px;height:45px;z-index:2147483647';
        document.body.prepend(input);return true})())JS");
    const auto openPicker=[&](bool folder = false) {
        host->requestActivate();
        QTRY_VERIFY(host->isActive());
        view->forceActiveFocus();
        const auto point=page("(()=>{const r=document.getElementById('owned-file').getBoundingClientRect();return [r.x+r.width/2,r.y+r.height/2]})()").toList();
        QSignalSpy rendered(host,&QQuickWindow::frameSwapped);
        host->update(); QVERIFY(rendered.wait(2000));
        QTest::mouseClick(host,Qt::LeftButton,Qt::NoModifier,
            view->mapToScene(QPointF(point[0].toDouble(),point[1].toDouble())).toPoint());
        QTRY_VERIFY(visibleEngineFileChooser(folder)!=nullptr);
    };
    const auto verifyOwner=[&](bool folder = false) {
        auto dialog=visibleEngineFileChooser(folder); QVERIFY(dialog);
        // Render only the disposable fixture directory, never the user's
        // personal home-directory listing.
        QVERIFY(dialog->setProperty("currentFolder",QUrl::fromLocalFile(directory.path())));
        QVERIFY2(evaluate(hostExpression+".pendingFileDialogRequest!==null").toBool(),
            "The file chooser request belongs to another window");
        QTRY_VERIFY(visiblePickerList(folder)!=nullptr);
        auto pickerWindow=visiblePicker(folder)->windowHandle();
        QVERIFY(pickerWindow && pickerWindow->transientParent()==host);
        QCOMPARE(visiblePicker(folder)->font(),host->property("font").value<QFont>());
        QCOMPARE(visiblePicker(folder)->palette().color(QPalette::Active,QPalette::Window),
            QQmlProperty::read(host,"palette.active.window").value<QColor>());
        QCOMPARE(visiblePicker(folder)->palette().color(QPalette::Active,QPalette::WindowText),
            QQmlProperty::read(host,"palette.active.windowText").value<QColor>());
        QVERIFY(evaluate(ui+".has_native_prompt_lease()").toBool());
        const auto artifacts=qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
        if (!artifacts.isEmpty()) {
            QDir().mkpath(artifacts);
            qInfo()<<"chooser appearance"<<kind<<ending
                <<"host font"<<host->property("font")
                <<"host palette type"<<host->property("palette").typeName()
                <<"chooser font"<<visiblePicker(folder)->font().pointSizeF();
            QVERIFY(visiblePicker(folder)->grab().save(artifacts+"/chooser-"+kind+"-"+ending+(folder ? "-folder" : "")+".png"));
            QVERIFY(host->grabWindow().save(artifacts+"/chooser-host-"+kind+"-"+ending+".png"));
        }
    };
    const auto verifyReturnedFocus=[&] {
        // No activation request here: dismissing the chooser must itself
        // return keyboard input to the page that opened it.
        QTRY_VERIFY(host->isActive());
        QTRY_VERIFY(view->hasActiveFocus());
    };
    openPicker(); if (QTest::currentTestFailed()) return;
    verifyOwner(); if (QTest::currentTestFailed()) return;
    QVERIFY(QMetaObject::invokeMethod(visibleEngineFileChooser(),"reject"));
    QTRY_VERIFY(evaluate(hostExpression+".pendingFileDialogRequest===null").toBool());
    QTRY_VERIFY(!evaluate(ui+".has_native_prompt_lease()").toBool());
    QCOMPARE(page("document.getElementById('owned-file').files.length").toInt(),0);
    verifyReturnedFocus(); if (QTest::currentTestFailed()) return;
    const auto first=QUrl::fromLocalFile(directory.filePath(QString::fromUtf8("résumé + 100% #?.txt")));
    const auto second=QUrl::fromLocalFile(directory.filePath("zeta.txt"));
    for (const auto &fileUrl : {first,second}) {
        QFile file(fileUrl.toLocalFile()); QVERIFY(file.open(QIODevice::WriteOnly)); file.write("Upload fixture");
    }
    QQmlComponent incomingFixture(engine.get());
    incomingFixture.setData(R"QML(import QtQml; QtObject {
        property bool accepted: false
        property int rejections: 0
        function dialogReject() { rejections++ }
    })QML",QUrl());
    std::unique_ptr<QObject> incoming(incomingFixture.create());
    QVERIFY2(incoming,qPrintable(incomingFixture.errorString()));
    engine->rootContext()->setContextProperty("incomingFileRequest",incoming.get());
    for (const auto &desired : {first,second}) {
        page("document.getElementById('owned-file').value='';true");
        openPicker(); if (QTest::currentTestFailed()) return;
        verifyOwner(); if (QTest::currentTestFailed()) return;
        auto chooser=visibleEngineFileChooser(); QVERIFY(chooser);
        if (desired==second) {
            QCOMPARE(evaluate(hostExpression+".pendingFileDialogRequest.defaultFileName").toString(),QString());
            QVERIFY2(chooser->property("selectedFile").toUrl()!=first,"Empty default retained the previous file");
            const auto pending=evaluate(hostExpression+".pendingFileDialogRequest").value<QObject *>();
            evaluate(hostExpression+".handleFileDialogRequested(incomingFileRequest,"+pageReceiver+","+ui+")");
            QCOMPARE(incoming->property("rejections").toInt(),1);
            QCOMPARE(evaluate(hostExpression+".pendingFileDialogRequest").value<QObject *>(),pending);
            QVERIFY(chooser->property("visible").toBool());
        }
        QVERIFY(chooser->setProperty("currentFolder",QUrl::fromLocalFile(directory.path())));
        selectPickerFile(desired); if (QTest::currentTestFailed()) return;
        QTRY_COMPARE(visiblePicker()->selectedUrls(),QList<QUrl>{desired});
        acceptPickerByPointer(); if (QTest::currentTestFailed()) return;
        QTRY_VERIFY(evaluate(hostExpression+".pendingFileDialogRequest===null").toBool());
        QTRY_COMPARE(page("document.getElementById('owned-file').files.length").toInt(),1);
        QCOMPARE(page("document.getElementById('owned-file').files[0].name").toString(),QFileInfo(desired.toLocalFile()).fileName());
        QTRY_VERIFY(!evaluate(ui+".has_native_prompt_lease()").toBool());
        QCOMPARE(evaluate(ui+".mode").toString(),initialMode);
        verifyReturnedFocus(); if (QTest::currentTestFailed()) return;
    }
    page("(()=>{const input=document.getElementById('owned-file');input.value='';input.multiple=true;return true})()");
    openPicker(); if (QTest::currentTestFailed()) return;
    verifyOwner(); if (QTest::currentTestFailed()) return;
    QCOMPARE(visiblePicker()->fileMode(),QFileDialog::ExistingFiles);
    QVERIFY(visibleEngineFileChooser()->setProperty("currentFolder",QUrl::fromLocalFile(directory.path())));
    selectPickerFile(first); if (QTest::currentTestFailed()) return;
    selectPickerFile(second,Qt::ControlModifier); if (QTest::currentTestFailed()) return;
    QCOMPARE(visiblePicker()->selectedUrls().size(),2);
    acceptPickerByPointer(); if (QTest::currentTestFailed()) return;
    QTRY_VERIFY(evaluate(hostExpression+".pendingFileDialogRequest===null").toBool());
    QTRY_COMPARE(page("document.getElementById('owned-file').files.length").toInt(),2);
    const auto names=page("Array.from(document.getElementById('owned-file').files,f=>f.name)").toStringList();
    QVERIFY(names.contains(QFileInfo(first.toLocalFile()).fileName()));
    QVERIFY(names.contains(QFileInfo(second.toLocalFile()).fileName()));
    QTRY_VERIFY(!evaluate(ui+".has_native_prompt_lease()").toBool());

    verifyReturnedFocus(); if (QTest::currentTestFailed()) return;

    const auto uploadFolder=directory.filePath("chooser-directory-upload");
    QVERIFY(QDir().mkpath(uploadFolder));
    for (const auto &name : {"one.txt","two.txt"}) {
        QFile file(QDir(uploadFolder).filePath(name)); QVERIFY(file.open(QIODevice::WriteOnly)); file.write("Folder upload fixture");
    }
    page("(()=>{const input=document.getElementById('owned-file');input.value='';input.multiple=false;input.setAttribute('webkitdirectory','');return true})()");
    openPicker(true); if (QTest::currentTestFailed()) return;
    verifyOwner(true); if (QTest::currentTestFailed()) return;
    QVERIFY(QMetaObject::invokeMethod(visibleEngineFileChooser(true),"reject"));
    QTRY_VERIFY(evaluate(hostExpression+".pendingFileDialogRequest===null").toBool());
    QTRY_VERIFY(!evaluate(ui+".has_native_prompt_lease()").toBool());
    QCOMPARE(page("document.getElementById('owned-file').files.length").toInt(),0);
    verifyReturnedFocus(); if (QTest::currentTestFailed()) return;
    openPicker(true); if (QTest::currentTestFailed()) return;
    verifyOwner(true); if (QTest::currentTestFailed()) return;
    QCOMPARE(visiblePicker(true)->fileMode(),QFileDialog::Directory);
    QVERIFY(visibleEngineFileChooser(true)->setProperty("currentFolder",QUrl::fromLocalFile(uploadFolder)));
    QTRY_COMPARE(visiblePicker(true)->directory().absolutePath(),uploadFolder);
    acceptPickerByPointer(true); if (QTest::currentTestFailed()) return;
    QTRY_VERIFY(evaluate(hostExpression+".pendingFileDialogRequest===null").toBool());
    QTRY_COMPARE(page("document.getElementById('owned-file').files.length").toInt(),2);
    const auto relativePaths=page("Array.from(document.getElementById('owned-file').files,f=>f.webkitRelativePath)").toStringList();
    QVERIFY(relativePaths.contains("chooser-directory-upload/one.txt"));
    QVERIFY(relativePaths.contains("chooser-directory-upload/two.txt"));
    QTRY_VERIFY(!evaluate(ui+".has_native_prompt_lease()").toBool());
    QCOMPARE(evaluate(ui+".mode").toString(),initialMode);
    verifyReturnedFocus(); if (QTest::currentTestFailed()) return;

    page("(()=>{const input=document.getElementById('owned-file');input.value='';input.removeAttribute('webkitdirectory');return true})()");
    openPicker(); if (QTest::currentTestFailed()) return;
    QPointer<QObject> chooser=visibleEngineFileChooser();
    QPointer<QFileDialog> nativePicker=visiblePicker();
    if (ending=="backend-failure") {
        const QString message="The system file chooser could not complete the request. Try again.";
        QVERIFY(QMetaObject::invokeMethod(chooser,"failed",Q_ARG(QString,message)));
        QTRY_VERIFY(evaluate(hostExpression+".pendingFileDialogRequest===null").toBool());
        QTRY_VERIFY(!nativePicker || !nativePicker->isVisible());
        QTRY_VERIFY(!evaluate(ui+".has_native_prompt_lease()").toBool());
        QCOMPARE(evaluate(ui+".status_text").toString(),message);
        QCOMPARE(page("document.getElementById('owned-file').files.length").toInt(),0);
        QCOMPARE(evaluate(ui+".mode").toString(),initialMode);
        return;
    }
    if (ending=="owner-close") {
        evaluate(hostExpression+".windowShutdownApproved=true");
        QVERIFY(host->close());
        QTRY_VERIFY(!nativePicker || !nativePicker->isVisible());
        QTRY_VERIFY(!chooser || !chooser->property("visible").toBool());
        pageReceiver="testWindow.activeWebView()";
        window->requestActivate(); QTRY_VERIFY(window->isActive());
        QTRY_VERIFY(evaluate("testWindow.browserKeyFocusActive").toBool());
        return;
    }
    if (ending=="renderer-loss") {
        const auto pid=evaluate(pageReceiver+".renderProcessPid").toLongLong();
        QVERIFY(pid>1 && pid!=getpid());
        const auto failure = FerricTest::terminateOwnedRenderer(pid);
        QVERIFY2(failure.isEmpty(), qPrintable(failure));
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.rendererFailureVisible").toBool(),10000);
        QTRY_VERIFY(evaluate(hostExpression+".pendingFileDialogRequest===null").toBool());
        QTRY_VERIFY(!nativePicker || !nativePicker->isVisible());
        QTRY_VERIFY(!evaluate(ui+".has_native_prompt_lease()").toBool());
        evaluate("testWindow.reloadRendererFailure()");
        QTRY_VERIFY_WITH_TIMEOUT(evaluate(pageReceiver+".renderProcessPid").toLongLong()>1
            && evaluate(pageReceiver+".renderProcessPid").toLongLong()!=pid,15000);
        QTRY_VERIFY_WITH_TIMEOUT(page("document.getElementById('button')!==null").toBool(),15000);
        return;
    }
    const auto next=page("location.href").toString()+"?picker-navigation";
    if (kind=="primary") QVERIFY(command("open "+next));
    else QVERIFY(evaluate(ui+".execute_secondary_interactive_command("+quoted("open "+next)+")").toBool());
    QTRY_VERIFY(evaluate(hostExpression+".pendingFileDialogRequest===null").toBool());
    QTRY_VERIFY(!chooser || !chooser->property("visible").toBool());
    QTRY_VERIFY(!evaluate(ui+".has_native_prompt_lease()").toBool());
}

void ApplicationTests::productPolishDownloadSelectionFailure() {
    // Exercise registry completion with an actual live BrowserUi/profile.
    // The native source suite separately qualifies real Qt download offers.
    const auto result=evaluate(R"JS((()=>{
        const registry=testWindow.profileResourceRegistry;
        const original=registry.downloadOwners;
        const ui=testWindow.browserUi;
        const profile=testWindow.activeWebView().profile;
        const firstId=ui.allocate_download_id(),secondId=ui.allocate_download_id();
        ui.offer_download(firstId,'https://example.test/first','first.bin');
        ui.offer_download(secondId,'https://example.test/second','second.bin');
        const first={id:'failed-selection',totalBytes:0},second={id:'user-cancelled',totalBytes:0};
        const failedHost={runtimeReleased:false,count:0,failed:false,
            handleDownloadFinished:function(download,failed){this.count++;this.failed=failed;}};
        const cancelledHost={runtimeReleased:false,count:0,failed:false,
            handleDownloadFinished:function(download,failed){this.count++;this.failed=failed;}};
        try {
            registry.downloadOwners=[{download:first,ui:ui,host:failedHost,id:firstId,profile:profile},
                {download:second,ui:ui,host:cancelledHost,id:secondId,profile:profile}];
            registry.markDownloadSelectionFailed(first);
            registry.handleDownloadFinished(null,second);
            const values=[failedHost.count,cancelledHost.count,cancelledHost.failed];
            registry.handleDownloadFinished(null,first);
            values.push(failedHost.count,failedHost.failed,registry.downloadOwners.length);
            registry.handleDownloadFinished(null,first);
            values.push(failedHost.count);
            return values;
        } finally {registry.downloadOwners=original;}
    })())JS").toList();
    QCOMPARE(result,QVariantList({0,1,false,1,true,0,1}));
}

void ApplicationTests::productPolishDownloadIdentity() {
    const auto encoded=evaluate(R"JS((()=>{
        const registry=testWindow.profileResourceRegistry;
        const original=registry.downloadOwners;
        const ui=testWindow.browserUi;
        const profile=testWindow.activeWebView().profile;
        const firstHost={runtimeReleased:false,started:[],finished:[],
            handleDownloadRequested:function(download,id){this.started.push(id);},
            handleDownloadFinished:function(download,failed,id){this.finished.push([id,failed]);}};
        const secondHost={runtimeReleased:false,started:[],finished:[],
            handleDownloadRequested:function(download,id){this.started.push(id);},
            handleDownloadFinished:function(download,failed,id){this.finished.push([id,failed]);}};
        const first={id:1,view:{host:firstHost},url:"https://example.test/first",suggestedFileName:"first.bin",
            isFinished:false,receivedBytes:0,totalBytes:0,state:1, // Qt DownloadInProgress
            cancelled:0,cancel:function(){this.cancelled++;}};
        const second={id:1,view:{host:secondHost},url:"https://example.test/second",suggestedFileName:"second.bin",
            isFinished:false,receivedBytes:0,totalBytes:0,state:1, // Qt DownloadInProgress
            cancelled:0,cancel:function(){this.cancelled++;}};
        const group={profile:profile,lifetime:{ownerUi:function(view){return view ? ui : null;},
            ownerHost:function(view){return view ? view.host : null;}}};
        try {
            registry.downloadOwners=[];
            // Test common bookkeeping without sending a JavaScript object to
            // the native QObject accessor. Real offers are qualified separately.
            registry.admitDownload(group,first,first.view);
            // Already-admitted identity must return without looking up its source.
            registry.handleDownloadRequested(group,first);
            registry.admitDownload(group,second,second.view);
            registry.markDownloadSelectionFailed(first);
            registry.handleDownloadFinished(group,first);
            registry.cancelDownloadsForHost(secondHost);
            registry.handleDownloadFinished(group,second);
            const rows=ui.download_rows();
            return JSON.stringify({allocations:rows.length,first:firstHost.started,
                second:secondHost.started,finished:firstHost.finished,
                cancelled:second.cancelled,updates:rows.filter(function(row){return row.state==='cancelled';}).map(function(row){return row.id;}),
                lateFinished:secondHost.finished.length,remaining:registry.downloadOwners.length});
        } finally {registry.downloadOwners=original;}
    })())JS").toString();
    const auto result=QJsonDocument::fromJson(encoded.toUtf8()).object();
    QCOMPARE(result["allocations"].toInt(),2);
    const auto first=result["first"].toArray();
    const auto second=result["second"].toArray();
    QCOMPARE(first.size(),1);
    QCOMPARE(second.size(),1);
    const auto firstId=first[0].toString(),secondId=second[0].toString();
    QVERIFY(!QUuid(firstId).isNull());
    QVERIFY(!QUuid(secondId).isNull());
    QVERIFY(firstId!=secondId);
    QCOMPARE(result["finished"].toArray(),QJsonArray({QJsonArray({firstId,true})}));
    QCOMPARE(result["updates"].toArray(),QJsonArray({secondId}));
    QCOMPARE(result["cancelled"].toInt(),1);
    QCOMPARE(result["lateFinished"].toInt(),0);
    QCOMPARE(result["remaining"].toInt(),0);
}
