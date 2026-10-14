import QtQuick
import QtTest
import "../../qml/components" as Ferric

TestCase {
    id:test
    name:"HintPresentation"
    when:windowShown
    visible:true
    width:640;height:480
    Item {
        id:canvas
        anchors.fill:parent
        property color warningColor:"#f0c54a"
        property color primaryTextColor:"#202020"
        property color backgroundColor:"#ffffff"
        function contrastText(color){return "#202020"}
        Rectangle {anchors.fill:parent;color:"white"}
        Text {x:25;y:15;text:"Repaired dense hints";font.pixelSize:20}
        Repeater {
            model:18
            Rectangle {
                x:40+index%9*26;y:100+Math.floor(index/9)*26;width:16;height:16
                color:"#244b76";radius:2
                Text {anchors.centerIn:parent;text:String(index+1);color:"white";font.pixelSize:9}
            }
        }
        Rectangle {x:340;y:100;width:230;height:270;color:"#e1e7ef";border.color:"#8094ad"}
        Ferric.FerricHintOverlay {
            id:overlay
            browserWindow:canvas;hintsVisible:true
            hintResults:[];sourceViewport:({width:640,height:480})
            hintState:({mode:"label",prefix:"",matching_labels:[],active_label:""})
            unmatchedPolicy:"hide";markerScale:1
        }
        Text {x:25;y:420;width:590;text:overlay.previewText;textFormat:Text.PlainText;elide:Text.ElideRight;font.pixelSize:15}
    }
    function state(rows,active){return {mode:"label",prefix:"",matching_labels:rows.map(function(r){return r.label}),active_label:active||""}}
    function row(label,x,y,w,h,id){return {label:label,text:"Control "+id,action:"Activate",kind:"button",element_id:id,frame_path:"0",x:x,y:y,width:w,height:h}}
    function cleanup(){overlay.hintsVisible=false;overlay.hintResults=[];overlay.hintsVisible=true}
    function test_stationary_target_keeps_placement() {
        var card=row("aa",340,100,230,270,1)
        overlay.hintResults=[card];overlay.hintState=state([card],"aa")
        var before=overlay.laidOutHints[0]
        overlay.hintResults=[row("as",340,80,16,16,2),card]
        var after=overlay.laidOutHints[1]
        compare(after.marker_x,before.marker_x);compare(after.marker_y,before.marker_y)
    }
    function test_distinct_overlap_reachable_and_actual_marker_group() {
        var rows=[row("aa",40,100,160,32,1),row("as",40,100,160,32,2)]
        // Constrain a narrow corner so local displacement cannot hide the collision.
        rows[0].height=rows[1].height=16
        rows[0].y=rows[1].y=0
        overlay.hintResults=rows;overlay.hintState=state(rows,"aa")
        // Both targets remain in the presentation model regardless of grouping.
        compare(overlay.laidOutHints.length,2)
        compare(overlay.displayedMarkers.length,1)
        compare(overlay.collisionLabels("aa").length,2)
        overlay.hintState=state(rows,"as")
        compare(overlay.activeHint.element_id,2)
        verify(overlay.markerVisible(overlay.activeHint))
    }
    function test_hover_does_not_replace_keyboard_selection() {
        var rows=[row("aa",40,100,100,32,1),row("as",200,100,100,32,2)]
        overlay.hintResults=rows;overlay.hintState=state(rows,"aa")
        var other=overlay.laidOutHints[1]
        mouseMove(overlay,other.marker_x+5,other.marker_y+5)
        tryCompare(overlay,"hoveredLabel","as")
        overlay.hintState=state(rows,"aa")
        compare(overlay.hoveredLabel,"as")
        compare(overlay.activeHint.label,"aa")
        verify(overlay.previewText.indexOf("aa")===0)
    }
    function test_small_controls_have_explicit_connections() {
        var rows=[],alphabet="asdfghjkl"
        for(var i=0;i<18;i++)rows.push(row(alphabet[Math.floor(i/9)]+alphabet[i%9],40+i%9*26,100+Math.floor(i/9)*26,16,16,i+1))
        rows.push(row("da",340,100,230,270,19))
        overlay.hintResults=rows;overlay.hintState=state(rows,"aa")
        for(var target of overlay.laidOutHints) {
            if(target.width>=target.marker_width&&target.height>=target.marker_height)continue
            verify(target.connected)
            verify(target.endpoint_x>=target.x&&target.endpoint_x<=target.x+target.width)
            verify(target.endpoint_y>=target.y&&target.endpoint_y<=target.y+target.height)
        }
        waitForRendering(overlay)
    }
}
