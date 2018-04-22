use base64;
use request::{Request};
use super::{ClientParams, ClientTransport};
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use futures::{future, Future, Stream, Sink};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_proto::streaming::pipeline::{Frame};

/// Client authentication options
pub struct ClientAuth {
    /// Client username or login
    pub username: String,
    /// Client password
    pub password: String,
}

impl ClientAuth {
    /// Instantiate client authentication parameters
    pub fn new<S>(username: S, password: S) -> Self
    where S: Into<String>
    {
        ClientAuth {
            username: username.into(),
            password: password.into(),
        }
    }
}

// TODO: Support more authentication mechanisms.
pub fn clientauth<T>(stream: ClientTransport<T>, params: &ClientParams, features: &[String]) ->
    Box<Future<Item = ClientTransport<T>, Error = IoError>>
where T: AsyncRead + AsyncWrite + 'static
{
    if params.auth.is_none() {
        return Box::new(future::ok(stream))
    }
    
    if let Some(ref auth_methods) = features.iter()
        .find(|feature| feature.starts_with("AUTH "))
        .map(|feature| feature.split_at(5).1.split(' '))
    {
        if auth_methods.clone().any(|method| method == "PLAIN") {
            let authdata = if let Some(ClientAuth { ref username, ref password }) = params.auth {
                base64::encode(&format!("{}\0{}\0{}", username, username, password))
            } else { unreachable!(); };

            // Send AUTH PLAIN request.
            Box::new(stream.send(Request::Auth {
                method: Some("PLAIN".into()),
                data: Some(authdata),
            }.into())
                     // Await auth response.
                     .and_then(|stream| stream.into_future().map_err(|(err, _)| err))
                     .and_then(|(response, stream)| {
                         let response = match response {
                             Some(Frame::Message { message, .. }) => message,
                             _ => return future::err(IoError::new(
                                 IoErrorKind::InvalidData, "connection closed during auth")),
                         };
                         
                         // Check auth status.
                         if !response.code.severity.is_positive() {
                             return future::err(IoError::new(
                                 IoErrorKind::InvalidData, "authentication failed"));
                         }
                         
                         future::ok(stream)
                     }))
        } else if auth_methods.clone().any(|method| method == "LOGIN") {
            let (username, password) = if let Some(ref authdata) = params.auth {
                (base64::encode(&authdata.username), base64::encode(&authdata.password))
            } else { unreachable!(); };
            
            // Send AUTH LOGIN request.
            Box::new(stream.send(Request::Auth {
                method: Some("LOGIN".into()),
                data: Some(username),
            }.into())
                     // Send password.
                     .and_then(|stream| stream.send(Request::Auth {
                         method: None,
                         data: Some(password)
                     }.into()))
                     // Await auth response.
                     .and_then(|stream| stream.into_future().map_err(|(err, _)| err))
                     .and_then(|(response, stream)| {
                         let response = match response {
                             Some(Frame::Message { message, .. }) => message,
                             _ => return future::err(IoError::new(
                                 IoErrorKind::InvalidData, "connection closed during auth")),
                         };
                         
                         // Check auth status.
                         if !response.code.severity.is_positive() {
                             return future::err(IoError::new(
                                 IoErrorKind::InvalidData, "authentication failed"));
                         }
                         
                         future::ok(stream)
                     }))
        } else {
            Box::new(future::err(IoError::new(
                IoErrorKind::InvalidData, "no supported auth methods found")))
        }
    } else {
        Box::new(future::err(IoError::new(
            IoErrorKind::InvalidData, "server does not support auth")))
    }
}
