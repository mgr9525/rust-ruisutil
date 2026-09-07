use std::{future::Future, pin::Pin, sync::atomic::AtomicUsize, time::Duration};

use crate::asyncs::tkocncel;

pub struct BoxStream<IO> {
    inner: Box<BoxTcpStreamInr<IO>>,
}

struct BoxTcpStreamInr<IO> {
    ctx: crate::asyncs::Context,
    ctxfut: Pin<Box<crate::asyncs::ContextFuture>>,
    tmr: crate::Timer,
    tmslpr: Pin<Box<tokio::time::Sleep>>,
    tmslpw: Pin<Box<tokio::time::Sleep>>,
    stream: Pin<Box<IO>>,
    // ln_rd: AtomicUsize,
    // ln_wd: AtomicUsize,
}
impl<IO> Drop for BoxTcpStreamInr<IO> {
    fn drop(&mut self) {
        self.ctx.cancel();
    }
}
const SLEEP_DURATION: Duration = Duration::from_secs(30);
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
        let ctx = ctx
            .map(|v| v.child())
            .unwrap_or(crate::asyncs::Context::new());
        let ctxfut = Box::pin(ctx.future());
        let tmr = crate::Timer::new(outdur);
        tmr.reset();
        Self {
            inner: Box::new(BoxTcpStreamInr {
                ctx: ctx,
                ctxfut: ctxfut,
                tmr: tmr,
                tmslpr: Box::pin(tokio::time::sleep(SLEEP_DURATION)),
                tmslpw: Box::pin(tokio::time::sleep(SLEEP_DURATION)),
                stream: Box::pin(stream),
                // ln_rd: AtomicUsize::new(0),
                // ln_wd: AtomicUsize::new(0),
            }),
        }
    }

    fn reset_tmslp(tmslp: Pin<&mut tokio::time::Sleep>) {
        let inst = tokio::time::Instant::now() + SLEEP_DURATION;
        tmslp.reset(inst);
    }
}

impl<IO> BoxStream<IO>
where
    IO: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    pub fn poll_reads(
        &mut self,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if self.inner.tmr.tmout() {
            self.inner.ctx.cancel();
        }
        if self.inner.ctx.cancelled() {
            return std::task::Poll::Ready(Err(crate::ioerr(
                "ctx end",
                Some(std::io::ErrorKind::BrokenPipe),
            )));
        }
        let rst = self.inner.stream.as_mut().poll_read(cx, buf);
        match &rst {
            std::task::Poll::Ready(Ok(_v)) => {
                if buf.filled().len() > 0 {
                    self.inner.tmr.reset();
                    // this.inner
                    //     .ln_rd
                    //     .fetch_add(buf.filled().len(), std::sync::atomic::Ordering::Relaxed);
                    Self::reset_tmslp(self.inner.tmslpr.as_mut());
                }
            }
            std::task::Poll::Pending => {
                if let std::task::Poll::Ready(v) = self.inner.ctxfut.as_mut().poll(cx) {
                    if !v.is_ok() {
                        return std::task::Poll::Ready(Err(crate::ioerr(
                            "ctx end, poll ctxfut err",
                            Some(std::io::ErrorKind::BrokenPipe),
                        )));
                    }
                }
                let _v = std::task::ready!(self.inner.tmslpr.as_mut().poll(cx));
                Self::reset_tmslp(self.inner.tmslpr.as_mut());
            }
            _ => {}
        }
        rst
    }
    
    pub fn poll_writes(
        &mut self,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        if self.inner.tmr.tmout() {
            self.inner.ctx.cancel();
        }
        if self.inner.ctx.cancelled() {
            return std::task::Poll::Ready(Err(crate::ioerr(
                "ctx end",
                Some(std::io::ErrorKind::BrokenPipe),
            )));
        }
        let rst = self.inner.stream.as_mut().poll_write(cx, buf);
        match &rst {
            std::task::Poll::Ready(Ok(n)) => {
                let ln = *n;
                if ln > 0 {
                    self.inner.tmr.reset();
                    // this.inner
                    //     .ln_wd
                    //     .fetch_add(ln, std::sync::atomic::Ordering::Relaxed);
                    Self::reset_tmslp(self.inner.tmslpw.as_mut());
                }
            }
            std::task::Poll::Pending => {
                if let std::task::Poll::Ready(v) = self.inner.ctxfut.as_mut().poll(cx) {
                    if !v.is_ok() {
                        return std::task::Poll::Ready(Err(crate::ioerr(
                            "ctx end, poll ctxfut err",
                            Some(std::io::ErrorKind::BrokenPipe),
                        )));
                    }
                }
                let _v = std::task::ready!(self.inner.tmslpw.as_mut().poll(cx));
                Self::reset_tmslp(self.inner.tmslpw.as_mut());
            }
            _ => {}
        }
        rst
    }

    pub fn poll_flushs(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        /* if self.ctx.cancelled() {
            return std::task::Poll::Ready(Err(crate::ioerr(
                "ctx end",
                Some(std::io::ErrorKind::BrokenPipe),
            )));
        } */
        self.inner.stream.as_mut().poll_flush(cx)
    }

    pub fn poll_shutdowns(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.inner.stream.as_mut().poll_shutdown(cx)
    }
}

impl<IO> tokio::io::AsyncRead for BoxStream<IO>
where
    IO: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.get_mut().poll_reads(cx, buf)
    }
}
impl<IO> tokio::io::AsyncWrite for BoxStream<IO>
where
    IO: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        self.get_mut().poll_writes(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.get_mut().poll_flushs(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.get_mut().poll_shutdowns(cx)
    }
}
