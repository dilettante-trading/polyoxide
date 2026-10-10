//! [`ClobThrottle`]: the CLOB's two rate limit layers over one hold.

use std::time::Duration;

use crate::hold::Hold;
use crate::hooks::{
    AttemptInfo, Charge, Cost, LayerCharge, LayerId, Refused, RequestMeta, ResponseMeta, Throttle,
};
use crate::rate_limit::RateLimiter;
use crate::signer_limit::{SignerLimiter, TradingBucket, TradingRequest};

use super::clob_limits;

/// Cloudflare's IP layer, which counts requests.
pub const CLOUDFLARE: LayerId = LayerId("cloudflare");
/// The per-signer order bucket, which counts orders placed.
pub const SIGNER_ORDER: LayerId = LayerId("signer-order");
/// The per-signer cancel bucket, which counts orders cancelled.
pub const SIGNER_CANCEL: LayerId = LayerId("signer-cancel");

/// What `request` costs the per-signer layer, as the [`Cost`] a request
/// builder hands [`HttpClient::send`](crate::HttpClient::send).
///
/// Exact for the four request kinds whose cost is a function of the payload;
/// `cancel-all` and `cancel-market-orders` report their floor of 1 as
/// inexact, so they are never refused (see [`TradingRequest::cost`]).
pub fn signer_cost(request: TradingRequest) -> Cost {
    Cost {
        layer: match request.bucket() {
            TradingBucket::Order => SIGNER_ORDER,
            TradingBucket::Cancel => SIGNER_CANCEL,
        },
        units: request.cost().max(1),
        exact: request.cost_is_exact(),
    }
}

/// The CLOB's throttle: Cloudflare's window quotas, which count requests,
/// composed with the per-signer buckets, which count orders, over one
/// [`Hold`].
///
/// Each attempt waits out the hold, is charged one request against the IP
/// table by its method and path, then is charged the signer bucket its
/// [`signer_cost`] names, and waits out the hold again if it moved meanwhile.
/// An exact cost above the signer bucket's capacity is [`Refused`] after the
/// IP charge, as the CLOB's own loop refuses it today, and nothing is sent.
///
/// Every response is read for `Poly-RateLimit-*` telemetry, whatever its
/// status, so a tier reported on a 429 is adopted. A hold (a 429) stops both
/// layers, and survives a tier change.
#[derive(Debug, Clone)]
pub struct ClobThrottle {
    table: RateLimiter,
    signer: SignerLimiter,
    hold: Hold,
}

impl ClobThrottle {
    /// Compose `table` and `signer` over `table`'s hold.
    ///
    /// `signer` is moved onto that hold with
    /// [`SignerLimiter::with_hold`], which makes a new limiter: read the
    /// signer layer's tier and telemetry through [`signer`](Self::signer).
    pub fn new(table: RateLimiter, signer: SignerLimiter) -> Self {
        let hold = table.hold().clone();
        Self {
            signer: signer.with_hold(hold.clone()),
            table,
            hold,
        }
    }

    /// The IP layer.
    pub fn table(&self) -> &RateLimiter {
        &self.table
    }

    /// The per-signer layer, with the tier and telemetry it has adopted.
    pub fn signer(&self) -> &SignerLimiter {
        &self.signer
    }
}

impl Throttle for ClobThrottle {
    async fn acquire(&self, meta: &RequestMeta<'_>) -> Result<Charge, Refused> {
        self.hold.wait().await;
        self.table.acquire(meta.path, Some(meta.method)).await;
        let mut charge = Charge::none().with(LayerCharge {
            layer: CLOUDFLARE,
            units: 1,
            window: None,
        });
        for cost in meta.costs {
            let bucket = match cost.layer {
                SIGNER_ORDER => TradingBucket::Order,
                SIGNER_CANCEL => TradingBucket::Cancel,
                // Not a layer of this throttle.
                _ => continue,
            };
            self.signer.charge(bucket, cost.units, cost.exact).await?;
            charge = charge.with(LayerCharge {
                layer: cost.layer,
                units: cost.units,
                window: None,
            });
        }
        // AD-23: a hold set while this attempt waited on a bucket is honoured.
        self.hold.wait().await;
        Ok(charge)
    }

    fn observe(&self, _charge: &Charge, response: &ResponseMeta<'_>, _attempt: &AttemptInfo) {
        self.signer.observe(response.headers);
    }

    fn hold(&self, delay: Duration) {
        self.hold.extend(delay);
    }
}

/// The CLOB's throttle: [`clob_limits`] and a [`SignerLimiter`] at the
/// tightest tier, over one hold.
pub fn clob_throttle() -> ClobThrottle {
    ClobThrottle::new(clob_limits(), SignerLimiter::new())
}
