use std::io::{Error as IoError, ErrorKind as IoErrorKind, Result as IoResult};
use nom::{IResult as NomResult};
use request::{Request};
use response::{Response, Severity};
use bytes::{BufMut, BytesMut};
use tokio_io::codec::{Encoder, Decoder};
use tokio_proto::streaming::pipeline::{Frame};

/// The codec used to encode client requests and decode server responses
#[derive(Default)]
pub struct ClientCodec {
    escape_count: u8,
}

impl ClientCodec {
    pub fn new() -> Self {
        ClientCodec::default()
    }
}

impl Encoder for ClientCodec {
    type Item = Frame<Request, Vec<u8>, IoError>;
    type Error = IoError;

    fn encode(&mut self, frame: Self::Item, buf: &mut BytesMut) -> IoResult<()> {
        debug!("C: {:?}", &frame);
        match frame {
            Frame::Message { message, .. } => {
                buf.put_slice(message.to_string().as_bytes());
            },
            Frame::Body { chunk: Some(chunk) } => {
                // Escape lines starting with a '.'
                // FIXME: additional encoding for non-ASCII?
                let mut start = 0;
                for (idx, byte) in chunk.iter().enumerate() {
                    match self.escape_count {
                        0 => self.escape_count = if *byte == b'\r' { 1 } else { 0 },
                        1 => self.escape_count = if *byte == b'\n' { 2 } else { 0 },
                        2 => self.escape_count = if *byte == b'.'  { 3 } else { 0 },
                        _ => unreachable!(),
                    }
                    if self.escape_count == 3 {
                        self.escape_count = 0;
                        buf.put_slice(&chunk[start..idx]);
                        buf.put_slice(b".");
                        start = idx;
                    }
                }
                buf.put_slice(&chunk[start..]);
            },
            Frame::Body { chunk: None } => {
                match self.escape_count {
                    0 => buf.put_slice(b"\r\n.\r\n"),
                    1 => buf.put_slice(b"\n.\r\n"),
                    2 => buf.put_slice(b".\r\n"),
                    _ => unreachable!(),
                }
                self.escape_count = 0;
            },
            Frame::Error { error } => {
                panic!("unimplemented error handling: {:?}", error);
            },
        }
        Ok(())
    }
}

impl Decoder for ClientCodec {
    type Item = Frame<Response, (), IoError>;
    type Error = IoError;
    
    fn decode(&mut self, buf: &mut BytesMut) -> IoResult<Option<Self::Item>> {
        let mut bytes: usize = 0;

        let res = match Response::parse(buf.as_ref()) {
            NomResult::Done(rest, res) => {
                // Calculate how much data to drain.
                bytes = buf.len() - rest.len();

                // Drop intermediate messages (e.g. DATA 354)
                if res.code.severity == Severity::PositiveIntermediate {
                    Ok(None)
                } else {
                    let frame = Frame::Message { message: res, body: false };
                    debug!("S: {:?}", &frame);
                    Ok(Some(frame))
                }
            },
            NomResult::Incomplete(_) => {
                Ok(None)
            },
            NomResult::Error(_) => {
                Err(IoError::new(IoErrorKind::InvalidData, "malformed response"))
            },
        };

        // Drain parsed data.
        if bytes != 0 {
            buf.split_to(bytes);

            // If we dropped the message, try to parse the remaining data.
            if let Ok(None) = res {
                return self.decode(buf);
            }
        }

        res
    }
}
