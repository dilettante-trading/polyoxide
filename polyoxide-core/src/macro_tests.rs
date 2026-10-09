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
