use std::{
    io::Result,
    pin::Pin,
    task::{Context, Poll},
};

use futures_io::{AsyncRead, AsyncWrite};
use tracing::{debug, instrument};

use crate::StartTlsExt;

use super::ImapStartTls;

impl<S: AsyncRead + AsyncWrite + Unpin> StartTlsExt<S, true> for ImapStartTls<'_, S, true> {
    type Context<'a> = Context<'a>;
    type Output<T> = Poll<Result<T>>;

    #[instrument(skip_all)]
    fn poll(&mut self, cx: &mut Context<'_>) -> Self::Output<()> {
        if !self.handshake_discarded {
            match Pin::new(&mut self.stream).poll_read(cx, &mut self.buf)? {
                Poll::Ready(n) => {
                    let plain = String::from_utf8_lossy(&self.buf[..n]);
                    debug!("read then discarded {n} bytes: {plain:?}");
                    self.buf.fill(0);
                    self.handshake_discarded = true;
                }
                Poll::Pending => {
                    debug!("reading still ongoing");
                    return Poll::Pending;
                }
            }
        }

        if !self.command_sent {
            match Pin::new(&mut self.stream).poll_write(cx, Self::COMMAND.as_bytes())? {
                Poll::Ready(n) => {
                    debug!("wrote {n} bytes: {:?}", Self::COMMAND);
                    self.command_sent = true;
                }
                Poll::Pending => {
                    debug!("writing still ongoing");
                }
            }
        }

        match Pin::new(&mut self.stream).poll_read(cx, &mut self.buf)? {
            Poll::Ready(n) => {
                let plain = String::from_utf8_lossy(&self.buf[..n]);
                debug!("read then discarded {n} bytes: {plain:?}");
                self.buf.fill(0);
            }
            Poll::Pending => {
                debug!("reading still ongoing");
                return Poll::Pending;
            }
        }

        Pin::new(&mut self.stream).poll_flush(cx)
    }
}