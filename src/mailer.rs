use mailbody::{IntoMailBody};
use client::{ClientParams, ClientAuth, ClientProto, ClientSecurity, ClientTlsParams};
use futures::{Future};
use native_tls::{Result as TlsResult, TlsConnector};
use request::{ClientId, Mailbox};
use std::io::{Error as IoError, Result as IoResult};
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::{Arc};
use tokio_core::reactor::{Handle};
use tokio_proto::{TcpClient as TokioTcpClient};
use sender::{sendmail};

struct MailerParams {
    addrs: Vec<SocketAddr>,
    params: Arc<ClientParams>,
}


/// Object used to send mail to a specific server.
///
/// A `Mailer` is created using a `MailerBuilder`.
pub struct Mailer(Arc<MailerParams>);

impl Mailer {
    /// Alias for `MailerBuilder::new(server)`.
    pub fn builder(server: String) -> MailerBuilder {
        MailerBuilder::new(server)
    }

    /// Alias for `MailerBuilder::local().build()`.
    pub fn local() -> Self {
        MailerBuilder::local().build()
            .expect("failed to build mailer for local delivery")
    }

    /// Send an email.
    pub fn send<B: IntoMailBody>(&self, return_path: Mailbox, recipients: Vec<Mailbox>, body: B, handle: &Handle)
            -> Box<Future<Item = (), Error = IoError>> {
        //self.send_raw(return_path, recipients, body.into_mail_body(handle), handle)
        sendmail(TokioTcpClient::new(ClientProto(self.0.params.clone()))
                 .connect(&self.0.addrs[0], handle),
                 return_path, recipients, body, handle)
    }
}


/// Builder for a `Mailer` instance.
pub struct MailerBuilder {
    server: String,
    client_id: ClientId,
    client_auth: Option<ClientAuth>,
    tls_connector: Option<TlsConnector>,
}

impl MailerBuilder {
    /// Create a builder.
    pub fn new(server: String) -> Self {
        MailerBuilder {
            server,
            client_id: ClientId::Domain("localhost".to_string()),
            client_auth: None,
            tls_connector: None,
        }
    }

    /// Create a builder setup for connecting to `localhost:25` with no TLS.
    pub fn local() -> MailerBuilder {
        Self::new("localhost:25".to_string())
    }

    /// Set the `EHLO` identifier to send.
    ///
    /// By default, this is `localhost`.
    pub fn set_client_id(mut self, client_id: ClientId) -> Self {
        self.client_id = client_id;
        self
    }

    /// Set the client authentication parameters
    ///
    /// By default, no authentication data is set.
    /// Many smtp services requires a valid user authentication to be able to send mail.
    pub fn set_client_auth(mut self, client_auth: ClientAuth) -> Self {
        self.client_auth = Some(client_auth);
        self
    }

    /// Enable TLS using the `STARTTLS` command, and use the given connector.
    ///
    /// By default, connections do not use TLS.
    pub fn set_tls_connector(mut self, tls_connector: TlsConnector) -> Self {
        self.tls_connector = Some(tls_connector);
        self
    }

    /// Enable TLS using the `STARTTLS` command, and use default connector (native).
    ///
    /// By default, connections do not use TLS.
    pub fn use_default_tls_connector(self) -> TlsResult<Self> {
        let connector = TlsConnector::builder()
            .and_then(|builder| builder.build())?;
        Ok(self.set_tls_connector(connector))
    }

    /// Transform this builder into a `Mailer`.
    pub fn build(self) -> IoResult<Mailer> {
        let addrs = self.server.to_socket_addrs()?.collect();
        Ok(Mailer(Arc::new(MailerParams {
            addrs,
            params: Arc::new(ClientParams {
                id: self.client_id,
                auth: self.client_auth,
                security: match self.tls_connector {
                    None => ClientSecurity::None,
                    Some(connector) => ClientSecurity::Required(ClientTlsParams {
                        connector,
                        sni_domain: self.server.rsplitn(2, ':')
                            .nth(1).unwrap().to_string(),
                    }),
                },
            }),
        })))
    }
}
