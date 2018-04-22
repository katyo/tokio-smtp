use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::sync::{Arc};
use futures::{future, Future, Stream, Sink};
use client::{ClientIo, ClientCodec, ClientParams, ClientTransport};
use client::auth::{clientauth};
use request::{Request};
use response::{Response};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_proto::streaming::pipeline::{Frame};

type HandshakeItem<T> = (Response, ClientTransport<T>);

// FIXME: Return opening response.
pub fn handshake<T>(io: ClientIo<T>, params: Arc<ClientParams>, await_opening: bool, do_auth: bool) ->
    Box<Future<Item = HandshakeItem<T>, Error = IoError>>
where T: AsyncRead + AsyncWrite + 'static
{
    Box::new(
        // Start codec.
        io.framed(ClientCodec::new())
        // Send EHLO.
            .send(Request::Ehlo(params.id.clone()).into())
            .and_then(move |stream| {
                // Receive server opening.
                if await_opening {
                    future::Either::A(stream.into_future()
                        .map_err(|(err, _)| err)
                        .and_then(|(response, stream)| {
                            // Fail if closed.
                            let response = match response {
                                Some(Frame::Message { message, .. }) => message,
                                _ => return future::err(IoError::new(
                                    IoErrorKind::InvalidData, "connection closed before handshake")),
                            };
                            
                            // Ensure it likes us, and supports ESMTP.
                            let esmtp = response.text.get(0)
                                .and_then(|line| line.split_whitespace().nth(1));
                            if !response.code.severity.is_positive() || esmtp != Some("ESMTP") {
                                return future::err(IoError::new(
                                    IoErrorKind::InvalidData, "invalid handshake"));
                            }
                            
                            future::ok(stream)
                        }))
                } else {
                    future::Either::B(future::ok(stream))
                }
            })
        // Receive EHLO response.
            .and_then(move |stream| {
                stream.into_future()
                    .map_err(|(err, _)| err)
                    .and_then(move |(response, stream)| {
                        // Fail if closed.
                        let response = match response {
                            Some(Frame::Message { message, .. }) => message,
                            _ => return future::Either::B(future::err(IoError::new(
                                IoErrorKind::InvalidData, "connection closed during handshake"))),
                        };

                        if do_auth {
                            return future::Either::A(future::Either::A(
                                clientauth(stream, &params, &response.text)
                                    .and_then(|stream| {
                                        future::ok((response, stream))
                                    })))
                        }
                        
                        future::Either::A(future::Either::B(
                            future::ok((response, stream))))
                    })
            })
    )
}
