import QtQuick
import QtTest
import "../../qml/components"

TestCase {
    name: "StoragePollScheduler"
    Component { id: factory; FerricStoragePollScheduler {} }
    function test_idle_and_admitted_work() {
        var calls = 0
        var pending = false
        var owner = { poll_storage_library: function() { ++calls; return true },
            storage_library_delay_ms: function() { return pending ? 50 : 1000 } }
        var scheduler = createTemporaryObject(factory, this, {
            enabled: true, entries: function() { return [{ui:owner}] }
        })
        compare(scheduler.interval, 1000)
        wait(120)
        compare(calls, 0, "idle has no 50 ms polling")
        pending = true
        scheduler.wake()
        tryCompare(scheduler, "interval", 50, 200)
        compare(calls, 1, "admission wakes immediately")
        tryVerify(function() { return calls >= 2 }, 200)
        pending = false
        tryCompare(scheduler, "interval", 1000, 200)
        var settled = calls
        wait(120)
        compare(calls, settled)
    }
    function test_only_durable_owners_and_disable() {
        var calls = 0
        var owner = { poll_storage_library: function() { ++calls; return false },
            storage_library_delay_ms: function() { return 1000 } }
        var scheduler = createTemporaryObject(factory, this, {enabled:true,
            entries:function() { return [null, {}, {ui:owner,privateProfile:true},
                {ui:owner,ephemeralProfile:true}, {ui:owner}] }})
        scheduler.wake()
        tryCompare(scheduler, "interval", 1000, 200)
        compare(calls, 1, "durable secondary is polled even with private peers")
        scheduler.enabled = false
        scheduler.wake()
        verify(!scheduler.running)
        wait(80)
        compare(calls, 1)
    }
}
