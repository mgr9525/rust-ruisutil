use std::{
    io,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use async_std::channel;

#[derive(Clone)]
pub struct Waker {
    inner: crate::ArcMut<Inner>,
}

struct Inner {
    closed: AtomicBool,
    sx: Option<channel::Sender<()>>,
    rx: Option<channel::Receiver<()>>,
}

impl Waker {
    pub fn new() -> Self {
        // let (sx, rx) = channel::unbounded::<()>();
        Self {
            inner: crate::ArcMut::new(Inner {
                closed: AtomicBool::new(true),
                sx: None,
                rx: None,
            }),
        }
    }
    async fn close(&self) {
        if self.inner.closed.load(Ordering::SeqCst) {
            return;
        }
        let ins = unsafe { self.inner.muts() };
        self.inner.closed.store(true, Ordering::SeqCst);
        if let Some(rx) = &self.inner.rx {
            rx.close();
        }
        // ins.sx = None;
        // ins.rx = None;
    }
    pub async fn wait(&self) -> io::Result<()> {
        if self.inner.closed.load(Ordering::SeqCst) {
            let ins = unsafe { self.inner.muts() };
            let (sx, rx) = channel::unbounded::<()>();
            self.inner.closed.store(false, Ordering::SeqCst);
            ins.sx = Some(sx);
            ins.rx = Some(rx);
        }

        if let Some(rx) = &self.inner.rx {
            if let Err(e) = rx.recv().await {
                return Err(crate::ioerr(format!("wait err:{}", e), None));
            }
        }
        Ok(())
    }
    pub async fn wait_timeout(&self, tm: Duration) -> io::Result<()> {
        match async_std::io::timeout(tm, self.wait()).await {
            Ok(v) => Ok(v),
            Err(e) => {
                self.close().await;
                Err(e)
            }
        }
    }
    pub async fn notify_one(&self) {
        if self.inner.closed.load(Ordering::SeqCst) {
            return;
        }
        if let Some(sx) = &self.inner.sx {
            sx.send(()).await;
        }
    }
    pub async fn notify_all(&self) {
        self.close().await;
    }
}
