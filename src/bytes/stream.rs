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
    wkr_can_read: WakerFut,
    wkr_can_write: WakerFut,

    wk_can_read: Option<std::task::Waker>,
    wk_can_write: Option<std::task::Waker>,
}

impl ByteSteamBuf {
    pub fn new(ctx: &crate::Context, max: usize, tmout: Duration) -> Self {
        let ctx = crate::Context::background(Some(ctx.clone()));
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
    pub fn ctx(&self) -> &crate::Context {
        &self.ctx
    }
    pub fn done_err(&self) -> std::io::Result<()> {
        match self.ctx.done_err() {
            Ok(_) => Ok(()),
            Err(e) => Err(crate::ioerr(
                "close chan!!!",
                Some(io::ErrorKind::BrokenPipe),
            )),
        }
    }
    pub fn close(&self) {
        self.ctx.stop();
        self.wkr_can_read.close();
        self.wkr_can_write.close();
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
            let _ = asyncs::timeout(Duration::from_millis(200), self.wkr_can_write.clone()).await;
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

    pub async fn push_front<T: Into<ByteBox>>(&self, data: T) -> io::Result<usize> {
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
    pub async fn push<T: Into<ByteBox>>(&self, data: T) -> io::Result<usize> {
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
        lkv.push(dt);
        self.notify_all_can_read();
        Ok(ln)
    }
    pub async fn pull(&self) -> Option<ByteBox> {
        while !self.ctx.done() {
            if self.buf.read().await.len() > 0 {
                break;
            }
            let _ = asyncs::timeouts(self.tmout.clone(), self.wkr_can_read.clone()).await;
        }
        let mut lkv = self.buf.write().await;
        let rts = lkv.pull();
        self.notify_all_can_write();
        rts
    }
    pub async fn pull_max(&self, max: usize) -> Option<ByteBox> {
        while !self.ctx.done() {
            if self.buf.read().await.len() > 0 {
                break;
            }
            let _ = asyncs::timeouts(self.tmout.clone(), self.wkr_can_read.clone()).await;
        }
        let mut lkv = self.buf.write().await;
        let rts = match lkv.pull() {
            None => None,
            Some(mut bts) => {
                if bts.len() > max {
                    if let Ok(obts) = bts.cut(max) {
                        lkv.push_front(obts);
                    }
                }
                Some(bts)
            }
        };
        self.notify_all_can_write();
        rts
    }
    pub async fn pull_size(
        &self,
        ctx: Option<&crate::Context>,
        sz: usize,
    ) -> io::Result<ByteBoxBuf> {
        self.more_max(sz).await;
        while !self.ctx.done() {
            if let Some(v) = ctx {
                v.done_err()?;
            }
            let lkv = self.buf.read().await;
            if lkv.len() >= sz {
                break;
            }
            std::mem::drop(lkv);
            // self.wkr2.wait_timeout(self.tmout.clone());
            let _ = asyncs::timeout(self.tmout.clone(), self.wkr_can_read.clone()).await;
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
        let rst = match std::pin::pin!(self.push(buf)).poll(cx) {
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
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, io::Error>> {
        self.wk_can_write = Some(cx.waker().clone());
        let rst = match std::pin::pin!(self.push(buf)).poll(cx) {
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

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        self.close();
        std::task::Poll::Ready(Ok(()))
    }
}
