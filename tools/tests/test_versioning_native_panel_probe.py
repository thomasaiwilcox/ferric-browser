#!/usr/bin/python3
"""Scope checks for the disposable native commit-list accessibility probe."""

import importlib.util
import pathlib
import unittest


PROBE_PATH = pathlib.Path(__file__).resolve().parents[1] / "versioning-native-panel-probe.py"
SPEC = importlib.util.spec_from_file_location("versioning_native_panel_probe", PROBE_PATH)
PROBE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROBE)

PID = 1234
COMMIT_ID = "commit:12345678-1234-4234-8234-123456789abc"


class Node:
    def __init__(self, name, children=(), pid=PID):
        self.name = name
        self.children = children
        self.pid = pid

    def get_process_id(self):
        return self.pid

    def get_name(self):
        return self.name

    def get_child_count(self):
        return len(self.children)

    def get_child_at_index(self, index):
        return self.children[index]


class InspectTargetTests(unittest.TestCase):
    def test_exact_row_inside_saved_list(self):
        row = Node("Commit identifier " + COMMIT_ID)
        app = Node("Ferric", [Node("Saved commits", [row])])
        self.assertTrue(PROBE.inspect_target(app, COMMIT_ID, PID))

    def test_matching_row_in_another_branch_is_not_a_list_result(self):
        row = Node("Commit identifier " + COMMIT_ID)
        app = Node("Ferric", [Node("Saved commits"), Node("Details", [row])])
        self.assertFalse(PROBE.inspect_target(app, COMMIT_ID, PID))

    def test_other_process_subtree_is_not_inspected(self):
        row = Node("Commit identifier " + COMMIT_ID)
        app = Node("Ferric", [Node("Saved commits", [row], pid=9876)])
        self.assertFalse(PROBE.inspect_target(app, COMMIT_ID, PID))

    def test_similar_identifier_does_not_match(self):
        row = Node("Commit identifier " + COMMIT_ID[:-1] + "d")
        app = Node("Ferric", [Node("Saved commits", [row])])
        self.assertFalse(PROBE.inspect_target(app, COMMIT_ID, PID))


if __name__ == "__main__":
    unittest.main()
