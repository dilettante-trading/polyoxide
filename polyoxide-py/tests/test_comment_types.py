"""Guards the py_type! field lists for the comment family — presence, not resolution.

`hasattr(polyoxide.Comment, attr)` only checks that a getter descriptor exists
on the class; PyO3 registers one for every entry in the `py_type!` list
regardless of which JSON key it resolves, so this check cannot fail on a
stale rename (py_type! getters resolve by camelCase key lookup and return
None for any key that is not there — polyoxide-py/src/convert.rs:23-29). What
it still catches: a field dropped from the list entirely (`CommentUser`
below), and, via `test_fixture_is_present`, the shared fixture going missing.

The instance-level guard against a getter silently resolving to None lives in
Rust instead, because py_type! exposes no Python-side constructor:
`comment_getters_resolve_against_the_shared_fixture` in
`polyoxide-py/src/types/gamma.rs` builds a real `PyComment` from the shared
fixture and asserts each getter is not None, pinning `parent_entity_id` and
`parent_comment_id` specifically — the two an `ID`-suffix rename mistake would
null out.
"""

import json
import pathlib

import polyoxide

FIXTURE = (
    pathlib.Path(__file__).parents[2]
    / "polyoxide-gamma"
    / "tests"
    / "fixtures"
    / "comment_full.json"
)


def test_fixture_is_present():
    assert FIXTURE.is_file(), f"missing shared fixture: {FIXTURE}"


def test_every_comment_getter_resolves():
    wire = json.loads(FIXTURE.read_text())
    expected = {
        "id": "3218542",
        "parent_entity_type": "Event",
        "parent_entity_id": 45915,
        "parent_comment_id": "3218360",
        "user_address": "0x8ef197aa37f6d0ec4c5802b40c34ec358e4da6ab",
        "reply_address": "0x3f70913f63616e1c9dc1975c092c3e682f7a9931",
        "report_count": 0,
        "reaction_count": 1,
    }
    # Sanity: the fixture really does carry these, under wire spelling.
    assert wire["parentEntityID"] == expected["parent_entity_id"]
    assert wire["parentCommentID"] == expected["parent_comment_id"]

    for attr in expected:
        assert hasattr(polyoxide.Comment, attr), (
            f"Comment.{attr} is missing from the py_type! field list"
        )
    for attr in ("name", "pseudonym", "proxy_wallet", "base_address", "positions"):
        assert hasattr(polyoxide.CommentProfile, attr), (
            f"CommentProfile.{attr} is missing from the py_type! field list"
        )
    for attr in ("id", "comment_id", "reaction_type", "user_address", "profile"):
        assert hasattr(polyoxide.CommentReaction, attr), (
            f"CommentReaction.{attr} is missing from the py_type! field list"
        )
    for attr in ("token_id", "position_size"):
        assert hasattr(polyoxide.CommentPosition, attr), (
            f"CommentPosition.{attr} is missing from the py_type! field list"
        )


def test_comment_user_is_gone():
    # It had no upstream counterpart and was removed in 0.28.0.
    assert not hasattr(polyoxide, "CommentUser")
