use std::{
    future::Future,
    io,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use asyncs::sync::RwLock;

use crate::{asyncs, sync::WakerFut};

use super::{ByteBox, ByteBoxBuf};

pub struct ByteSteamBuf {
    ctx: crate::Context,
    buf: RwLock<ByteBoxBuf>,
    max: AtomicUsize,
    tmout: Duration,
    wkr1: WakerFut,
    wkr2: WakerFut,
}

impl ByteSteamBuf {
    pub fn new(ctx: &crate::Context, max: usize, tmout: Duration) -> Self {
        let ctx = crate::Context::background(Some(ctx.clone()));
        Self {
            ctx: ctx.clone(),
            buf: RwLock::new(ByteBoxBuf::new()),
            max: AtomicUsize::new(max),
            tmout: tmout,
            wkr1: WakerFut::new(&ctx),
            wkr2: WakerFut::new(&ctx),
        }
    }
    pub fn close(&self) {
        self.ctx.stop();
        self.wkr1.close();
        self.wkr2.close();
    }
    pub async fn waits(&self, tmout: Option<Duration>) {
        let ctxs = match tmout {
            Some(v) => crate::Context::with_timeout(Some(self.ctx.clone()), v),
            None => self.ctx.clone(),
        };
        while !ctxs.done() {
            let lkv = self.buf.read().await;
            if lkv.len() <= 0 {
                break;
            }
            std::mem::drop(lkv);
            asyncs::timeout(Duration::from_millis(200), self.wkr1.clone()).await;
        }
    }
    pub async fn clear(&self) {
        let mut lkv = self.buf.write().await;
        lkv.clear();
        self.wkr1.notify_all();
        self.wkr2.notify_all();
    }
    pub async fn push_all(&self, data: &ByteBoxBuf) -> io::Result<()> {
        for v in data.iter() {
            self.push(v.clone()).await?;
        }
        Ok(())
    }
    pub async fn push<T: Into<ByteBox>>(&self, data: T) -> io::Result<()> {
        if self.get_max() > 0 {
            loop {
                if self.ctx.done() {
                    return Err(crate::ioerr(
                        "close chan!!!",
                        Some(io::ErrorKind::BrokenPipe),
                    ));
                }
                let lkv = self.buf.read().await;
                if lkv.len() <= self.get_max() {
                    break;
                }
                std::mem::drop(lkv);
                // self.wkr1.wait_timeout(self.tmout.clone());
                asyncs::timeout(self.tmout.clone(), self.wkr1.clone()).await;
                // self.wkr1.notify_all();
            }
        }
        let mut lkv = self.buf.write().await;
        lkv.push(data);
        self.wkr2.notify_all();
        Ok(())
    }
    pub async fn pull(&self) -> Option<ByteBox> {
        while !self.ctx.done() {
            let lkv = self.buf.read().await;
            if lkv.len() > 0 {
                break;
            }
            std::mem::drop(lkv);
            // self.wkr2.wait_timeout(self.tmout.clone());
            asyncs::timeout(self.tmout.clone(), self.wkr2.clone()).await;
            // self.wkr2.notify_all();
        }
        let mut lkv = self.buf.write().await;
        let rts = lkv.pull();
        self.wkr1.notify_all();
        rts
    }
    pub async fn pull_size(
        &self,
        ctx: Option<&crate::Context>,
        sz: usize,
    ) -> io::Result<ByteBoxBuf> {
        self.more_max(sz).await;
        loop {
            if self.ctx.done() {
                return Err(crate::ioerr(
                    "close chan!!!",
                    Some(io::ErrorKind::BrokenPipe),
                ));
            }
            if let Some(v) = ctx {
                if v.done() {
                    return Err(crate::ioerr("ctx end!!!", Some(io::ErrorKind::BrokenPipe)));
                }
            }
            let lkv = self.buf.read().await;
            if lkv.len() >= sz {
                break;
            }
            std::mem::drop(lkv);
            // self.wkr2.wait_timeout(self.tmout.clone());
            asyncs::timeout(self.tmout.clone(), self.wkr2.clone()).await;
        }
        let mut lkv = self.buf.write().await;
        lkv.cut_front(sz)
        /* match lkv.cut_front(sz) {
            Err(e) => Err(e),
            Ok(v) => Ok(v.to_bytes()),
        } */
    }
    pub fn notify_all(&self) {
        self.wkr1.notify_all();
        self.wkr2.notify_all();
    }
    /* pub async fn clear(&self) {
        let mut lkv = self.buf.write().await;
        lkv.clear();
        self.wkr1.notify_one();
    } */
    pub async fn len(&self) -> usize {
        let lkv = self.buf.read().await;
        lkv.len()
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

    async fn readbts(&self, ln: usize) -> std::io::Result<ByteBox> {
        match self.pull().await {
            None => Err(crate::ioerr(
                "buff is closed?",
                Some(std::io::ErrorKind::BrokenPipe),
            )),
            Some(mut it) => {
                if ln < it.len() {
                    let mut lkv = self.buf.write().await;
                    if let Ok(rgt) = it.cut(ln) {
                        lkv.push_front(rgt);
                    }
                }
                Ok(it)
            }
        }
    }
}

#[cfg(feature = "asyncs")]
impl crate::asyncs::AsyncRead for ByteSteamBuf {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<io::Result<usize>> {
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
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        let rst = match std::pin::pin!(self.push(buf)).poll(cx) {
            std::task::Poll::Pending => return std::task::Poll::Pending,
            std::task::Poll::Ready(Err(e)) => Err(e),
            std::task::Poll::Ready(Ok(_)) => Ok(buf.len()),
        };
        std::task::Poll::Ready(rst)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
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
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        let rst = match std::pin::pin!(self.readbts(buf.remaining())).poll(cx) {
            std::task::Poll::Pending => return std::task::Poll::Pending,
            std::task::Poll::Ready(Err(e)) => Err(e),
            std::task::Poll::Ready(Ok(it)) => {
                buf.put_slice(&it[..]);
                Ok(())
            }
        };
        std::task::Poll::Ready(rst)
    }
}

#[cfg(feature = "tokios")]
impl crate::asyncs::AsyncWrite for ByteSteamBuf {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, io::Error>> {
        let rst = match std::pin::pin!(self.push(buf)).poll(cx) {
            std::task::Poll::Pending => return std::task::Poll::Pending,
            std::task::Poll::Ready(Err(e)) => Err(e),
            std::task::Poll::Ready(Ok(_)) => Ok(buf.len()),
        };
        std::task::Poll::Ready(rst)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        self.close();
        std::task::Poll::Ready(Ok(()))
    }
}
