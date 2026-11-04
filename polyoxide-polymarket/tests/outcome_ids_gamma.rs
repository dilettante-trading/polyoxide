//! `Market::outcome_ids` chooses a market's trading ids by its `version`.
//!
//! The binary cases are held to py-sdk 0.12.0's
//! `normalize_market_position_context` through
//! `tests/fixtures/gamma/protocol_v2/selector_vectors.json`: the pair py-sdk
//! chose, or a refusal where py-sdk refused. py-sdk cannot express a
//! three-outcome market or a malformed `outcomes`, so those cases assert the
//! refusal or the pairing read straight off the input they were built from.

use polyoxide_polymarket::gamma::{
    types::{Market, OutcomeId},
    OutcomeIdsError,
};
use polyoxide_test_support::fixtures;
use serde_json::Value;

/// The first market of the captured CLOB venue fixture, which the vectors edit.
fn base_market() -> Value {
    fixtures!("clob/venue").json("gamma_markets")[0].clone()
}

fn market_of(value: &Value) -> Market {
    serde_json::from_value(value.clone()).unwrap()
}

/// The base market with only `version`, `clobTokenIds`, `positionIds` and
/// `outcomes` set to what is given, and removed where `None`.
fn market(
    version: Option<&str>,
    clob_token_ids: Option<&str>,
    position_ids: Option<&[&str]>,
    outcomes: Option<&str>,
) -> Market {
    let mut value = base_market();
    let object = value.as_object_mut().unwrap();
    let edits = [
        ("version", version.map(Value::from)),
        ("clobTokenIds", clob_token_ids.map(Value::from)),
        (
            "positionIds",
            position_ids.map(|ids| Value::from(ids.to_vec())),
        ),
        ("outcomes", outcomes.map(Value::from)),
    ];
    for (key, edit) in edits {
        match edit {
            Some(edit) => {
                object.insert(key.to_owned(), edit);
            }
            None => {
                object.remove(key);
            }
        }
    }
    market_of(&value)
}

/// A JSON-text array of strings, as Gamma sends `outcomes` and `clobTokenIds`.
fn json_strings(text: &Value) -> Vec<String> {
    serde_json::from_str(text.as_str().unwrap()).unwrap()
}

fn strings(array: &Value) -> Vec<String> {
    serde_json::from_value(array.clone()).unwrap()
}

fn pairs(outcomes: &[&str], ids: &[&str]) -> Vec<OutcomeId> {
    outcomes
        .iter()
        .zip(ids)
        .map(|(outcome, id)| OutcomeId {
            outcome: (*outcome).to_owned(),
            id: (*id).to_owned(),
        })
        .collect()
}

fn vectors() -> Value {
    fixtures!("gamma/protocol_v2").json("selector_vectors")
}

fn case<'a>(vectors: &'a Value, name: &str) -> &'a Value {
    vectors["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap_or_else(|| panic!("no selector vector named {name}"))
}

#[test]
fn the_selector_agrees_with_py_sdk_on_every_vector() {
    let vectors = vectors();
    let cases = vectors["cases"].as_array().unwrap();
    let names: Vec<&str> = cases
        .iter()
        .map(|case| case["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "v1_with_position_ids",
            "v2_with_both",
            "missing_version",
            "unknown_version",
            "v2_one_position_id",
            "v1_no_clob_token_ids",
        ]
    );

    let (mut chosen, mut refused) = (0, 0);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let result = market_of(&case["market"]).outcome_ids();
        match &case["ids"] {
            Value::Null => {
                refused += 1;
                assert!(result.is_err(), "{name}: py-sdk refused, got {result:?}");
            }
            pair => {
                chosen += 1;
                let labels = json_strings(&case["market"]["outcomes"]);
                let expected: Vec<OutcomeId> = strings(pair)
                    .into_iter()
                    .zip(labels)
                    .map(|(id, outcome)| OutcomeId { outcome, id })
                    .collect();
                assert_eq!(result.as_ref(), Ok(&expected), "{name}");
            }
        }
    }
    assert_eq!((chosen, refused), (2, 4), "the vectors changed shape");
}

/// py-sdk records only that it refused; which refusal the selector gives is
/// the rule of the design, read off each case's input.
#[test]
fn each_refused_vector_is_refused_for_its_reason() {
    let vectors = vectors();
    let outcomes = |case: &Value| json_strings(&case["market"]["outcomes"]).len();

    let case_of = |name| case(&vectors, name);
    assert_eq!(
        market_of(&case_of("missing_version")["market"]).outcome_ids(),
        Err(OutcomeIdsError::MissingVersion)
    );
    assert_eq!(
        market_of(&case_of("unknown_version")["market"]).outcome_ids(),
        Err(OutcomeIdsError::UnsupportedVersion("v3".to_owned()))
    );
    let one = case_of("v2_one_position_id");
    assert_eq!(
        market_of(&one["market"]).outcome_ids(),
        Err(OutcomeIdsError::CountMismatch {
            outcomes: outcomes(one),
            ids: strings(&one["market"]["positionIds"]).len(),
        })
    );
    assert_eq!(
        market_of(&case_of("v1_no_clob_token_ids")["market"]).outcome_ids(),
        Err(OutcomeIdsError::MissingIds)
    );
}

/// The vectors that carry both fields differ in them, and the version alone
/// picks: `v1` takes `clobTokenIds`, `v2` takes `positionIds`.
#[test]
fn the_version_picks_the_field_when_both_are_present() {
    let vectors = vectors();
    for (name, takes_position_ids) in [("v1_with_position_ids", false), ("v2_with_both", true)] {
        let case = case(&vectors, name);
        let clob = json_strings(&case["market"]["clobTokenIds"]);
        let position = strings(&case["market"]["positionIds"]);
        assert_ne!(
            clob, position,
            "{name}: the fields must differ to tell them apart"
        );

        let got: Vec<String> = market_of(&case["market"])
            .outcome_ids()
            .unwrap()
            .into_iter()
            .map(|outcome| outcome.id)
            .collect();
        assert_eq!(
            got,
            if takes_position_ids { position } else { clob },
            "{name}"
        );
    }
}

#[test]
fn a_market_with_three_outcomes_pairs_each_in_order() {
    let labels = r#"["A","B","C"]"#;
    let v1 = market(Some("v1"), Some(r#"["11","22","33"]"#), None, Some(labels));
    assert_eq!(
        v1.outcome_ids(),
        Ok(pairs(&["A", "B", "C"], &["11", "22", "33"]))
    );
    let v2 = market(Some("v2"), None, Some(&["44", "55", "66"]), Some(labels));
    assert_eq!(
        v2.outcome_ids(),
        Ok(pairs(&["A", "B", "C"], &["44", "55", "66"]))
    );
}

#[test]
fn a_different_number_of_outcomes_and_ids_is_a_count_mismatch() {
    let more_ids = market(
        Some("v1"),
        Some(r#"["1","2","3"]"#),
        None,
        Some(r#"["Yes","No"]"#),
    );
    assert_eq!(
        more_ids.outcome_ids(),
        Err(OutcomeIdsError::CountMismatch {
            outcomes: 2,
            ids: 3
        })
    );
    let fewer_ids = market(Some("v2"), None, Some(&["1"]), Some(r#"["Yes","No"]"#));
    assert_eq!(
        fewer_ids.outcome_ids(),
        Err(OutcomeIdsError::CountMismatch {
            outcomes: 2,
            ids: 1
        })
    );
    // The count is checked before the digits, so a short list of bad ids is a
    // count mismatch.
    let both = market(Some("v1"), Some(r#"["x"]"#), None, Some(r#"["Yes","No"]"#));
    assert_eq!(
        both.outcome_ids(),
        Err(OutcomeIdsError::CountMismatch {
            outcomes: 2,
            ids: 1
        })
    );
}

#[test]
fn an_id_that_is_not_all_ascii_digits_is_refused_with_the_id() {
    // Empty, letters, a sign, a prefix, a fraction, a space and full-width
    // digits are all refused; none is a decimal integer.
    for bad in ["", "abc", "-1", "0x10", "1.5", " 7", "７"] {
        let ids = serde_json::to_string(&["12", bad]).unwrap();
        let v1 = market(
            Some("v1"),
            Some(ids.as_str()),
            None,
            Some(r#"["Yes","No"]"#),
        );
        assert_eq!(
            v1.outcome_ids(),
            Err(OutcomeIdsError::InvalidId(bad.to_owned())),
            "v1 {bad:?}"
        );
        let v2 = market(
            Some("v2"),
            None,
            Some(&["12", bad]),
            Some(r#"["Yes","No"]"#),
        );
        assert_eq!(
            v2.outcome_ids(),
            Err(OutcomeIdsError::InvalidId(bad.to_owned())),
            "v2 {bad:?}"
        );
    }
}

#[test]
fn outcomes_that_are_not_a_json_array_of_strings_are_malformed() {
    for outcomes in [
        None,
        Some("not json"),
        Some(r#"{"a":"b"}"#),
        Some("[1,2]"),
        Some(r#"["Yes",null]"#),
    ] {
        let v1 = market(Some("v1"), Some(r#"["1","2"]"#), None, outcomes);
        assert_eq!(
            v1.outcome_ids(),
            Err(OutcomeIdsError::MalformedOutcomes),
            "v1 {outcomes:?}"
        );
        let v2 = market(Some("v2"), None, Some(&["1", "2"]), outcomes);
        assert_eq!(
            v2.outcome_ids(),
            Err(OutcomeIdsError::MalformedOutcomes),
            "v2 {outcomes:?}"
        );
    }
}

#[test]
fn clob_token_ids_that_are_not_a_json_array_of_strings_are_malformed() {
    for ids in ["not json", r#"{"a":"b"}"#, "[1,2]", r#""12""#] {
        // The `positionIds` beside them are fine and never read for `v1`.
        let market = market(
            Some("v1"),
            Some(ids),
            Some(&["1", "2"]),
            Some(r#"["Yes","No"]"#),
        );
        assert_eq!(
            market.outcome_ids(),
            Err(OutcomeIdsError::MalformedIds),
            "{ids:?}"
        );
    }
}

#[test]
fn the_field_the_version_does_not_name_is_never_read() {
    let labels = Some(r#"["Yes","No"]"#);
    // A `v2` market takes `positionIds` whatever `clobTokenIds` holds.
    let v2 = market(Some("v2"), Some("garbage"), Some(&["7", "8"]), labels);
    assert_eq!(v2.outcome_ids(), Ok(pairs(&["Yes", "No"], &["7", "8"])));
    // A `v1` market takes `clobTokenIds` whatever `positionIds` holds.
    let v1 = market(
        Some("v1"),
        Some(r#"["5","6"]"#),
        Some(&["abc", "def", "ghi"]),
        labels,
    );
    assert_eq!(v1.outcome_ids(), Ok(pairs(&["Yes", "No"], &["5", "6"])));
}

#[test]
fn the_field_the_version_names_must_be_present() {
    let labels = Some(r#"["Yes","No"]"#);
    // Present in the other field, absent in the named one.
    let v1 = market(Some("v1"), None, Some(&["1", "2"]), labels);
    assert_eq!(v1.outcome_ids(), Err(OutcomeIdsError::MissingIds));
    let v2 = market(Some("v2"), Some(r#"["1","2"]"#), None, labels);
    assert_eq!(v2.outcome_ids(), Err(OutcomeIdsError::MissingIds));
}

#[test]
fn the_version_is_checked_before_anything_else() {
    // No version, and nothing else either.
    assert_eq!(
        market(None, None, None, None).outcome_ids(),
        Err(OutcomeIdsError::MissingVersion)
    );
    // A version this client does not know, beside ids that would be fine.
    for version in ["v3", "V1", ""] {
        let unknown = market(
            Some(version),
            Some(r#"["1","2"]"#),
            Some(&["1", "2"]),
            Some(r#"["Yes","No"]"#),
        );
        assert_eq!(
            unknown.outcome_ids(),
            Err(OutcomeIdsError::UnsupportedVersion(version.to_owned())),
            "{version:?}"
        );
    }
}

#[test]
fn the_ids_are_read_before_the_outcomes() {
    let both_bad = market(Some("v1"), Some("not json"), None, Some("not json"));
    assert_eq!(both_bad.outcome_ids(), Err(OutcomeIdsError::MalformedIds));
}
