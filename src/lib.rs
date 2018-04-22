//! An SMTP library for [Tokio].
//!
//! The toplevel module exports a basic interface to send mail, through the
//! `Mailer` type. This interface is hopefully sufficient for the common use
//! case where mail just needs to be delivered to a trusted local mail server
//! or remote mail service.
//!
//! A low-level client implementation on top of [tokio-proto] is available in
//! [the client module](client/). The server-side is not yet implemented.
//!
//!  [Tokio]: https://tokio.rs/
//!  [tokio-proto]: https://docs.rs/tokio-proto/
//!
//! # Example
//!
//! ```no_run
//! extern crate tokio_core;
//! extern crate tokio_smtp;
//!
//! use tokio_core::reactor::{Core};
//! use tokio_smtp::{Mailer};
//!
//! // In this example, we grab the mail body from a fixture.
//! const TEST_EML: &'static str = include_str!("fixtures/test.eml");
//!
//! fn main() {
//!     // Create the event loop that will drive this server.
//!     let mut core = Core::new().unwrap();
//!     let handle = core.handle();
//!
//!     // Create a mailer that delivers to `localhost:25`.
//!     let mailer = Mailer::local();
//!
//!     // Send an email. The `send` method returns an empty future (`()`).
//!     let return_path = "john@example.test".parse().unwrap();
//!     let recipient = "alice@example.test".parse().unwrap();
//!     let body = TEST_EML.to_string();
//!     let f = mailer.send(return_path, vec![recipient], body, &handle);
//!
//!     // Start the client on the event loop.
//!     core.run(f).unwrap();
//! }
//! ```

// FIXME: Add server protocol

extern crate emailaddress;
extern crate base64;
extern crate futures;
extern crate native_tls;
#[macro_use]
extern crate nom;
extern crate bytes;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;
extern crate tokio_io;
extern crate tokio_tls;
#[macro_use]
extern crate log;

pub mod mailbody;
pub mod client;
pub mod request;
pub mod response;
pub mod mailer;
mod util;

pub use mailbody::{MailBody, IntoMailBody};
pub use client::{ClientParams, ClientAuth, ClientSecurity, ClientTlsParams};
pub use mailer::{Mailer, MailerBuilder};
