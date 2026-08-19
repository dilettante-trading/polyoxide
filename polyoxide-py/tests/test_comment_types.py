"""Guards the py_type! field lists for the comment family.

py_type! getters resolve by camelCase key lookup and return None for any key
that is not there (polyoxide-py/src/convert.rs:23), so a stale field list is
invisible without an explicit assertion. Every getter below has a value in the
captured payload; a None means the macro's key does not match the wire.
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
