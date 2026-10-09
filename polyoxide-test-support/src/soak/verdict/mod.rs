//! The two rulebooks that judge a harness's stages.
//!
//! They are two policies, not copies of one, and stay apart:
//!
//! - [`tolerant`] keeps per-request samples, tells the origin's 429 from the
//!   CDN's IP block, invalidates a stage at 1% errors, a repeated URL or any
//!   cache hit, reads saturation off a latency climb over the first stage, and
//!   pins a count or says why it cannot.
//! - [`strict`] keeps replies only, invalidates a stage at any error, reads
//!   saturation off the share served from the cache, and pins the last clean
//!   rate or nothing.

pub mod strict;
pub mod tolerant;
