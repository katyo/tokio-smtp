use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::sync::{Arc};
use futures::{future, Future, Stream, Sink};
use client::{ClientParams, ClientSecurity, ClientIo, ClientTransport, ClientBindTransport};
use client::handshake::{handshake};
use request::{Request};
use response::{Response};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_proto::streaming::pipeline::{ClientProto as TokioClientProto, Frame};
use tokio_tls::{TlsConnectorExt};

/// The Tokio client protocol implementation
///
/// Implements an SMTP client using a streaming pipeline protocol.
pub struct ClientProto(pub Arc<ClientParams>);


impl ClientProto {
    pub fn connect<T>(io: T, params: Arc<ClientParams>) -> ClientBindTransport<T>
    where T: AsyncRead + AsyncWrite + 'static
    {
        match params.security {
            ClientSecurity::None => {
                Self::connect_plain(io, params)
            },
            ClientSecurity::Optional(_) | ClientSecurity::Required(_) => {
                Self::connect_starttls(io, params)
            },
            ClientSecurity::Immediate(_) => {
                Self::connect_immediate_tls(io, params)
            },
        }
    }

    fn connect_plain<T>(io: T, params: Arc<ClientParams>) -> ClientBindTransport<T>
    where T: AsyncRead + AsyncWrite + 'static
    {
        // Perform the handshake.
        Box::new(handshake(ClientIo::Plain(io), params, true, true)
                 .map(|(_, stream)| stream))
    }

    fn connect_starttls<T>(io: T, params: Arc<ClientParams>) -> ClientBindTransport<T>
    where T: AsyncRead + AsyncWrite + 'static
    {
        let is_required =
            if let ClientSecurity::Required(_) = params.security { true } else { false };
        // Perform the handshake, and send STARTTLS.
        Box::new(handshake(ClientIo::Plain(io), params.clone(), true, false)
                 .and_then(move |(ehlo_response, stream)| {
                     let is_supported = None != ehlo_response.text.iter()
                         .find(|feature| feature.as_str() == "STARTTLS");
                     
                     if !is_supported {
                         if is_required {
                             return future::Either::B(future::Either::B(future::err(IoError::new(
                                 IoErrorKind::InvalidData, "server doesn't support starttls"))));
                         }
                         
                         return future::Either::B(future::Either::A(future::ok(stream)));
                     }
                     
                     future::Either::A(stream.send(Request::StartTls.into())
                     // Receive STARTTLS response.
                         .and_then(|stream| {
                             stream.into_future()
                                 .map_err(|(err, _)| err)
                         })
                         .and_then(move |(response, stream)| {
                             // Fail if closed.
                             let response = match response {
                                 Some(Frame::Message { message, .. }) => message,
                                 None => return future::err(IoError::new(
                                     IoErrorKind::InvalidData, "connection closed before starttls")),
                                 _ => unreachable!(),
                             };
                             
                             // Handle rejection.
                             if !response.code.severity.is_positive() && is_required {
                                 return future::err(IoError::new(
                                     IoErrorKind::InvalidData, "starttls rejected"));
                             }
                             
                             future::ok(stream)
                         })
                         .and_then(move |stream| {
                             // Get the inner `Io` back, then start TLS on it.
                             // The block is to ensure the lifetime of `params.
                             {
                                 let io = stream.into_inner().unwrap_plain();
                                 let tls_params = match params.security {
                                     ClientSecurity::Optional(ref tls_params) |
                                     ClientSecurity::Required(ref tls_params) => tls_params,
                                     _ => panic!("bad params to connect_starttls"),
                                 };
                                 tls_params.connector.connect_async(&tls_params.sni_domain, io)
                                     .map_err(|err| IoError::new(IoErrorKind::Other, err))
                             }
                             .and_then(move |io| {
                                 // Re-do the handshake.
                                 handshake(ClientIo::Secure(io), params, false, true)
                                     .map(|(_, stream)| stream)
                             })
                         }))
                 }))
    }

    fn connect_immediate_tls<T>(io: T, params: Arc<ClientParams>) -> ClientBindTransport<T>
    where T: AsyncRead + AsyncWrite + 'static
    {
        // Start TLS on the `Io` first.
        // The block is to ensure the lifetime of `params.
        Box::new({
            let tls_params = match params.security {
                ClientSecurity::Immediate(ref tls_params) => tls_params,
                _ => panic!("bad params to connect_immediate_tls"),
            };
            tls_params.connector.connect_async(&tls_params.sni_domain, io)
                .map_err(|err| IoError::new(IoErrorKind::Other, err))
        }
            .and_then(move |io| {
                // Perform the handshake.
                handshake(ClientIo::Secure(io), params, true, true)
                    .map(|(_, stream)| stream)
            }))
    }
}

impl<T> TokioClientProto<T> for ClientProto
where T: AsyncRead + AsyncWrite + 'static
{
    type Request = Request;
    type RequestBody = Vec<u8>;
    type Response = Response;
    type ResponseBody = ();
    type Error = IoError;
    type Transport = ClientTransport<T>;
    type BindTransport = ClientBindTransport<T>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Self::connect(io, self.0.clone())
    }
}
