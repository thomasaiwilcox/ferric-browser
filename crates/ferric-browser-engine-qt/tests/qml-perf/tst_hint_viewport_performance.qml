import QtQuick
import QtQuick.Window
import QtTest
import Ferric.Testing 1.0
import "../../qml/components"
import "../../qml/scripts/HintPresentation.js" as Presentation

TestCase {
    id: test
    name: "HintViewportPerformance"
    when: windowShown
    visible: true
    width: 640; height: 480
    Item {
        id: palette
        anchors.fill: parent
        property color warningColor: "#e0b040"
        property color primaryTextColor: "#202020"
        property color backgroundColor: "#ffffff"
        function contrastText(color) { return "#202020" }
        FerricHintOverlay {
            id: overlay
            browserWindow: palette
            hintsVisible: false
            hintResults: []
            sourceViewport: ({width:640,height:480})
            hintState: ({mode:"label",prefix:"",matching_labels:[],active_label:""})
            unmatchedPolicy: "show"
            markerScale: 1
        }
    }
    SignalSpy { id: layouts; target: overlay; signalName: "laidOutHintsChanged" }
    function test_refresh_batch_data() { return [{tag:"100",count:100},{tag:"500",count:500}] }
    function test_refresh_batch(data) {
        var rows=[]
        var columns=data.count === 100 ? 10 : 25
        for(var i=0;i<data.count;++i) rows.push({label:"a"+i,element_id:i+1,kind:"button",
            text:"Target "+i,frame_owner:"",frame_path:"0",
            x:(i%columns)*(600/columns),y:Math.floor(i/columns)*20,width:18,height:14})
        overlay.hintResults=rows
        function batch(reuse) {
            layouts.clear()
            var started=PerformanceClock.nowNsecs()
            var viewport=reuse ? Presentation.viewportSnapshot({width:640,height:480},overlay.sourceViewport)
                               : {width:640,height:480} // previous production refresh assignment
            if(viewport !== overlay.sourceViewport) overlay.sourceViewport=viewport
            overlay.hintResults=rows.slice()
            var elapsed=(PerformanceClock.nowNsecs()-started)/1000000
            compare(layouts.count,reuse ? 1 : 2)
            compare(overlay.laidOutHints.length,data.count)
            return elapsed
        }
        for(var warm=0;warm<10;++warm) { batch(false);batch(true) }
        var before=[],after=[]
        for(var sample=0;sample<100;++sample) {
            if(sample%2) { after.push(batch(true));before.push(batch(false)) }
            else { before.push(batch(false));after.push(batch(true)) }
        }
        var expected=JSON.stringify(overlay.laidOutHints)
        batch(false)
        compare(JSON.stringify(overlay.laidOutHints),expected,"same viewport preserves layout")
        batch(true)
        compare(JSON.stringify(overlay.laidOutHints),expected,"removed pass preserves layout")
        console.info("FERRIC_VIEWPORT_PERFORMANCE_JSON "+JSON.stringify({count:data.count,
            warmups:10,before_ms:before,after_ms:after,layouts_before:2,layouts_after:1,
            graphics_api:GraphicsInfo.api,
            order:"alternating control/candidate first in the same process",
            boundary:"production overlay: viewport plus fresh result array; synthetic geometry; excludes DOM/Rust"}))
        overlay.hintResults=[]
    }
}
