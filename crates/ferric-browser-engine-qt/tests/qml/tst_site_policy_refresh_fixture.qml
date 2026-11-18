import QtQuick
import QtTest
import QtWebEngine
import "../../qml/scripts/BrowserScripts.js" as BrowserScripts

TestCase {
    name: "SitePolicyRefreshFixture"
    when: windowShown
    width: 1000
    height: 800
    property bool done: false
    property var result
    WebEngineView {
        id: page
        anchors.fill: parent
        property bool loaded: false
        onLoadingChanged: function(request) { loaded = request.status === WebEngineView.LoadSucceededStatus }
    }
    function evaluate(source, world) {
        done = false
        page.runJavaScript(source, world, function(value) { result = value; done = true })
        tryVerify(function() { return done }, 15000)
        return result
    }
    function test_thousand_actual_mutations_preserve_targets_and_update_geometry() {
        page.url = Qt.resolvedUrl("../fixtures/site-policy/render-refresh-targets.html")
        tryVerify(function() { return page.loaded }, 10000)
        tryVerify(function() { return evaluate("document.body.dataset.ready==='true'", WebEngineScript.MainWorld) }, 5000)
        var original = evaluate(BrowserScripts.hintCollector("links"), WebEngineScript.ApplicationWorld)
        compare(original.candidates.length, 1000)
        evaluate("window.fixtureTargets=Array.from(document.querySelectorAll('a')); window.fixtureObserver=new MutationObserver(function(){}); fixtureObserver.observe(document.body,{subtree:true,attributes:true,attributeFilter:['style']}); true", WebEngineScript.MainWorld)
        for (var revision = 1; revision <= 2; ++revision) {
            var changed = evaluate("(()=>{fixtureObserver.takeRecords();const value=refreshFixture.mutate();const changes=fixtureObserver.takeRecords();return {revision:value.revision,total:value.total_mutations,records:changes.length,onlyTargets:changes.every(m=>m.target.id.startsWith('refresh-target-')),sameNodes:fixtureTargets.every((node,i)=>node===document.querySelectorAll('a')[i])};})()", WebEngineScript.MainWorld)
            compare(changed.revision, revision)
            compare(changed.total, revision * 1000)
            compare(changed.records, 1000)
            verify(changed.onlyTargets)
            verify(changed.sameNodes)
            var current = evaluate(BrowserScripts.hintCollector("links"), WebEngineScript.ApplicationWorld)
            compare(current.candidates.length, 1000)
            for (var index = 0; index < 1000; ++index) {
                compare(current.candidates[index].element_id, original.candidates[index].element_id)
                compare(current.candidates[index].href, original.candidates[index].href)
                verify(Math.abs(current.candidates[index].geometry.x - original.candidates[index].geometry.x - (revision % 2)) < 0.01)
            }
        }
        evaluate("fixtureObserver.disconnect(); true", WebEngineScript.MainWorld)
        evaluate(BrowserScripts.hintStopTracking(), WebEngineScript.ApplicationWorld)
    }
}
