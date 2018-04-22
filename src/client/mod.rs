//! The SMTP client implementation.
//!
//! The client is implemented as a [tokio-proto] streaming pipeline protocol.
//!
//!  [tokio-proto]: https://docs.rs/tokio-proto/
//!
//! # Example
//!
//! ```no_run
//! extern crate futures;
//! extern crate tokio_core;
//! extern crate tokio_proto;
//! extern crate tokio_service;
//! extern crate tokio_smtp;
//!
//! use futures::future;
//! use futures::{Future, Sink};
//! use std::net::{ToSocketAddrs};
//! use tokio_core::reactor::{Core};
//! use tokio_proto::streaming::{Body, Message};
//! use tokio_service::{Service};
//! use tokio_smtp::request::{Request as SmtpRequest};
//! use tokio_smtp::client::{Client as SmtpClient};
//!
//! // In this example, we grab the mail body from a fixture.
//! const TEST_EML: &'static str = include_str!("../fixtures/test.eml");
//!
//! fn main() {
//!     // Create the event loop that will drive this server.
//!     let mut core = Core::new().unwrap();
//!     let handle = core.handle();
//!
//!     // Create a client. `Client` parameters are used in the SMTP and TLS
//!     // handshake, but do not set the address and port to connect to.
//!     let client = SmtpClient::localhost(None);
//!
//!     // Make a connection to an SMTP server. Here, we use the default address
//!     // that MailHog listens on. This also takes care of TLS, if set in the
//!     // `Client` parameters, and sends the `EHLO` command.
//!     let addr = "localhost:1025".to_socket_addrs().unwrap().next().unwrap();
//!     let f = client.connect(&addr, &handle)
//!
//!         // The future results in a service instance.
//!         .and_then(|service| {
//!
//!             // Create a body sink and stream. The stream is consumed when the
//!             // `DATA` command is sent. We asynchronously write the mail body
//!             // to the stream by spawning another future on the core.
//!             let (body_sender, body) = Body::pair();
//!             handle.spawn(
//!                 body_sender.send(Ok((TEST_EML.as_ref() as &[u8]).to_vec()))
//!                     .map_err(|e| panic!("body send error: {:?}", e))
//!                     .and_then(|_| future::ok(()))
//!             );
//!
//!             // Following the `EHLO` handshake, send `MAIL FROM`, `RCPT TO`,
//!             // and `DATA` with the body, then finally `QUIT`.
//!             future::join_all(vec![
//!                 service.call(Message::WithoutBody(SmtpRequest::Mail {
//!                     from: "john@example.test".parse().unwrap(),
//!                     params: vec![],
//!                 })),
//!                 service.call(Message::WithoutBody(SmtpRequest::Rcpt {
//!                     to: "alice@example.test".parse().unwrap(),
//!                     params: vec![],
//!                 })),
//!                 service.call(Message::WithBody(SmtpRequest::Data, body)),
//!                 service.call(Message::WithoutBody(SmtpRequest::Quit)),
//!             ])
//!
//!         })
//!
//!         // This future results in a `Vec` of messages. Responses from
//!         // the server are always `Message::WithoutBody`.
//!         .and_then(|responses| {
//!
//!             // Grab the `Response` from the `Message`, and print it.
//!             for response in responses {
//!                 println!("{:?}", response.into_inner());
//!             }
//!
//!             future::ok(())
//!
//!         });
//!
//!     // Start the client on the event loop.
//!     core.run(f).unwrap();
//! }
//! ```

mod codec;
mod io;
mod handshake;
mod auth;
mod proto;

use futures::{Future};
use native_tls::{Result as TlsResult, TlsConnector};
use client::codec::{ClientCodec};
use client::io::{ClientIo};
use request::{ClientId};
use std::io::{Error as IoError};
use std::sync::{Arc};
use tokio_io::codec::{Framed};
use tokio_proto::{TcpClient as TokioTcpClient};
use tokio_proto::streaming::{Body};
use tokio_proto::streaming::pipeline::{StreamingPipeline};

// FIXME: `<T: Io + 'static>`, but E0122
pub type ClientTransport<T> = Framed<ClientIo<T>, ClientCodec>;
pub type ClientBindTransport<T> = Box<Future<Item = ClientTransport<T>, Error = IoError>>;
pub type TcpClient = TokioTcpClient<StreamingPipeline<Body<Vec<u8>, IoError>>, ClientProto>;

pub use client::auth::{ClientAuth};
pub use client::proto::{ClientProto};

/// Parameters to use for secure clients
pub struct ClientTlsParams {
    /// A connector from `native-tls`
    pub connector: TlsConnector,
    /// The domain to send during the TLS handshake
    pub sni_domain: String,
}


/// How to apply TLS to a client connection
pub enum ClientSecurity {
    /// Insecure connection
    None,
    /// Use `STARTTLS`, allow rejection
    Optional(ClientTlsParams),
    /// Use `STARTTLS`, fail on rejection
    Required(ClientTlsParams),
    /// Use TLS without negotation
    Immediate(ClientTlsParams),
}


/// Parameters to use during the client handshake
pub struct ClientParams {
    /// Client identifier, the parameter to `EHLO`
    pub id: ClientId,
    /// Whether to use a secure connection, and how
    pub security: ClientSecurity,
    /// Authentication data
    pub auth: Option<ClientAuth>,
}


/// Utility for creating a `TcpClient`
///
/// This unit struct itself serves no real purpose, but contains constructor
/// methods for creating a `TcpClient` set up with the SMTP protocol.
pub struct Client;

impl Client {
    /// Setup a client for connecting to the local server
    pub fn localhost(auth: Option<ClientAuth>) -> TcpClient {
        Self::insecure(ClientId::Domain("localhost".to_string()), auth)
    }

    /// Setup a client for connecting without TLS
    pub fn insecure(id: ClientId, auth: Option<ClientAuth>) -> TcpClient {
        Self::with_params(ClientParams {
            security: ClientSecurity::None,
            id, auth,
        })
    }

    /// Setup a client for connecting with TLS using STARTTLS
    pub fn secure(id: ClientId, sni_domain: String, auth: Option<ClientAuth>) -> TlsResult<TcpClient> {
        Ok(Self::with_params(ClientParams {
            security: ClientSecurity::Required(ClientTlsParams {
                connector: TlsConnector::builder()
                    .and_then(|builder| builder.build())?,
                sni_domain,
            }),
            id, auth,
        }))
    }

    /// Setup a client for connecting with TLS on a secure port
    pub fn secure_port(id: ClientId, sni_domain: String, auth: Option<ClientAuth>) -> TlsResult<TcpClient> {
        Ok(Self::with_params(ClientParams {
            security: ClientSecurity::Immediate(ClientTlsParams {
                connector: TlsConnector::builder()
                    .and_then(|builder| builder.build())?,
                sni_domain,
            }),
            id, auth,
        }))
    }

    /// Setup a client using custom parameters
    pub fn with_params(params: ClientParams) -> TcpClient {
        TokioTcpClient::new(ClientProto(Arc::new(params)))
    }
}
