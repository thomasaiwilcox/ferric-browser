import QtQuick
import QtTest
import "../../qml/scripts/SwitcherResults.js" as Results

TestCase {
    name: "SwitcherResultBatches"
    function row(kind, id, owner, profile, rank) {
        return {kind: kind, id: id, owner_token: owner, profile: profile, rank: rank}
    }
    function test_previous_owner_waits_for_its_own_replacement() {
        var old = row("tab", "selected", "child", "default", 100)
        var current = row("tab", "new", "root", "default", 90)
        var partial = Results.publish([current], [old], ["child"], 100)
        compare(partial.length, 2)
        compare(partial[0], old)
        var completed = Results.publish([current], [old], [], 100)
        compare(completed.length, 1)
        compare(completed[0], current)
    }
    function test_catalogs_deduplicate_without_merging_profiles_or_live_owners() {
        var values = [row("bookmark", "a", "root", "default", 10),
            row("bookmark", "a", "child", "default", 10),
            row("bookmark", "a", "work", "work", 10),
            row("tab", "a", "root", "default", 20),
            row("tab", "a", "child", "default", 20)]
        var published = Results.publish(values, [], [], 100)
        compare(published.length, 4)
        compare(published.filter(function(value) { return value.kind === "bookmark" }).length, 2)
        compare(published.filter(function(value) { return value.kind === "tab" }).length, 2)
        compare(published.find(function(value) { return value.kind === "bookmark"
            && value.profile === "default" }).owner_token, "root")
    }
    function test_invoker_commands_and_global_limit_apply_to_the_combined_results() {
        var values = [row("command", "open", "invoker", "work", 5),
            row("command", "open", "other", "default", 5),
            row("tab", "third", "other", "default", 8),
            row("tab", "second", "invoker", "work", 9),
            row("tab", "first", "other", "default", 10)]
        var all = Results.publish(values, [], [], 100)
        compare(all.length, 4)
        compare(all.find(function(value) { return value.kind === "command" }).owner_token, "invoker")
        var limited = Results.publish(values, [], [], 2)
        compare(limited.length, 2)
        compare(limited[0].id, "first")
        compare(limited[1].id, "second")
    }
}
