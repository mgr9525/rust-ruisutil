use std::{
    future::Future,
    io,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use crate::{asyncs, bytes::BytesCut, sync::WakerFut};
use asyncs::sync::RwLock;

use super::ByteBoxBuf;

pub struct ByteSteamBuf {
    ctx: crate::asyncs::Context,
    buf: RwLock<ByteBoxBuf>,
    max: AtomicUsize,
    tmout: Duration,
    wkr_can_read: WakerFut,
    wkr_can_write: WakerFut,

    wk_can_read: Option<std::task::Waker>,
    wk_can_write: Option<std::task::Waker>,
}

impl ByteSteamBuf {
    pub fn new(ctx: &crate::asyncs::Context, max: usize, tmout: Duration) -> Self {
        let ctx = ctx.child();
        Self {
            ctx: ctx.clone(),
            buf: RwLock::new(ByteBoxBuf::new()),
            max: AtomicUsize::new(max),
            tmout: tmout,
            wkr_can_read: WakerFut::new(&ctx),
            wkr_can_write: WakerFut::new(&ctx),

            wk_can_read: None,
            wk_can_write: None,
        }
    }
    pub fn ctx(&self) -> &crate::asyncs::Context {
        &self.ctx
    }
    pub fn done_err(&self) -> std::io::Result<()> {
        if self.ctx.cancelled() {
            Err(crate::ioerr(
                "close chan!!!",
                Some(io::ErrorKind::BrokenPipe),
            ))
        } else {
            Ok(())
        }
    }
    pub fn close(&self) {
        self.ctx.cancel();
        self.wkr_can_read.close();
        self.wkr_can_write.close();
    }
    pub async fn waits(&self, tmout: Option<Duration>) {
        while !self.ctx.cancelled() {
            let lkv = self.buf.read().await;
            if lkv.len() <= 0 {
                break;
            }
            std::mem::drop(lkv);
            let _ = match tmout {
                None => self.ctx.wait_fut(self.wkr_can_write.clone()).await,
                Some(v) => {
                    self.ctx
                        .child_timeout(v)
                        .wait_fut(self.wkr_can_write.clone())
                        .await
                }
            };
        }
    }
    pub async fn clear(&self) {
        let mut lkv = self.buf.write().await;
        lkv.clear();
        self.notify_all();
    }
    pub async fn push_all(&self, data: &ByteBoxBuf) -> io::Result<usize> {
        let mut ln = 0;
        for v in data.iter() {
            ln += self.push(v.clone()).await?;
        }
        Ok(ln)
    }

    pub async fn push_front<T: Into<bytes::Bytes>>(&self, data: T) -> io::Result<usize> {
        if self.get_max() > 0 {
            loop {
                self.done_err()?;
                if self.buf.read().await.len() <= self.get_max() {
                    break;
                }
                let _ = asyncs::timeouts(self.tmout.clone(), self.wkr_can_write.clone()).await;
            }
        }
        self.done_err()?;
        let mut lkv = self.buf.write().await;
        let dt = data.into();
        let ln = dt.len();
        lkv.push_front(dt);
        self.notify_all_can_read();
        Ok(ln)
    }
    pub async fn push<T: Into<bytes::Bytes>>(&self, data: T) -> io::Result<usize> {
        if self.get_max() > 0 {
            loop {
                self.done_err()?;
                if self.buf.read().await.len() <= self.get_max() {
                    break;
                }
                let _ = self
                    .ctx
                    .child_timeout(self.tmout.clone())
                    .wait_fut(self.wkr_can_write.clone())
                    .await;
            }
        }
        self.done_err()?;
        let mut lkv = self.buf.write().await;
        let dt = data.into();
        let ln = dt.len();
        lkv.push(dt);
        self.notify_all_can_read();
        Ok(ln)
    }
    pub async fn pull(&self) -> Option<bytes::Bytes> {
        while !self.ctx.cancelled() {
            if self.buf.read().await.len() > 0 {
                break;
            }
            let _ = self
                .ctx
                .child_timeout(self.tmout.clone())
                .wait_fut(self.wkr_can_read.clone())
                .await;
        }
        let mut lkv = self.buf.write().await;
        let rts = lkv.pull();
        self.notify_all_can_write();
        rts
    }
    pub async fn pull_max(&self, max: usize) -> Option<bytes::Bytes> {
        while !self.ctx.cancelled() {
            if self.buf.read().await.len() > 0 {
                break;
            }
            let _ = self
                .ctx
                .child_timeout(self.tmout.clone())
                .wait_fut(self.wkr_can_read.clone())
                .await;
        }
        let mut lkv = self.buf.write().await;
        let rts = match lkv.pull() {
            None => None,
            Some(mut bts) => {
                let bt = bts.split_tos(max);
                if bts.len() > 0 {
                    lkv.push_front(bts);
                }
                Some(bt)
            }
        };
        self.notify_all_can_write();
        rts
    }
    pub async fn pull_size(
        &self,
        ctx: Option<&crate::asyncs::Context>,
        sz: usize,
    ) -> io::Result<ByteBoxBuf> {
        self.more_max(sz).await;
        while !self.ctx.cancelled() {
            if let Some(v) = ctx {
                v.done_err()?;
            }
            if self.buf.read().await.len() >= sz {
                break;
            }
            // self.wkr2.wait_timeout(self.tmout.clone());
            let _ = self
                .ctx
                .child_timeout(self.tmout.clone())
                .wait_fut(self.wkr_can_read.clone())
                .await;
        }
        let mut lkv = self.buf.write().await;
        let rts = lkv.cut_front(sz);
        self.notify_all_can_write();
        rts
    }
    fn notify_all_can_read(&self) {
        self.wkr_can_read.notify_all();
        if let Some(v) = &self.wk_can_read {
            v.wake_by_ref();
        }
    }
    fn notify_all_can_write(&self) {
        self.wkr_can_write.notify_all();
        if let Some(v) = &self.wk_can_write {
            v.wake_by_ref();
        }
    }
    pub fn notify_all(&self) {
        self.notify_all_can_read();
        self.notify_all_can_write();
    }
    /* pub async fn clear(&self) {
        let mut lkv = self.buf.write().await;
        lkv.clear();
        self.wkr1.notify_one();
    } */
    pub async fn len(&self) -> usize {
        self.buf.read().await.len()
    }
    pub async fn frtlen(&self) -> usize {
        self.buf.read().await.frtlen()
    }
    pub fn get_max(&self) -> usize {
        self.max.load(Ordering::SeqCst)
    }
    pub fn set_max(&self, max: usize) {
        self.max.store(max, Ordering::SeqCst);
    }
    pub fn set_maxs(&self, max: usize) {
        let maxs = self.get_max();
        if max > maxs {
            self.set_max(max);
        }
    }
    pub async fn more_max(&self, adds: usize) {
        let maxs = self.get_max();
        let sz = { self.buf.read().await.len() + adds };
        if sz > maxs {
            self.set_max(sz);
        }
    }

    pub async fn get_byte(&self, idx: usize) -> io::Result<u8> {
        let lkv = self.buf.read().await;
        lkv.get_byte(idx)
    }

    async fn readbts(&self, ln: usize) -> std::io::Result<bytes::Bytes> {
        match self.pull().await {
            None => Err(crate::ioerr(
                "buff is closed?",
                Some(std::io::ErrorKind::BrokenPipe),
            )),
            Some(mut it) => {
                let bt = it.split_tos(ln);
                if it.len() > 0 {
                    self.buf.write().await.push_front(it);
                }
                Ok(bt)
            }
        }
    }
}

#[cfg(feature = "asyncs")]
impl crate::asyncs::AsyncRead for ByteSteamBuf {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<io::Result<usize>> {
        self.wk_can_read = Some(cx.waker().clone());
        let rst = match std::pin::pin!(self.readbts(buf.len())).poll(cx) {
            std::task::Poll::Pending => return std::task::Poll::Pending,
            std::task::Poll::Ready(Err(e)) => Err(e),
            std::task::Poll::Ready(Ok(it)) => {
                let bufs = &mut buf[..it.len()];
                bufs.copy_from_slice(&it[..]);
                Ok(it.len())
            }
        };
        std::task::Poll::Ready(rst)
    }
}
#[cfg(feature = "asyncs")]
impl crate::asyncs::AsyncWrite for ByteSteamBuf {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        self.wk_can_write = Some(cx.waker().clone());
        let bts = bytes::Bytes::copy_from_slice(buf);
        let rst = match std::pin::pin!(self.push(bts)).poll(cx) {
            std::task::Poll::Pending => return std::task::Poll::Pending,
            std::task::Poll::Ready(Err(e)) => Err(e),
            std::task::Poll::Ready(Ok(_)) => Ok(buf.len()),
        };
        std::task::Poll::Ready(rst)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        if self.ctx.done() {
            return std::task::Poll::Ready(Err(crate::ioerr(
                "buff is closed?",
                Some(std::io::ErrorKind::BrokenPipe),
            )));
        }
        self.wk_can_write = Some(cx.waker().clone());
        match std::pin::pin!(self.waits(None)).poll(cx) {
            std::task::Poll::Pending => std::task::Poll::Pending,
            std::task::Poll::Ready(_) => std::task::Poll::Ready(Ok(())),
        }
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        self.close();
        std::task::Poll::Ready(Ok(()))
    }
}
#[cfg(feature = "tokios")]
impl crate::asyncs::AsyncRead for ByteSteamBuf {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        self.wk_can_read = Some(cx.waker().clone());
        let rst = match std::pin::pin!(self.readbts(buf.remaining())).poll(cx) {
            std::task::Poll::Pending => return std::task::Poll::Pending,
            std::task::Poll::Ready(Err(e)) => Err(e),
            std::task::Poll::Ready(Ok(it)) => {
                buf.put_slice(&it);
                Ok(())
            }
        };
        std::task::Poll::Ready(rst)
    }
}

#[cfg(feature = "tokios")]
impl crate::asyncs::AsyncWrite for ByteSteamBuf {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, io::Error>> {
        self.wk_can_write = Some(cx.waker().clone());
        let bts = bytes::Bytes::copy_from_slice(buf);
        let rst = match std::pin::pin!(self.push(bts)).poll(cx) {
            std::task::Poll::Pending => return std::task::Poll::Pending,
            std::task::Poll::Ready(Err(e)) => Err(e),
            std::task::Poll::Ready(Ok(_)) => Ok(buf.len()),
        };
        std::task::Poll::Ready(rst)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        if self.ctx.cancelled() {
            return std::task::Poll::Ready(Err(crate::ioerr(
                "buff is closed?",
                Some(std::io::ErrorKind::BrokenPipe),
            )));
        }
        self.wk_can_write = Some(cx.waker().clone());
        match std::pin::pin!(self.waits(None)).poll(cx) {
            std::task::Poll::Pending => std::task::Poll::Pending,
            std::task::Poll::Ready(_) => std::task::Poll::Ready(Ok(())),
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        self.close();
        std::task::Poll::Ready(Ok(()))
    }
}

pub struct PeekStream<IO> {
    inner: Box<PeekInner<IO>>,
}
struct PeekInner<IO> {
    ctx: crate::asyncs::Context,
    otrbts: ByteBoxBuf,
    stream: std::pin::Pin<Box<IO>>,
}
impl<IO> PeekStream<IO> {
    pub fn new(ctx: &crate::asyncs::Context, stream: IO) -> Self {
        Self {
            inner: Box::new(PeekInner {
                ctx: ctx.clone(),
                stream: Box::pin(stream),
                otrbts: ByteBoxBuf::new(),
            }),
        }
    }
    pub fn push_otrbts<T: Into<bytes::Bytes>>(&mut self, data: T) {
        let bts = data.into();
        if bts.len() <= 0 {
            return;
        }
        self.inner.otrbts.push(bts);
    }
    pub fn push_otrbuf(&mut self, buf: &ByteBoxBuf) {
        if buf.len() <= 0 {
            return;
        }
        self.inner.otrbts.push_all(buf);
    }
    pub fn repush_otrbuf(&mut self, buf: ByteBoxBuf) {
        if buf.len() <= 0 {
            return;
        }
        if self.inner.otrbts.len() <= 0 {
            self.inner.otrbts = buf;
        } else {
            // 从后向前
            for it in buf.iter().rev() {
                self.inner.otrbts.push_front(it.clone());
            }
        }
    }
    /* pub fn own_io(&mut self) -> IO {
        std::mem::take(&mut self.inner.stream)
    } */
}
impl<IO> PeekInner<IO>
where
    IO: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    fn poll_reads(
        &mut self,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if buf.remaining() <= 0 {
            return std::task::Poll::Ready(Ok(()));
        }

        let mut read_otrbts = false;
        while self.otrbts.len() > 0 && buf.remaining() > 0 {
            if let Some(mut bts) = self.otrbts.pull() {
                let bt = bts.split_tos(buf.remaining());
                if bts.len() > 0 {
                    self.otrbts.push_front(bts);
                }
                if bt.len() > 0 {
                    buf.put_slice(&bt);
                    read_otrbts = true;
                }
            } else {
                break;
            }
        }

        if read_otrbts {
            return std::task::Poll::Ready(Ok(()));
        }

        self.stream.as_mut().poll_read(cx, buf)
    }
}
#[cfg(feature = "tokios")]
impl<IO> tokio::io::AsyncRead for PeekStream<IO>
where
    IO: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if self.inner.ctx.cancelled() {
            return std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "ctx end,conn",
            )));
        }
        let this = self.get_mut();
        let rst = this.inner.poll_reads(cx, buf);
        rst
    }
}

#[cfg(all(test, feature = "tokios"))]
mod tests {
    use super::PeekStream;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn poll_reads_fills_from_multiple_otrbts_chunks() {
        let ctx = crate::asyncs::Context::new();
        let (stream, _peer) = tokio::io::duplex(64);
        let mut stream = PeekStream::new(&ctx, stream);

        stream.push_otrbts("ab");
        stream.push_otrbts("cd");
        stream.push_otrbts("ef");

        let mut buf = [0u8; 5];
        let n = stream.read(&mut buf).await.unwrap();
        assert_eq!(n, 5);
        assert_eq!(&buf, b"abcde");

        let mut buf = [0u8; 5];
        let n = stream.read(&mut buf).await.unwrap();
        assert_eq!(n, 1);
        assert_eq!(&buf[..n], b"f");
    }

    #[tokio::test]
    async fn poll_reads_keeps_remainder_when_readbuf_is_smaller_than_chunk() {
        let ctx = crate::asyncs::Context::new();
        let (stream, _peer) = tokio::io::duplex(64);
        let mut stream = PeekStream::new(&ctx, stream);

        stream.push_otrbts("abcdef");

        let mut buf = [0u8; 2];
        let n = stream.read(&mut buf).await.unwrap();
        assert_eq!(n, 2);
        assert_eq!(&buf, b"ab");

        let n = stream.read(&mut buf).await.unwrap();
        assert_eq!(n, 2);
        assert_eq!(&buf, b"cd");

        let n = stream.read(&mut buf).await.unwrap();
        assert_eq!(n, 2);
        assert_eq!(&buf, b"ef");
    }

    #[tokio::test]
    async fn poll_reads_uses_stream_after_otrbts_is_empty() {
        let ctx = crate::asyncs::Context::new();
        let (stream, mut peer) = tokio::io::duplex(64);
        let mut stream = PeekStream::new(&ctx, stream);

        stream.push_otrbts("ab");
        peer.write_all(b"cd").await.unwrap();

        let mut buf = [0u8; 2];
        let n = stream.read(&mut buf).await.unwrap();
        assert_eq!(n, 2);
        assert_eq!(&buf, b"ab");

        let n = stream.read(&mut buf).await.unwrap();
        assert_eq!(n, 2);
        assert_eq!(&buf, b"cd");
    }
}

#[cfg(feature = "tokios")]
impl<IO> tokio::io::AsyncWrite for PeekStream<IO>
where
    IO: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        if self.inner.ctx.cancelled() {
            return std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "ctx end,conn",
            )));
        }
        let this = self.get_mut();
        let rst = this.inner.stream.as_mut().poll_write(cx, buf);
        rst
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.get_mut();
        this.inner.stream.as_mut().poll_flush(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        // log::debug!("TcpConn shutdown: addr={}", &self.inner.info.addrcli());
        let this = self.get_mut();
        this.inner.stream.as_mut().poll_shutdown(cx)
    }
}
