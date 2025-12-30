use std::{
    future::Future,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
    task::Poll,
    time::Duration,
};

use crate::Context;

pub struct WakerFut {
    wk: Option<std::task::Waker>,
    inner: std::sync::Arc<Inner>,
}

struct Inner {
    ctx: Context,
    ticks: std::sync::Mutex<Vec<Item>>,
}
struct Item {
    ticked: AtomicBool,
    wk: std::task::Waker,
}

impl Clone for WakerFut {
    fn clone(&self) -> Self {
        Self {
            wk: None,
            inner: self.inner.clone(),
        }
    }
}
impl WakerFut {
    pub fn new(ctx: &Context) -> Self {
        // let (sx, rx) = channel::unbounded::<()>();
        Self {
            wk: None,
            inner: std::sync::Arc::new(Inner {
                ctx: Context::background(Some(ctx.clone())),
                ticks: std::sync::Mutex::new(Vec::new()),
            }),
        }
    }
    pub fn done(&self) -> bool {
        self.inner.ctx.done()
    }
    pub fn close(&self) {
        if self.inner.ctx.done() {
            return;
        }
        self.inner.ctx.stop();
        self.notify_all();
    }
    pub fn notify_one(&self) -> bool {
        let lkv = match self.inner.ticks.try_lock() {
            Ok(v) => v,
            Err(_) => return false,
        };
        if lkv.len() > 0 {
            lkv[0].ticked.store(true, Ordering::SeqCst);
            lkv[0].wk.wake_by_ref();
        }
        true
    }
    pub fn notify_all(&self) -> bool {
        let lkv = match self.inner.ticks.try_lock() {
            Ok(v) => v,
            Err(_) => return false,
        };
        for v in &*lkv {
            v.ticked.store(true, Ordering::SeqCst);
            v.wk.wake_by_ref();
        }
        true
    }
    fn checks(&self, it: &Item) -> bool {
        if self.done() {
            return true;
        }
        if it.ticked.load(Ordering::SeqCst) {
            it.ticked.store(false, Ordering::SeqCst);
            return true;
        }
        false
    }
}

impl Future for WakerFut {
    type Output = std::io::Result<()>;
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();
        if this.done() {
            return std::task::Poll::Ready(Ok(()));
        }
        let mut lkv = match this.inner.ticks.try_lock() {
            Ok(v) => v,
            Err(_) => {
                cx.waker().wake_by_ref();
                return std::task::Poll::Pending;
            }
        };
        if let Some(vs) = &this.wk {
            if !vs.will_wake(cx.waker()) {
                let mut i = 0;
                for v in &*lkv {
                    if v.wk.will_wake(vs) {
                        lkv.remove(i);
                        break;
                    }
                    i += 1;
                }
            }
        }
        this.wk = Some(cx.waker().clone());

        let mut i = 0;
        for v in &*lkv {
            if v.wk.will_wake(cx.waker()) {
                if this.checks(v) {
                    lkv.remove(i);
                    return std::task::Poll::Ready(Ok(()));
                }
                return std::task::Poll::Pending;
            }
            i += 1;
        }

        lkv.push(Item {
            ticked: AtomicBool::new(false),
            wk: cx.waker().clone(),
        });

        std::task::Poll::Pending
    }
}

impl Drop for WakerFut {
    fn drop(&mut self) {
        if let Some(wk) = &self.wk {
            if let Ok(mut lkv) = self.inner.ticks.lock() {
                let mut i = 0;
                for v in &*lkv {
                    if v.wk.will_wake(wk) {
                        // println!("WakerFut drop rm weker:{}", i);
                        lkv.remove(i);
                        break;
                    }
                    i += 1;
                }
            }
        }
    }
}

pub struct WakerOneFut {
    inner: std::sync::Arc<WakerOneFutInner>,
}
struct WakerOneFutInner {
    done: AtomicBool,
    waker: futures::task::AtomicWaker,
}

impl Clone for WakerOneFut {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl Future for WakerOneFut {
    type Output = ();

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<()> {
        if self.inner.done.load(Ordering::SeqCst) {
            self.inner.done.store(false, Ordering::SeqCst);
            return Poll::Ready(());
        }

        self.inner.waker.register(cx.waker());
        Poll::Pending
    }
}

impl WakerOneFut {
    pub fn new() -> Self {
        Self {
            inner: std::sync::Arc::new(WakerOneFutInner {
                done: AtomicBool::new(false),
                waker: futures::task::AtomicWaker::new(),
            }),
        }
    }
    pub fn notify(&self) {
        self.inner.done.store(true, Ordering::SeqCst);
        self.inner.waker.wake();
    }
}
