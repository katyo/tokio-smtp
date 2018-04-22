use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use mailbody::{MailBody, IntoMailBody};
use request::{Mailbox, Request as SmtpRequest};
use response::{Response as SmtpResponse};
use futures::{future, Future};
use tokio_core::reactor::{Handle};
use tokio_proto::streaming::{Message, Body};
use tokio_service::{Service};

pub type SmtpRequestBody = MailBody;
pub type SmtpResponseBody = Body<(), IoError>;
pub type SmtpRequestMessage = Message<SmtpRequest, SmtpRequestBody>;
pub type SmtpResponseMessage = Message<SmtpResponse, SmtpResponseBody>;

/// Send an email.
pub fn sendmail<B, C, S>(
    client: C,
    return_path: Mailbox,
    recipients: Vec<Mailbox>,
    body: B,
    handle: &Handle
) -> Box<Future<Item = (), Error = IoError>>
where B: IntoMailBody,
      C: Future<Item = S, Error = IoError> + 'static,
      S: Service<Request = SmtpRequestMessage, Response = SmtpResponseMessage, Error = IoError>,
      S::Future: 'static,
{
    let body = body.into_mail_body(handle);
    
    // FIXME: Iterate addrs.
    Box::new(
        client.and_then(move |service| {
            let mut reqs = Vec::with_capacity(4);
            reqs.push(service.call(
                Message::WithoutBody(SmtpRequest::Mail {
                    from: return_path,
                    params: vec![],
                })
            ));
            for recipient in recipients {
                reqs.push(service.call(
                    Message::WithoutBody(SmtpRequest::Rcpt {
                        to: recipient,
                        params: vec![],
                    })
                ));
            }
            reqs.push(service.call(
                Message::WithBody(SmtpRequest::Data, body)
            ));
            reqs.push(service.call(
                Message::WithoutBody(SmtpRequest::Quit)
            ));
            future::join_all(reqs)
        })
            .and_then(|responses| {
                for response in responses {
                    let response = response.into_inner();
                    if !response.code.severity.is_positive() {
                        return future::err(IoError::new(IoErrorKind::Other,
                                                        format!("bad smtp response {}", response.code)))
                    }
                }
                future::ok(())
            })
    )
}
