//! SMTP request, containing one of several commands, and arguments

// FIXME: Add parsing.

use emailaddress::{EmailAddress, AddrError};
use std::io::{Error as IoError};
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::{FromStr};
use tokio_proto::streaming::pipeline::{Frame};


/// Client identifier, as sent in `EHLO`.
#[derive(PartialEq,Eq,Clone,Debug)]
pub enum ClientId {
    Domain(String),
    Ipv4(Ipv4Addr),
    Ipv6(Ipv6Addr),
    Other { tag: String, value: String },
}

impl Display for ClientId {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match *self {
            ClientId::Domain(ref value) => f.write_str(value),
            ClientId::Ipv4(ref value) => write!(f, "{}", value),
            ClientId::Ipv6(ref value) => write!(f, "IPv6:{}", value),
            ClientId::Other { ref tag, ref value } => write!(f, "{}:{}", tag, value),
        }
    }
}


/// A mailbox specified in `MAIL FROM` or `RCPT TO`.
#[derive(PartialEq,Clone,Debug)]
pub struct Mailbox(pub Option<EmailAddress>);

impl From<EmailAddress> for Mailbox {
    fn from(addr: EmailAddress) -> Self {
        Mailbox(Some(addr))
    }
}

impl FromStr for Mailbox {
    type Err = AddrError;

    fn from_str(string: &str) -> Result<Mailbox, AddrError> {
        if string.is_empty() {
            Ok(Mailbox(None))
        } else {
            Ok(EmailAddress::new(string)?.into())
        }
    }
}

impl Display for Mailbox {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        // FIXME: xtext?
        match self.0 {
            Some(ref email) => write!(f, "<{}>", email),
            None => f.write_str("<>"),
        }
    }
}


/// A `MAIL FROM` extension parameter.
#[derive(PartialEq,Eq,Clone,Debug)]
pub enum MailParam {
    EightBitMime,
    Size(usize),
    Other { keyword: String, value: Option<String> },
}

impl Display for MailParam {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match *self {
            MailParam::EightBitMime => f.write_str("8BITMIME"),
            MailParam::Size(size) => write!(f, "SIZE={}", size),
            MailParam::Other { ref keyword, value: Some(ref value) } => {
                write!(f, "{}={}", keyword, value)
            },
            MailParam::Other { ref keyword, value: None } => {
                f.write_str(keyword)
            },
        }
    }
}


/// A `RCPT TO` extension parameter.
#[derive(PartialEq,Eq,Clone,Debug)]
pub enum RcptParam {
    Other { keyword: String, value: Option<String> },
}

impl Display for RcptParam {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match *self {
            RcptParam::Other { ref keyword, value: Some(ref value) } => {
                write!(f, "{}={}", keyword, value)
            },
            RcptParam::Other { ref keyword, value: None } => {
                f.write_str(keyword)
            },
        }
    }
}


/// A complete SMTP request.
#[derive(PartialEq,Clone,Debug)]
pub enum Request {
    Ehlo(ClientId),
    StartTls,
    Mail { from: Mailbox, params: Vec<MailParam> },
    Rcpt { to: Mailbox, params: Vec<RcptParam> },
    Data,
    Quit,
}

impl Display for Request {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match *self {
            Request::Ehlo(ref id) => write!(f, "EHLO {}\r\n", id),
            Request::StartTls => write!(f, "STARTTLS\r\n"),
            Request::Mail { ref from, ref params } => {
                write!(f, "MAIL FROM:{}", from)?;
                for param in params {
                    write!(f, " {}", param)?;
                }
                f.write_str("\r\n")
            },
            Request::Rcpt { ref to, ref params } => {
                write!(f, "RCPT TO:{}", to)?;
                for param in params {
                    write!(f, " {}", param)?;
                }
                f.write_str("\r\n")
            },
            Request::Data => {
                f.write_str("DATA\r\n")
            },
            Request::Quit => {
                f.write_str("QUIT\r\n")
            },
        }
    }
}

impl From<Request> for Frame<Request, Vec<u8>, IoError> {
    fn from(request: Request) -> Self {
        let has_body = request == Request::Data;
        Frame::Message {
            message: request,
            body: has_body,
        }
    }
}


#[cfg(test)]
mod tests {
    use ::{ClientId, MailParam, RcptParam, Request};

    #[test]
    fn test() {
        for (input, expect) in vec![
            (
                Request::Ehlo(
                    ClientId::Domain("foobar.example".to_string())
                ),
                "EHLO foobar.example\r\n",
            ),
            (
                Request::Ehlo(
                    ClientId::Ipv4("127.0.0.1".parse().unwrap())
                ),
                "EHLO 127.0.0.1\r\n",
            ),
            (
                Request::StartTls,
                "STARTTLS\r\n",
            ),
            (
                Request::Mail {
                    from: "".parse().unwrap(),
                    params: vec![],
                },
                "MAIL FROM:<>\r\n",
            ),
            (
                Request::Mail {
                    from: "".parse().unwrap(),
                    params: vec![
                        MailParam::Size(1024),
                    ],
                },
                "MAIL FROM:<> SIZE=1024\r\n",
            ),
            (
                Request::Mail {
                    from: "john@example.test".parse().unwrap(),
                    params: vec![],
                },
                "MAIL FROM:<john@example.test>\r\n",
            ),
            (
                Request::Rcpt {
                    to: "".parse().unwrap(),
                    params: vec![],
                },
                "RCPT TO:<>\r\n",
            ),
            (
                Request::Rcpt {
                    to: "".parse().unwrap(),
                    params: vec![
                        RcptParam::Other { keyword: "FOOBAR".to_string(), value: None },
                    ],
                },
                "RCPT TO:<> FOOBAR\r\n",
            ),
            (
                Request::Rcpt {
                    to: "alice@example.test".parse().unwrap(),
                    params: vec![],
                },
                "RCPT TO:<alice@example.test>\r\n",
            ),
            (
                Request::Data,
                "DATA\r\n",
            ),
            (
                Request::Quit,
                "QUIT\r\n",
            ),
        ] {
            assert_eq!(input.to_string(), expect);
        }
    }
}