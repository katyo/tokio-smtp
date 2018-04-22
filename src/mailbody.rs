use std::io::{Error as IoError};
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
    fn into_mail_body(self) -> MailBody;
}

impl IntoMailBody for MailBody {
    fn into_mail_body(self) -> MailBody {
        self
    }
}

impl IntoMailBody for Vec<u8> {
    fn into_mail_body(self) -> MailBody {
        self.into()
    }
}

impl IntoMailBody for String {
    fn into_mail_body(self) -> MailBody {
        self.into_bytes().into()
    }
}
