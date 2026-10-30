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
        self.operations = 0
    def add(self, window, scope=None, auto=True):
        self.roles[window.tag] = (window, scope or self.scope, auto)
    def apply(self, **overrides):
        requested = []
        for window, scope, auto in self.roles.values():
            requested.append(dict(tag=window.tag, app=window.fixture.app_id, pid=window.pid,
                                  stable=str(window.stable_id), address=window.address,
                                  scope=scope, auto=auto, ready=True))
        options = dict(process=self.process, epoch=self.epoch, scope=self.scope,
                       fresh=str(uuid.uuid4()), pid=self.fixture.process.pid,
                       app=self.fixture.app_id, caps=self.caps, allow_create=True)
        options.update(overrides)
        result = eval_lua('return (function() local apply=(function()\n' + SCRIPT +
                          '\nend)() return apply(' + lua(requested) + ',' + lua(options) + ') end)()')
        self.operations += 1
        if result.startswith(('created|', 'added|', 'unchanged|', 'retired|', 'forgotten|')):
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
                    last_original_seed_retirement=True)
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
        print(json.dumps(run(args.fixture, require_target_compositor()), sort_keys=True))
        return 0
    except Exception as error:
        print(json.dumps(dict(result='failed', reason=str(error))), file=sys.stderr)
        return 1

if __name__ == '__main__': raise SystemExit(main())
