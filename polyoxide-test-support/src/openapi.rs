//! Agreement between a type and its schema in a vendored OpenAPI document.
//!
//! [`check`] holds one type to one schema, from values synthesised out of the
//! schema itself:
//!
//! 1. **Optionality.** A property that is required and not nullable must be a
//!    plain field: removing it from the minimal object must fail to decode.
//!    Every other property must accept `null`.
//! 2. **Field names.** A fully populated value must encode back to exactly the
//!    schema's property set, plus the wire-only fields the caller names. Serde
//!    ignores unknown keys on the way in, so without this a struct that forgot
//!    or misspelled a field would pass (1).
//!
//! An `allOf` schema is read as the one flat object the server sends.

use std::collections::BTreeSet;

use serde::{de::DeserializeOwned, Serialize};
use serde_json::{Map, Value};

/// The document's `components.schemas`.
#[track_caller]
pub fn schemas(spec: &Value) -> Map<String, Value> {
    spec["components"]["schemas"]
        .as_object()
        .expect("components.schemas")
        .clone()
}

/// The schema name a `$ref` points at: its last path segment.
pub fn ref_name(r: &str) -> &str {
    r.rsplit('/').next().unwrap()
}

/// Whether a property admits `null`: a `null` in its type list, a `null` arm
/// in its `oneOf`, or `nullable: true`. A `$ref` is followed, since a document
/// may put `nullable` on the target rather than on the property.
#[track_caller]
pub fn is_nullable(schemas: &Map<String, Value>, prop: &Value) -> bool {
    if let Some(r) = prop["$ref"].as_str() {
        return is_nullable(schemas, &schemas[ref_name(r)]);
    }
    if let Some(types) = prop["type"].as_array() {
        return types.iter().any(|t| t == "null");
    }
    prop["nullable"] == true
        || prop["oneOf"]
            .as_array()
            .is_some_and(|arms| arms.iter().any(|a| a["type"] == "null"))
}

/// A value of the property's type. `full` also fills non-required properties
/// of any object it descends into.
///
/// An `enum` gives its first value. A positional row (an array with no
/// `items`, or with primitive `items`) is taken from its `example`, and so is a
/// string, since a decimal field typed `string` carries a decimal spelled as a
/// string there, and a decimal type would reject a placeholder.
#[track_caller]
pub fn synth(schemas: &Map<String, Value>, prop: &Value, full: bool) -> Value {
    if let Some(r) = prop["$ref"].as_str() {
        let target = &schemas[ref_name(r)];
        if target["type"] == "object"
            || target.get("allOf").is_some()
            || target.get("properties").is_some()
        {
            return synth_object(schemas, ref_name(r), full);
        }
        return synth(schemas, target, full);
    }
    if let Some(arms) = prop["oneOf"].as_array() {
        let arm = arms
            .iter()
            .find(|a| a["type"] != "null")
            .expect("non-null arm");
        return synth(schemas, arm, full);
    }
    if let Some(e) = prop["enum"].as_array() {
        return e[0].clone();
    }
    let ty = match &prop["type"] {
        Value::String(t) => t.as_str(),
        Value::Array(ts) => ts
            .iter()
            .filter_map(Value::as_str)
            .find(|t| *t != "null")
            .unwrap(),
        other => panic!("unsupported type {other} in {prop}"),
    };
    match ty {
        "string" => prop
            .get("example")
            .filter(|e| e.is_string())
            .cloned()
            .unwrap_or_else(|| Value::from("x")),
        "integer" => Value::from(1),
        "number" => Value::from(1.5),
        "boolean" => Value::from(true),
        "array" => {
            let items = prop.get("items");
            let positional = items.is_none_or(|i| i.get("$ref").is_none() && i["type"] != "object");
            match prop.get("example") {
                Some(example) if positional => example.clone(),
                _ => {
                    // Not a closure: `#[track_caller]` does not reach into one.
                    let Some(items) = items else {
                        panic!("array without items or example: {prop}")
                    };
                    Value::Array(vec![synth(schemas, items, full)])
                }
            }
        }
        "object" => synth_object_inline(schemas, prop, full),
        other => panic!("unsupported type {other}"),
    }
}

/// An object schema's properties and required names. An `allOf` contributes
/// every arm's, so a schema that extends another reads as the one flat row the
/// server sends. A property declared by two arms fails: the flat row could not
/// say which declaration it follows.
#[track_caller]
pub fn fields(
    schemas: &Map<String, Value>,
    schema: &Value,
) -> (Map<String, Value>, BTreeSet<String>) {
    if let Some(r) = schema["$ref"].as_str() {
        return fields(schemas, &schemas[ref_name(r)]);
    }
    let own = schema["properties"].as_object();
    let arms = schema["allOf"].as_array();
    assert!(
        own.is_some() || arms.is_some(),
        "neither properties nor allOf: {schema}"
    );
    let mut props = own.cloned().unwrap_or_default();
    let mut required: BTreeSet<String> = schema["required"]
        .as_array()
        .map(|r| {
            r.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    for arm in arms.into_iter().flatten() {
        let (arm_props, arm_required) = fields(schemas, arm);
        for (key, prop) in arm_props {
            assert!(
                props.insert(key.clone(), prop).is_none(),
                "{key} is declared by two allOf arms"
            );
        }
        required.extend(arm_required);
    }
    (props, required)
}

/// An object for the inline schema `schema`: its required, non-nullable
/// properties, or with `full` every property.
#[track_caller]
pub fn synth_object_inline(schemas: &Map<String, Value>, schema: &Value, full: bool) -> Value {
    let (props, required) = fields(schemas, schema);
    let mut out = Map::new();
    for (key, prop) in &props {
        if full || (required.contains(key) && !is_nullable(schemas, prop)) {
            out.insert(key.clone(), synth(schemas, prop, full));
        }
    }
    Value::Object(out)
}

/// An object for the named schema, as [`synth_object_inline`] makes one.
#[track_caller]
pub fn synth_object(schemas: &Map<String, Value>, name: &str, full: bool) -> Value {
    synth_object_inline(schemas, &schemas[name], full)
}

/// Holds `T` to the schema `name`, as the module documentation describes.
///
/// `observed_extra` lists `(schema, field)` pairs the wire sends but the
/// document does not declare. Each must be optional on the type, and is
/// expected among the emitted keys. A pair the schema now declares fails, so
/// the list cannot go stale.
///
/// A required-but-nullable property is treated as omittable: it is left out
/// of the minimal object and set to `null` in the null check.
#[track_caller]
pub fn check<T: DeserializeOwned + Serialize>(
    schemas: &Map<String, Value>,
    name: &str,
    observed_extra: &[(&str, &str)],
) {
    let (props, required) = fields(schemas, &schemas[name]);

    for (_, field) in observed_extra.iter().filter(|(schema, _)| *schema == name) {
        assert!(
            !props.contains_key(*field),
            "{name}.{field} is now documented; drop the OBSERVED_EXTRA row and the OBSERVED.md entry"
        );
    }

    let minimal = synth_object(schemas, name, false);
    if let Err(e) = serde_json::from_value::<T>(minimal.clone()) {
        panic!("{name}: only required fields present should deserialize: {e}");
    }

    for (key, prop) in &props {
        if required.contains(key) && !is_nullable(schemas, prop) {
            let mut without = minimal.clone();
            without.as_object_mut().unwrap().remove(key);
            assert!(
                serde_json::from_value::<T>(without).is_err(),
                "{name}.{key} is required and non-nullable in the spec but the type accepts it missing"
            );
        } else {
            let mut with_null = minimal.clone();
            with_null
                .as_object_mut()
                .unwrap()
                .insert(key.clone(), Value::Null);
            if let Err(e) = serde_json::from_value::<T>(with_null) {
                panic!(
                    "{name}.{key} is optional or nullable in the spec but the type rejects null: {e}"
                );
            }
        }
    }

    let full = synth_object(schemas, name, true);
    let parsed: T = match serde_json::from_value(full) {
        Ok(parsed) => parsed,
        Err(e) => panic!("{name}: every field present should deserialize: {e}"),
    };
    let emitted = serde_json::to_value(&parsed).unwrap();
    let emitted: BTreeSet<&str> = emitted
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let mut documented: BTreeSet<&str> = props.keys().map(String::as_str).collect();
    documented.extend(
        observed_extra
            .iter()
            .filter(|(schema, _)| *schema == name)
            .map(|(_, field)| *field),
    );
    assert_eq!(
        emitted, documented,
        "{name}: emitted keys differ from the spec's properties"
    );
}

#[cfg(test)]
mod tests {
    use std::panic::catch_unwind;

    use serde::Deserialize;
    use serde_json::json;

    use super::*;

    fn doc() -> Map<String, Value> {
        schemas(&json!({"components": {"schemas": {
            "Row": {
                "type": "object",
                "required": ["id", "side", "price", "note"],
                "properties": {
                    "id": {"type": "integer"},
                    "side": {"$ref": "#/components/schemas/side"},
                    "price": {"type": "string", "example": "0.55"},
                    "note": {"type": ["string", "null"]},
                    "open": {"$ref": "#/components/schemas/maybe"},
                    "level": {"type": "array", "items": {"type": "string"}, "example": ["1.5", "2"]},
                    "child": {"type": "object", "properties": {"n": {"type": "number"}}}
                }
            },
            "side": {"type": "string", "enum": ["buy", "sell"]},
            "maybe": {"type": "boolean", "nullable": true},
            "Base": {"type": "object", "required": ["a"], "properties": {"a": {"type": "integer"}}},
            "Extended": {"allOf": [
                {"$ref": "#/components/schemas/Base"},
                {"type": "object", "properties": {"b": {"type": "boolean"}}}
            ]},
            "Clash": {"allOf": [
                {"$ref": "#/components/schemas/Base"},
                {"type": "object", "properties": {"a": {"type": "string"}}}
            ]}
        }}}))
    }

    #[test]
    fn the_minimal_object_holds_required_non_nullable_properties_only() {
        let schemas = doc();
        assert_eq!(
            synth_object(&schemas, "Row", false),
            json!({"id": 1, "side": "buy", "price": "0.55"})
        );
    }

    #[test]
    fn the_full_object_follows_refs_enums_examples_and_inline_objects() {
        let schemas = doc();
        assert_eq!(
            synth_object(&schemas, "Row", true),
            json!({
                "id": 1, "side": "buy", "price": "0.55", "note": "x", "open": true,
                "level": ["1.5", "2"], "child": {"n": 1.5}
            })
        );
    }

    #[test]
    fn nullability_follows_a_ref_to_its_target() {
        let schemas = doc();
        assert!(is_nullable(
            &schemas,
            &json!({"$ref": "#/components/schemas/maybe"})
        ));
        assert!(is_nullable(
            &schemas,
            &json!({"oneOf": [{"type": "null"}, {"type": "integer"}]})
        ));
        assert!(!is_nullable(&schemas, &json!({"type": "integer"})));
    }

    #[test]
    fn all_of_reads_as_one_flat_object_and_refuses_a_twice_declared_key() {
        let schemas = doc();
        let (props, required) = fields(&schemas, &schemas["Extended"]);
        assert_eq!(props.keys().collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(required, BTreeSet::from(["a".to_owned()]));
        let panic = catch_unwind(|| fields(&schemas, &schemas["Clash"])).unwrap_err();
        assert_eq!(
            panic.downcast_ref::<String>().unwrap(),
            "a is declared by two allOf arms"
        );
    }

    #[derive(Deserialize, Serialize)]
    struct Extended {
        a: i64,
        b: Option<bool>,
        seen: Option<u8>,
    }

    #[test]
    fn check_accepts_a_faithful_type_with_its_observed_extra() {
        check::<Extended>(&doc(), "Extended", &[("Extended", "seen")]);
    }

    #[derive(Deserialize, Serialize)]
    struct Loose {
        a: Option<i64>,
        b: Option<bool>,
    }

    #[test]
    fn check_refuses_a_required_field_made_optional() {
        let schemas = doc();
        let panic = catch_unwind(|| check::<Loose>(&schemas, "Extended", &[])).unwrap_err();
        assert_eq!(
            panic.downcast_ref::<String>().unwrap(),
            "Extended.a is required and non-nullable in the spec but the type accepts it missing"
        );
    }

    #[test]
    fn an_observed_extra_the_schema_now_declares_is_stale() {
        let schemas = doc();
        let panic = catch_unwind(|| check::<Extended>(&schemas, "Extended", &[("Extended", "b")]))
            .unwrap_err();
        assert!(panic
            .downcast_ref::<String>()
            .unwrap()
            .starts_with("Extended.b is now documented"));
    }
}
