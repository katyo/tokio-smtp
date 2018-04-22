use std::io::{Error as IoError, Result as IoResult, Read, Write};
use futures::{Poll};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_tls::{TlsStream};

/// An `Io` implementation that wraps a secure or insecure transport into a
/// single type.
pub enum ClientIo<T> {
    /// Insecure transport
    Plain(T),
    /// Secure transport
    Secure(TlsStream<T>),
}

impl<T> ClientIo<T> {
    pub fn unwrap_plain(self) -> T {
        if let ClientIo::Plain(io) = self {
            io
        } else {
            panic!("called unwrap_plain on non-plain stream")
        }
    }
}

impl<T> Read for ClientIo<T>
where T: AsyncRead + 'static, TlsStream<T>: Read
{
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        match *self {
            ClientIo::Plain(ref mut stream) => stream.read(buf),
            ClientIo::Secure(ref mut stream) => stream.read(buf),
        }
    }
}

impl<T> Write for ClientIo<T>
where T: AsyncWrite + 'static, TlsStream<T>: Write
{
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        match *self {
            ClientIo::Plain(ref mut stream) => stream.write(buf),
            ClientIo::Secure(ref mut stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        match *self {
            ClientIo::Plain(ref mut stream) => stream.flush(),
            ClientIo::Secure(ref mut stream) => stream.flush(),
        }
    }
}

impl<T> AsyncRead for ClientIo<T>
where T: AsyncRead + 'static, TlsStream<T>: AsyncRead + Read
{}

impl<T> AsyncWrite for ClientIo<T>
where T: AsyncWrite + 'static, TlsStream<T>: AsyncWrite + Write
{
    fn shutdown(&mut self) -> Poll<(), IoError> {
        match *self {
            ClientIo::Plain(ref mut t) => t.shutdown(),
            ClientIo::Secure(ref mut t) => t.shutdown(),
        }
    }
}
