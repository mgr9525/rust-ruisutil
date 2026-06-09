use std::{sync::atomic::AtomicUsize, time::Duration};

#[derive(Default)]
pub struct H2StreamsNums {
    pub max_send_streams: AtomicUsize,
    pub num_send_streams: AtomicUsize,
    pub max_recv_streams: AtomicUsize,
    pub num_recv_streams: AtomicUsize,
}

pub struct BoxStream<IO> {
    inner: Box<BoxTcpStreamInr<IO>>,
}
struct BoxTcpStreamInr<IO> {
    ctx: crate::asyncs::Context,
    tmr: crate::Timer,
    stream: IO,
}
impl<IO> Drop for BoxTcpStreamInr<IO> {
    fn drop(&mut self) {
        self.ctx.cancel();
    }
}
impl<IO> BoxStream<IO> {
    pub fn new(ctx: &crate::asyncs::Context, stream: IO) -> Self {
        Self::newctx(Some(ctx), stream)
    }
    pub fn new_tmout(ctx: &crate::asyncs::Context, stream: IO, outdur: Duration) -> Self {
        Self::newctx_tmout(Some(ctx), stream, outdur)
    }
    pub fn newctx(ctx: Option<&crate::asyncs::Context>, stream: IO) -> Self {
        Self::newctx_tmout(ctx, stream, Duration::from_secs(60 * 2))
    }
    pub fn newctx_tmout(
        ctx: Option<&crate::asyncs::Context>,
        stream: IO,
        outdur: Duration,
    ) -> Self {
        Self {
            inner: Box::new(BoxTcpStreamInr {
                ctx: ctx
                    .map(|v| v.child())
                    .unwrap_or(crate::asyncs::Context::new()),
                tmr: crate::Timer::new(outdur),
                stream: stream,
            }),
        }
    }

    pub fn start(&self) {
        let ctx = self.inner.ctx.clone();
        let tmr = self.inner.tmr.clone();
        tmr.reset();
        crate::asyncs::task::spawn(async move {
            while !ctx.cancelled() {
                tokio::time::sleep(Duration::from_secs(5)).await;
                if tmr.tmout() {
                    ctx.cancel();
                    break;
                }
            }
        });
    }
}

impl<IO> tokio::io::AsyncRead for BoxStream<IO>
where
    IO: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if self.inner.ctx.cancelled() {
            return std::task::Poll::Ready(Err(crate::ioerr(
                "ctx end",
                Some(std::io::ErrorKind::BrokenPipe),
            )));
        }
        let rst = std::pin::Pin::new(&mut self.inner.stream).poll_read(cx, buf);

        match &rst {
            std::task::Poll::Ready(Ok(_v)) => {
                if buf.filled().len() > 0 {
                    self.inner.tmr.reset();
                }
            }
            _ => {}
        }
        rst
    }
}
impl<IO> tokio::io::AsyncWrite for BoxStream<IO>
where
    IO: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        if self.inner.ctx.cancelled() {
            return std::task::Poll::Ready(Err(crate::ioerr(
                "ctx end",
                Some(std::io::ErrorKind::BrokenPipe),
            )));
        }
        let rst = std::pin::Pin::new(&mut self.inner.stream).poll_write(cx, buf);

        match &rst {
            std::task::Poll::Ready(Ok(n)) => {
                if *n > 0 {
                    self.inner.tmr.reset();
                }
            }
            _ => {}
        }
        rst
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        /* if self.ctx.cancelled() {
            return std::task::Poll::Ready(Err(crate::ioerr(
                "ctx end",
                Some(std::io::ErrorKind::BrokenPipe),
            )));
        } */
        std::pin::Pin::new(&mut self.inner.stream).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner.stream).poll_shutdown(cx)
    }
}
