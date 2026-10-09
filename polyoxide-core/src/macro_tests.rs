//! Tests for the macros a venue crate invokes, from outside `macros.rs` so
//! that file only grows at its end.

/// A client field that counts its clones.
#[derive(Debug, Default)]
struct Handle {
    name: &'static str,
    clones: std::cell::Cell<u32>,
}

impl Clone for Handle {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            name: self.name,
            clones: std::cell::Cell::new(0),
        }
    }
}

struct Markets {
    http_client: Handle,
    signer: Handle,
}

struct Pnl {
    http_client: Handle,
}

struct Client {
    http_client: Handle,
    signer: Handle,
    pnl_http_client: Handle,
}

impl Client {
    crate::namespaces! { http_client, signer;
        /// The markets namespace.
        markets: Markets,
        /// The PnL namespace, on its own host.
        #[must_use]
        pnl: Pnl { http_client: pnl_http_client },
    }
}

#[test]
fn namespaces_clone_the_listed_fields_and_map_one_from_another() {
    let client = Client {
        http_client: Handle {
            name: "main",
            ..Handle::default()
        },
        signer: Handle {
            name: "signer",
            ..Handle::default()
        },
        pnl_http_client: Handle {
            name: "pnl",
            ..Handle::default()
        },
    };

    let markets = client.markets();
    assert_eq!(markets.http_client.name, "main");
    assert_eq!(markets.signer.name, "signer");
    assert_eq!(client.http_client.clones.get(), 1);
    assert_eq!(client.signer.clones.get(), 1);

    let pnl = client.pnl();
    assert_eq!(pnl.http_client.name, "pnl");
    assert_eq!(client.pnl_http_client.clones.get(), 1);
    assert_eq!(
        client.http_client.clones.get(),
        1,
        "a mapped accessor clones only the field it names"
    );
}

/// A builder with one setter of each `query_setters!` arm, on core's
/// `Request`.
struct Things {
    request: crate::Request<(), crate::ApiError>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Open,
    Unknown,
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Open => "OPEN",
            Self::Unknown => "UNKNOWN",
        })
    }
}

impl Things {
    crate::query_setters! {
        /// A plain value.
        limit: u32 => "limit",
        /// A string.
        title: impl Into<String> => "title",
        /// A repeated key.
        ids: many impl IntoIterator<Item = i64> => "id",
        /// Comma-joined.
        tags: csv impl IntoIterator<Item = impl ToString> => "tags",
        /// Computed from the argument.
        open(open: bool) => "closed" = !open,
        /// Sent only when the condition holds.
        kind(kind: Kind) => "kind" if kind != Kind::Unknown,
        /// Comma-joined, computed from the argument.
        kinds(kinds: impl IntoIterator<Item = Kind>) => csv "kinds"
            = kinds.into_iter().filter(|k| *k != Kind::Unknown),
    }
}

/// A builder whose request sits in another field, with explicit generics.
struct Paged {
    inner: crate::Request<(), crate::ApiError>,
}

impl Paged {
    crate::query_setters! { self.inner;
        /// Comma-joined, with explicit generics.
        conditions: csv<I, S> => "condition",
    }
}

fn things() -> Things {
    let http = crate::HttpClientBuilder::new("https://example.com")
        .build()
        .unwrap();
    Things {
        request: crate::Request::new(http, "/things"),
    }
}

fn pairs(query: &[(String, String)]) -> Vec<(&str, &str)> {
    query
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect()
}

#[test]
fn query_setters_plain() {
    let t = things().limit(5u32).limit(6u32);
    assert_eq!(pairs(&t.request.query), [("limit", "5"), ("limit", "6")]);
}

#[test]
fn query_setters_into_string() {
    let t = things().title("a b").title(String::from("c"));
    assert_eq!(pairs(&t.request.query), [("title", "a b"), ("title", "c")]);
}

#[test]
fn query_setters_many() {
    let t = things().ids([1i64, 2]).ids([]);
    assert_eq!(pairs(&t.request.query), [("id", "1"), ("id", "2")]);
}

#[test]
fn query_setters_csv() {
    let t = things()
        .tags(["a", "b"])
        .tags(Vec::<String>::new())
        .tags([""]);
    assert_eq!(pairs(&t.request.query), [("tags", "a,b")]);
}

#[test]
fn query_setters_csv_generic_on_another_field() {
    let http = crate::HttpClientBuilder::new("https://example.com")
        .build()
        .unwrap();
    let p = Paged {
        inner: crate::Request::new(http, "/rows"),
    }
    .conditions(["0xa", "0xb"])
    .conditions(Vec::<&str>::new());
    assert_eq!(pairs(&p.inner.query), [("condition", "0xa,0xb")]);
}

#[test]
fn query_setters_expr() {
    let t = things().open(true).open(false);
    assert_eq!(
        pairs(&t.request.query),
        [("closed", "false"), ("closed", "true")]
    );
}

#[test]
fn query_setters_if() {
    let t = things().kind(Kind::Unknown).kind(Kind::Open);
    assert_eq!(pairs(&t.request.query), [("kind", "OPEN")]);
}

#[test]
fn query_setters_csv_expr() {
    let t = things()
        .kinds([Kind::Open, Kind::Unknown, Kind::Open])
        .kinds([Kind::Unknown]);
    assert_eq!(pairs(&t.request.query), [("kinds", "OPEN,OPEN")]);
}
