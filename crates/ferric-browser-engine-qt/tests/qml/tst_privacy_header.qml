import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"
import "../../qml/scripts/ChromePresentation.js" as ChromePresentation

TestCase {
    name: "PrivacyHeader"
    when: windowShown
    ApplicationWindow { id: host; visible: true; width: 360; height: 640 }
    QtObject {
        id: theme
        property font font: Qt.font({family:"monospace",pointSize:12})
        property color panelColor: "#181825"
        property color privateColor: "#cba6f7"
        property color accentColor: "#89b4fa"
        function readableTextColor(color,background) { return ChromePresentation.readableTextColor(color,background) }
    }
    TextField { id: editor; parent: host.contentItem; y: 200 }
    Component {
        id: fixture
        FerricPrivacyHeader { width: host.width; browserWindow: theme; privateProfile: false; ephemeralProfile: false }
    }
    function test_identity_data() {
        return [
            {tag:"regular",privateProfile:false,ephemeral:false,label:""},
            {tag:"private",privateProfile:true,ephemeral:false,label:"Private"},
            {tag:"temporary",privateProfile:false,ephemeral:true,label:"Temporary"},
            {tag:"temporary-priority",privateProfile:true,ephemeral:true,label:"Temporary"}
        ]
    }
    function test_identity(data) {
        const header=createTemporaryObject(fixture,host.contentItem,{privateProfile:data.privateProfile,ephemeralProfile:data.ephemeral})
        verify(header)
        compare(header.label,data.label)
        compare(header.visible,data.label.length>0)
        compare(header.height>0,data.label.length>0)
        compare(header.Accessible.name,data.label.length?data.label+" window":"")
        compare(header.Accessible.role,Accessible.StaticText)
        host.requestActivate();tryCompare(host,"active",true)
        editor.forceActiveFocus();tryVerify(function(){return editor.activeFocus})
        header.privateProfile=!data.privateProfile
        compare(editor.activeFocus,true)
        compare(header.activeFocusOnTab,false)
    }
    function test_narrow_large_text_data() {
        return [{tag:"private-26",ephemeral:false,size:26},{tag:"temporary-26",ephemeral:true,size:26},
            {tag:"private-60",ephemeral:false,size:60},{tag:"temporary-60",ephemeral:true,size:60}]
    }
    function test_hidden_parent_keeps_intrinsic_height_data() {
        return [{tag:"private",ephemeral:false},{tag:"temporary",ephemeral:true}]
    }
    function test_hidden_parent_keeps_intrinsic_height(data) {
        const hidden=createTemporaryObject(hiddenParent,host.contentItem)
        const header=createTemporaryObject(fixture,hidden,{privateProfile:true,ephemeralProfile:data.ephemeral})
        verify(!header.visible)
        verify(header.implicitHeight>0,"The parent uses intrinsic height to decide whether it is visible")
    }
    Component { id: hiddenParent; Item { visible: false } }
    function test_narrow_large_text(data) {
        theme.font=Qt.font({family:"monospace",pointSize:data.size})
        const header=createTemporaryObject(fixture,host.contentItem,{width:320,privateProfile:true,ephemeralProfile:data.ephemeral})
        const text=findChild(header,"windowPrivacyLabel")
        verify(text)
        compare(text.font.pointSize,data.size)
        tryVerify(function(){return text.contentWidth<=text.width+1})
        verify(text.height<=header.height)
        verify(header.Accessible.name.length>0)
    }
    function cleanup() { theme.font=Qt.font({family:"monospace",pointSize:12}) }
}
