//! # polyoxide-venue
//!
//! The vocabulary every polyoxide crate shares, whichever venue it talks to.
//!
//! - [`Class`] sorts every failure into one of eight classes, and [`Classify`],
//!   implemented by every public error type in the workspace, reports an
//!   error's class, whether it is a fault, and how long the server asked the
//!   caller to wait. [`ClassifiedError`] carries any of them.
//! - [`class_for_status`], [`class_for_close_code`] and
//!   [`class_for_handshake_status`] are the one status and close-code table.
//! - [`parse_retry_after`] is the one `Retry-After` parser, and
//!   [`retry_delay`] lets it only lengthen the client's own backoff.
//! - [`Secret`] holds a value its `Debug` never prints.
//!
//! The crate depends on nothing, so a credential-free socket crate can use it
//! without building an HTTP or signing stack.
//!
//! ```
//! use polyoxide_venue::{class_for_status, Class};
//!
//! let class = class_for_status(503).unwrap().with_code("dependency_unavailable");
//! assert!(class.is_retriable());
//! assert_eq!(class.code(), Some("dependency_unavailable"));
//! assert_eq!(class_for_status(451), Some(Class::Restricted));
//! ```

#![warn(missing_docs)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

mod class;
mod retry_after;
mod secret;
mod socket;
mod status;

pub use class::{Class, ClassifiedError, Classify};
pub use retry_after::{parse_retry_after, retry_delay};
pub use secret::Secret;
pub use socket::{class_for_close_code, class_for_handshake_status};
pub use status::class_for_status;
