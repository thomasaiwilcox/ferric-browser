#!/usr/bin/env python3
"""Production managed-group Lua against exact disposable native windows."""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import sys
import uuid
from hyprland_proof import FocusEvents, eval_lua, focus_owned, require, require_target_compositor
from page_group_transactions import TransactionFixture, transaction

SCRIPT = (Path(__file__).resolve().parents[3] / 'src/hyprland/managed_groups.lua').read_text()
ROOT = '__ferric_managed_page_groups'

def lua(value):
    if isinstance(value, str): return json.dumps(value)
    if isinstance(value, bool): return 'true' if value else 'false'
    if isinstance(value, int): return str(value)
    if isinstance(value, list): return '{' + ','.join(map(lua, value)) + '}'
    return '{' + ','.join(key + '=' + lua(item) for key, item in value.items()) + '}'

class Manager:
    def __init__(self, fixture):
        self.fixture = fixture
        self.process, self.epoch, self.scope = [str(uuid.uuid4()) for _ in range(3)]
        self.caps = []
        self.roles = {}
        self.orders = {}
        self.operations = 0
    def add(self, window, scope=None, auto=True):
        self.roles[window.tag] = (window, scope or self.scope, auto)
    def apply(self, **overrides):
        requested = []
        for index, (window, scope, auto) in enumerate(self.roles.values(), start=1):
            requested.append(dict(tag=window.tag, app=window.fixture.app_id, pid=window.pid,
                                  stable=str(window.stable_id), address=window.address,
                                  scope=scope, auto=auto, ready=True, order=self.orders.get(window.tag, index)))
        options = dict(process=self.process, epoch=self.epoch, scope=self.scope,
                       fresh=str(uuid.uuid4()), pid=self.fixture.process.pid,
                       app=self.fixture.app_id, caps=self.caps, allow_create=True)
        options.update(overrides)
        result = eval_lua('return (function() local apply=(function()\n' + SCRIPT +
                          '\nend)() return apply(' + lua(requested) + ',' + lua(options) + ') end)()')
        self.operations += 1
        if result.startswith(('created|', 'added|', 'unchanged|', 'retired|', 'forgotten|', 'ordered|', 'ordering|')):
            kind, key, scope, members = result.split('|')
            require(scope == self.scope or any(cap == dict(key=key, scope=scope) for cap in self.caps),
                    'receipt-changed-birth-scope')
            self.caps = [cap for cap in self.caps if cap['key'] != key]
            if members: self.caps.append(dict(key=key, scope=scope))
        return result

def run(executable, instance):
    fixture = TransactionFixture(executable)
    neighbor = None
    events = None
    try:
        source, target = fixture.create(), fixture.create()
        neighbor = TransactionFixture(executable, 'ferric-w00-neighbor-proof')
        sentinel = neighbor.create()
        events = FocusEvents(instance)
        owned = {source.address, target.address, sentinel.address}
        manager = Manager(fixture)
        manager.add(source); manager.add(target)
        focus_owned(source)
        require(manager.apply(allow_create=False) == 'reject-registry-allocation', 'disabled-mint-created-group')
        require(manager.apply().startswith('created|'), 'managed-create-failed')
        focus_owned(sentinel); events.drain(owned)
        require(manager.apply().startswith('added|'), 'managed-add-failed')
        require(manager.apply().startswith('unchanged|'), 'managed-unchanged-failed')
        manager.orders[source.tag], manager.orders[target.tag] = 2, 1
        require(manager.apply().startswith('ordering|'), 'background-order-not-deferred')
        count, foreign = events.drain(owned)
        require(count == 0 and not foreign, 'deferred-order-changed-focus')
        focus_owned(source); events.drain(owned)
        def snapshot():
            fixture._send(dict(op='focus-snapshot', key=source.key))
            result = fixture._read_event()
            require(result['event'] == 'focus-snapshot' and result['active'] and result['focus_window'], 'owned-qt-focus-not-active')
            return result
        before = snapshot()
        require(manager.apply().startswith('ordered|'), 'active-forward-order-failed')
        events.drain(owned)
        require(snapshot() == before, 'active-forward-order-changed-qt-focus')
        manager.orders[source.tag], manager.orders[target.tag] = 1, 2
        require(manager.apply().startswith('ordered|'), 'active-backward-order-failed')
        events.drain(owned)
        require(snapshot() == before, 'active-backward-order-changed-qt-focus')
        focus_owned(sentinel); events.drain(owned)
        require(manager.apply(epoch=str(uuid.uuid4())) == 'reject-managed-capability', 'wrong-epoch-adopted-group')
        require(manager.apply(process=str(uuid.uuid4())) == 'reject-managed-capability', 'wrong-process-adopted-group')
        require(manager.apply(caps=[]) == 'reject-managed-capability', 'lost-receipt-adopted-group')
        original_scope = manager.scope
        require(manager.apply(scope=str(uuid.uuid4())) == 'defer-pages', 'birth-scope-was-relabeled')
        count, foreign = events.drain(owned)
        require(count == 0 and not foreign, 'background-operation-changed-focus')
        focus_owned(source)
        independent = fixture.create()
        owned.add(independent.address)
        other_scope = str(uuid.uuid4())
        manager.add(independent, other_scope)
        focus_owned(independent); events.drain(owned)
        require(manager.apply().startswith('retired|'), 'independent-active-retirement-failed')
        require(manager.scope == original_scope, 'retirement-relabeled-capability')
        count, foreign = events.drain(owned)
        require(count == 0 and not foreign, 'retirement-changed-focus')
        require(eval_lua('return hl.get_active_window().group == nil and "separate" or "grouped"') == 'separate',
                'retired-window-still-grouped')
        # Auto-off is removal-only, including the final singleton member.
        manager.add(target, auto=False)
        focus_owned(target); events.drain(owned)
        require(manager.apply(allow_create=False).startswith('retired|'), 'auto-off-member-retirement-failed')
        manager.add(source, auto=False)
        focus_owned(source); events.drain(owned)
        require(manager.apply(allow_create=False).endswith('|'), 'auto-off-singleton-retirement-failed')
        require(not manager.caps, 'expired-group-capability-retained')
        count, foreign = events.drain(owned)
        require(count == 0 and not foreign, 'auto-off-retirement-changed-focus')
        manager.add(source); manager.add(target)
        require(manager.apply().startswith('created|'), 'last-seed-setup-failed')
        contaminant_b, contaminant_c = fixture.create(), fixture.create()
        owned.update([contaminant_b.address, contaminant_c.address])
        manager.add(contaminant_b, str(uuid.uuid4()))
        manager.add(contaminant_c, str(uuid.uuid4()))
        manager.add(source, auto=False)
        focus_owned(source); events.drain(owned)
        require(manager.apply().startswith('retired|') and manager.caps, 'last-original-seed-stranded-contaminants')
        focus_owned(contaminant_b); events.drain(owned)
        require(manager.apply().startswith('retired|') and manager.caps, 'remaining-contaminant-capability-lost')
        focus_owned(contaminant_c); events.drain(owned)
        require(manager.apply().startswith('retired|') and not manager.caps, 'final-contaminant-retirement-failed')
        count, foreign = events.drain(owned)
        require(count == 0 and not foreign, 'last-seed-retirement-changed-focus')
        manager.add(source)
        focus_owned(source)
        require(manager.apply().startswith('created|'), 'managed-recreate-failed')
        require(transaction([source, target, independent], 'remove') == 'removed', 'destroy-group-failed')
        require(transaction([source, target, independent], 'create') == 'created', 'user-recreate-failed')
        require(manager.apply().startswith('forgotten|'), 'expired-weak-capability-not-forgotten')
        require(not manager.caps, 'expired-weak-receipt-retained')
        require(manager.apply() == 'reject-managed-capability', 'weak-capability-adopted-recreated-group')
        # State lost on reload cannot be reconstructed from matching members.
        manager.caps = [dict(key=str(uuid.uuid4()), scope=original_scope)]
        eval_lua('return (function() _G.' + ROOT + '=nil return "cleared" end)()')
        require(manager.apply().startswith('forgotten|'), 'reload-receipt-not-forgotten')
        require(manager.apply() == 'reject-managed-capability', 'reload-adopted-user-group')
        # Obtain a fresh managed group and show an unknown real native member
        # declines cleanup even though the active member is application-owned.
        require(transaction([source, target, independent], 'remove') == 'removed', 'user-group-cleanup-failed')
        manager.caps = []
        require(manager.apply().startswith('created|'), 'fresh-capability-failed')
        # Surviving actual windows must not retain obsolete weak group leases
        # until a later successful mint loses its receipt at the Rust bound.
        for _ in range(130):
            require(transaction([source, target, independent], 'remove') == 'removed', 'cycle-destroy-failed')
            require(manager.apply().startswith('forgotten|') and not manager.caps, 'cycle-expired-receipt-retained')
            require(manager.apply().startswith('created|') and len(manager.caps) == 1, 'cycle-create-lost-capability')
        foreign_window = neighbor.create()
        owned.add(foreign_window.address)
        focus_owned(source); events.drain(owned)
        manager.add(source, auto=False)
        require(manager.apply() == 'reject-membership', 'unknown-native-member-was-touched')
        count, foreign = events.drain(owned)
        require(count == 0 and not foreign, 'unknown-member-decline-changed-focus')
        return dict(result='pass', operations=manager.operations, background_focus_events=0,
                    scope_immutable=True, auto_off_removal_only=True, weak_capability=True,
                    lost_receipt_declines=True, unknown_member_declines=True, expired_group_cycles=130,
                    last_original_seed_retirement=True, active_native_order=True, qt_focus_continuity=True)
    finally:
        try:
            eval_lua('return (function() _G.' + ROOT + '=nil return "cleared" end)()')
        finally:
            if events: events.close()
            try:
                if neighbor: neighbor.close()
            finally: fixture.close()

def advanced_order(executable, instance):
    fixture = TransactionFixture(executable)
    neighbor = None
    events = None
    try:
        a, b, c, d = [fixture.create() for _ in range(4)]
        neighbor = TransactionFixture(executable, 'ferric-w00-neighbor-proof')
        sentinel = neighbor.create()
        owned = {w.address for w in (a,b,c,d,sentinel)}
        events = FocusEvents(instance)
        manager = Manager(fixture)
        for window in (a,b,c,d): manager.add(window)
        manager.orders = {a.tag:3,b.tag:1,c.tag:2,d.tag:4}
        def state():
            return eval_lua('return (function() local a=nil for _,w in ipairs(hl.get_windows()) do '
                            'if w.pid==' + str(a.pid) + ' and w.xdg_tag==' + json.dumps(a.tag) + ' then a=w end end '
                            'if not a or not a.group then return "missing" end local ids={} '
                            'for _,w in ipairs(a.group.members) do ids[#ids+1]=tostring(w.stable_id) end '
                            'return table.concat(ids,",").."|"..tostring(a.group.current.stable_id) end)()')
        def expected(windows, current):
            return ','.join(str(w.stable_id) for w in windows) + '|' + str(current.stable_id)
        def snapshot(window):
            fixture._send(dict(op='focus-snapshot', key=window.key))
            result = fixture._read_event()
            require(result['event']=='focus-snapshot', 'qt-focus-snapshot-unavailable')
            return result
        focus_owned(a)
        require(manager.apply().startswith('created|'), 'advanced-create-failed')
        focus_owned(sentinel); events.drain(owned)
        before = snapshot(a)
        for _ in range(3): require(manager.apply().startswith('added|'), 'advanced-insertion-failed')
        require(state()==expected([b,c,a,d],a), 'insertion-before-current-lost-order-or-current')
        count, foreign = events.drain(owned)
        require(count==0 and not foreign and snapshot(a)==before, 'inactive-insertion-changed-qt-focus')
        focus_owned(a); events.drain(owned); before = snapshot(a)
        manager.orders = {a.tag:1,b.tag:2,c.tag:3,d.tag:4}
        require(manager.apply().startswith('ordered|'), 'nonadjacent-backward-order-failed')
        require(state()==expected([a,b,c,d],a), 'nonadjacent-backward-order-wrong')
        events.drain(owned); require(snapshot(a)==before, 'nonadjacent-backward-changed-qt-focus')
        manager.orders = {b.tag:1,c.tag:2,d.tag:3,a.tag:4}
        require(manager.apply().startswith('ordered|'), 'nonadjacent-forward-order-failed')
        require(state()==expected([b,c,d,a],a), 'nonadjacent-forward-order-wrong')
        events.drain(owned); require(snapshot(a)==before, 'nonadjacent-forward-changed-qt-focus')
        manager.orders = {c.tag:1,b.tag:2,d.tag:3,a.tag:4}
        require(manager.apply().startswith('ordering|'), 'partial-permutation-not-deferred')
        require(state()==expected([b,c,d,a],a), 'partial-permutation-mutated-other-members')
        count, foreign = events.drain(owned)
        require(count==0 and not foreign and snapshot(a)==before, 'partial-deferral-changed-focus')
        manager.orders = {c.tag:1,b.tag:2,a.tag:3,d.tag:4}
        require(manager.apply().startswith('ordering|'), 'partial-progress-not-reported-deferred')
        require(state()==expected([b,c,a,d],a), 'partial-progress-reordered-other-members')
        events.drain(owned); require(snapshot(a)==before, 'partial-progress-changed-qt-focus')
        focus_owned(b); events.drain(owned); before = snapshot(b)
        require(manager.apply().startswith('ordered|'), 'later-current-cannot-finish-order')
        require(state()==expected([c,b,a,d],b), 'later-current-finished-wrong-order')
        events.drain(owned); require(snapshot(b)==before, 'later-current-order-changed-qt-focus')
        focus_owned(sentinel); events.drain(owned)
        manager.orders = {c.tag:1,b.tag:2,d.tag:3,a.tag:4}
        require(manager.apply().startswith('ordering|'), 'inactive-permutation-not-deferred')
        require(state()==expected([c,b,a,d],b), 'inactive-permutation-mutated-group')
        count, foreign = events.drain(owned)
        require(count==0 and not foreign, 'inactive-permutation-changed-focus')
        # Acknowledgement of real on-map same-scope peers must retain weak
        # witnesses even when every pre-map seed later closes naturally.
        focus_owned(b)
        e, f = fixture.create(), fixture.create()
        owned.update((e.address,f.address))
        manager.add(e); manager.add(f)
        manager.orders[e.tag], manager.orders[f.tag] = 5, 6
        result = manager.apply()
        require(result.startswith(('ordered|','ordering|','unchanged|')), 'on-map-peer-acknowledgement-failed')
        for window in (a,b,c,d):
            fixture.destroy(window)
            manager.roles.pop(window.tag)
            manager.orders.pop(window.tag)
        manager.orders = {e.tag:1,f.tag:2}
        require(manager.apply().startswith(('ordered|','ordering|','unchanged|')), 'acknowledged-peer-capability-lost-after-old-seeds-close')
        manager.add(f,auto=False); focus_owned(f)
        require(manager.apply().startswith('retired|'), 'acknowledged-peer-retirement-failed')
        manager.add(e,auto=False); focus_owned(e)
        require(manager.apply().startswith('retired|') and not manager.caps, 'acknowledged-final-peer-retirement-failed')
        return dict(advanced_order_operations=manager.operations, nonadjacent_native_order=True,
                    partial_order_defers=True, insertion_preserves_current=True,
                    mapped_peer_ack_survives_old_seed_close=True)
    finally:
        try:
            eval_lua('return (function() _G.' + ROOT + '=nil return "cleared" end)()')
        finally:
            if events: events.close()
            try:
                if neighbor: neighbor.close()
            finally: fixture.close()

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--fixture', required=True)
    args = parser.parse_args()
    try:
        instance = require_target_compositor()
        result = run(args.fixture, instance)
        result.update(advanced_order(args.fixture, instance))
        print(json.dumps(result, sort_keys=True))
        return 0
    except Exception as error:
        print(json.dumps(dict(result='failed', reason=str(error))), file=sys.stderr)
        return 1

if __name__ == '__main__': raise SystemExit(main())
