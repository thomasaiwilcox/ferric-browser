import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: capture
    required property var browserWindow
    property var hostWindow: browserWindow
    required property var browserUi
    required property var workspace
    required property var runtime
    property bool opened: false
    property bool busy: false
    property bool hadFailure: false
    property string owner: ""
    property var rows: []
    property string notice: ""
    readonly property bool inputAvailable:!!hostWindow&&hostWindow.browserKeyFocusActive
    onInputAvailableChanged:if(opened&&!inputAvailable)releaseForOtherInput()
    visible: opened
    enabled: !browserUi.profile_bootstrap_pending
    objectName: "sitePolicyCapture"
    anchors.fill: parent
    z:80
    Accessible.role:Accessible.Pane
    Accessible.name:"Capture a target; type a label and Enter, arrows or Tab choose a text equivalent, Escape cancels"
    onOpenedChanged:if(opened)Accessible.announce(Accessible.name)
    onNoticeChanged:if(opened&&notice)Accessible.announce(notice)
    MouseArea { anchors.fill:parent;acceptedButtons:Qt.AllButtons;onWheel:function(event){event.accepted=true} }
    function releaseForOtherInput() {
        notice="Capture closed because another browser control owns input. Request a fresh capture when it is dismissed."
        workspace.notice=notice
        opened=false;rows=[];busy=false;runtime.stop()
    }
    function begin() {
        owner=workspace.profileInstance; rows=[];notice="Collecting checked candidates…";busy=false;hadFailure=false
        workspace.hostWindow.sitePoliciesVisible=false
        if(!inputAvailable){releaseForOtherInput();return}
        opened=true;labelText=""
        var root=hostWindow.rootWindow||hostWindow
        root.restoreOverlayFocus(hostWindow)
        root.runtime_focusOverlayController.contentFocusTarget(runtime.activeView).forceActiveFocus()
    }
    property string labelText:""
    onLabelTextChanged:if(opened){
        var row=rows.find(function(value){return value.label===labelText})
        Accessible.announce(row?row.label+" · "+row.candidate.kind+" · "+row.candidate.text:"Capture label: "+labelText)
    }
    function handlePageKey(text,key,modifiers,repeat) {
        if(key===Qt.Key_Escape){close();return}
        if(repeat||busy)return
        if(key===Qt.Key_Return||key===Qt.Key_Enter){choose(labelText.trim());return}
        if(key===Qt.Key_Backspace){labelText=labelText.slice(0,-1);return}
        if(key===Qt.Key_Tab||key===Qt.Key_Backtab||key===Qt.Key_Down||key===Qt.Key_Up){
            if(!rows.length)return
            rowList.currentIndex=(rowList.currentIndex+(key===Qt.Key_Up||key===Qt.Key_Backtab||(modifiers&Qt.ShiftModifier)?-1:1)+rows.length)%rows.length
            labelText=rows[rowList.currentIndex].label;rowList.positionViewAtIndex(rowList.currentIndex,ListView.Contain);return
        }
        if(!(modifiers&(Qt.ControlModifier|Qt.AltModifier|Qt.MetaModifier))&&/^[\x21-\x7e]$/.test(text)&&labelText.length<8)labelText+=text
    }
    function close() {
        opened=false;rows=[];busy=false
        runtime.stop()
        if (!browserUi.profile_bootstrap_pending && owner===workspace.profileInstance) workspace.show(workspace.contextUrl)
    }
    function choose(value) {
        if (busy || !runtime.captureReady) return
        busy=true
        runtime.chooseCapture(value,function(facts) {
            busy=false
            if(facts.error){notice=facts.error;hadFailure=true;rows=[];return}
            workspace.acceptCapture(facts)
            close()
        })
    }
    property Item markers: FerricHintOverlay {
        parent: capture
        z: 1
        browserWindow: capture.browserWindow
        hintsVisible: capture.opened && capture.runtime.captureReady
        interactionActive: hintsVisible && !capture.busy
        hintResults: capture.rows.map(function(row) { return Object.assign({label:row.label},row.candidate,row.candidate.geometry) })
        sourceViewport: capture.runtime.activeView
        hintState: ({mode:"label",prefix:"",active_label:capture.labelText})
        unmatchedPolicy: "show"
        markerScale: 1
        // Geometry is local to the actual WebEngine viewport, below chrome.
        anchors.fill: undefined
        x: capture.runtime.activeView ? capture.runtime.activeView.mapToItem(capture,0,0).x : 0
        y: capture.runtime.activeView ? capture.runtime.activeView.mapToItem(capture,0,0).y : 0
        width: capture.runtime.activeView ? capture.runtime.activeView.width : 0
        height: capture.runtime.activeView ? capture.runtime.activeView.height : 0
        onActivationRequested: function(value) { capture.choose(value) }
        onActionsRequested: function(value) { capture.choose(value) }
        onSelectionRequested: function(value) { capture.labelText=value }
    }
    Rectangle {
        anchors.right:parent.right;anchors.top:parent.top;anchors.margins:16
        width:Math.min(460,parent.width-32);height:Math.min(500,parent.height-32);z:2
        color:capture.browserWindow.panelColor;border.color:capture.browserWindow.accentColor;border.width:2;radius:8
      ColumnLayout {
        anchors.fill: parent;anchors.margins:12
        Label { Layout.fillWidth:true;text:"Capture a target";font.bold:true }
        Label { Layout.fillWidth:true;wrapMode:Text.Wrap;text:"Type a marker label and Enter. Arrows or Tab choose a text equivalent; Escape cancels. Capture never activates a target or sends keys to the page." }
        Label { Layout.fillWidth: true; wrapMode: Text.Wrap; textFormat: Text.PlainText; text: capture.notice; Accessible.name: text }
        Label { Layout.fillWidth:true;text:"Label: "+capture.labelText;textFormat:Text.PlainText;Accessible.name:text;objectName:"policyCaptureLabel" }
        Button { text: "Inspect selected label";focusPolicy:Qt.NoFocus;enabled: capture.runtime.captureReady && !capture.busy; onClicked: capture.choose(capture.labelText.trim()) }
        ListView {
            id:rowList
            Layout.fillWidth: true; Layout.fillHeight: true; clip: true
            model: capture.rows
            ScrollBar.vertical: ScrollBar {}
            delegate: Button {
                id: rowButton
                required property int index
                required property var modelData
                width: ListView.view.width
                text: modelData.label+" · "+modelData.candidate.kind+" · "+modelData.candidate.text
                Accessible.name: text
                Accessible.focused:rowList.currentIndex===index
                focusPolicy:Qt.NoFocus
                enabled: !capture.busy && capture.runtime.captureReady
                contentItem: Label { text: rowButton.text; textFormat: Text.PlainText; wrapMode: Text.Wrap }
                onClicked: capture.choose(modelData.label)
            }
        }
        Button { text: "Cancel and return to draft";focusPolicy:Qt.NoFocus;objectName: "policyCaptureCancel"; onClicked: capture.close() }
      }
    }
    Connections {
        target: capture.runtime
        function onCaptureStarted(id) { capture.begin() }
        function onCaptureInventory(values) { capture.rows=values;capture.notice=values.length+" checked candidates. Names are shortened here; inspect one for bounded details." }
        function onFailed(reason) { if(capture.opened){capture.notice=reason;capture.hadFailure=true} }
        function onCaptureStopped() { if(capture.opened){capture.busy=false;capture.rows=[];if(!capture.hadFailure)capture.notice="Capture is closed. Return to the draft and request a fresh sample."} }
    }
    Connections {
        target: capture.browserUi
        function onProfile_bootstrap_pendingChanged() {
            if(capture.browserUi.profile_bootstrap_pending&&capture.opened){capture.opened=false;capture.rows=[];capture.workspace.close()}
        }
        function onModeChanged() {
            if(capture.opened&&capture.browserUi.mode!=="normal"&&capture.browserUi.mode!=="command"){capture.opened=false;capture.rows=[];capture.workspace.close()}
        }
    }
}
