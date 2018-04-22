use std::io::{Error as IoError};
use futures::{future, Future, Sink};
use tokio_core::reactor::{Handle};
use tokio_proto::streaming::{Body};

pub type MailBody = Body<Vec<u8>, IoError>;

/// A trait for objects that can be converted to a `MailBody`.
///
/// When sending mail using `Mailer::send`, any object that implements this
/// trait can be passed as the body.
pub trait IntoMailBody {
    /// Converts this object to a `MailBody`.
    ///
    /// The handle can optionally be used to write the body.
    fn into_mail_body(self, &Handle) -> MailBody;
}

impl IntoMailBody for MailBody {
    fn into_mail_body(self, _: &Handle) -> MailBody {
        self
    }
}

impl IntoMailBody for Vec<u8> {
    fn into_mail_body(self, handle: &Handle) -> MailBody {
        let (sender, body) = MailBody::pair();
        handle.spawn(
            sender.send(Ok(self))
                .and_then(|_| future::ok(()))
                .or_else(|_| future::ok(()))
        );
        body
    }
}

impl IntoMailBody for String {
    fn into_mail_body(self, handle: &Handle) -> MailBody {
        self.into_bytes().into_mail_body(handle)
    }
}
