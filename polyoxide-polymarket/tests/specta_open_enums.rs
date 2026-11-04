//! The open enums export to TypeScript as named open unions (spec D17): every
//! known wire spelling, then `OpenString`, which is `string & {}`, so a
//! TypeScript consumer keeps the literals and an unknown spelling still
//! type-checks, as `Other(raw)` decodes it.

use std::collections::BTreeMap;

use polyoxide_polymarket::{data::types as data, gamma::types as gamma};
use specta::{Type, TypeMap};
use specta_typescript::Typescript;

const OPEN_STRING: &str = "export type OpenString = (string) & ({ [key in never]: never })";

/// Every named type `register` puts in a type map, by name, as
/// specta-typescript exports it.
fn exports(register: impl FnOnce(&mut TypeMap)) -> BTreeMap<String, String> {
    let mut map = TypeMap::default();
    register(&mut map);
    assert_eq!(specta::internal::detect_duplicate_type_names(&map), vec![]);
    let ts = Typescript::default();
    map.iter()
        .map(|(_, named)| {
            let text = specta_typescript::export_named_datatype(&ts, named, &map).unwrap();
            (named.name().to_string(), text)
        })
        .collect()
}

/// `export type Name = "A" | "B" | OpenString`, from the enum's own spellings.
fn union<'a>(name: &str, spellings: impl IntoIterator<Item = &'a str>) -> String {
    let known: Vec<String> = spellings.into_iter().map(|s| format!("\"{s}\"")).collect();
    format!("export type {name} = {} | OpenString", known.join(" | "))
}

macro_rules! open_unions {
    ($($ty:ty => $name:literal),* $(,)?) => {{
        let exported = exports(|map| {
            $( let _ = <$ty as Type>::reference(map, &[]); )*
        });
        let open = exported.get("OpenString").expect("OpenString is exported by name");
        assert!(open.ends_with(OPEN_STRING), "{open}");
        $(
            let spellings = <$ty>::ALL.iter().map(|v| v.as_str());
            assert_eq!(exported[$name], union($name, spellings));
        )*
        assert_eq!(exported.len(), [$($name),*].len() + 1, "{exported:#?}");
    }};
}

#[test]
fn every_open_enum_exports_its_known_spellings_and_open_string() {
    open_unions! {
        data::TradeSide => "TradeSide",
        data::ActivitySide => "ActivitySide",
        data::ActivityType => "ActivityType",
        data::PositionStatus => "PositionStatus",
        data::TokenModule => "TokenModule",
        gamma::ProtocolVersion => "ProtocolVersion",
        gamma::ResolutionStatus => "ResolutionStatus",
        gamma::HomeAway => "HomeAway",
    }
}

#[test]
fn a_field_of_an_open_enum_names_the_union() {
    // A record's field refers to the named union, not an inlined `string`.
    let exported = exports(|map| {
        let _ = <data::Trade as Type>::reference(map, &[]);
    });
    assert!(
        exported["Trade"].contains("side: TradeSide"),
        "{}",
        exported["Trade"]
    );
    assert!(exported.contains_key("TradeSide"), "{exported:#?}");
}
