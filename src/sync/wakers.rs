use std::{
    future::Future,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

pub struct WakerFut {
    wk: Option<std::task::Waker>,
    inner: crate::ArcMut<Inner>,
}

struct Inner {
    closed: AtomicBool,
    ticks: Mutex<Vec<Item>>,
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
    pub fn new() -> Self {
        // let (sx, rx) = channel::unbounded::<()>();
        Self {
            wk: None,
            inner: crate::ArcMut::new(Inner {
                closed: AtomicBool::new(false),
                ticks: Mutex::new(Vec::new()),
            }),
        }
    }
    pub fn close(&self) {
        if self.inner.closed.load(Ordering::SeqCst) {
            return;
        }
        self.inner.closed.store(true, Ordering::SeqCst);
        self.notify_all();
    }
    pub fn notify_one(&self) {
        let lkv = match self.inner.ticks.lock() {
            Ok(v) => v,
            Err(_) => return,
        };
        if lkv.len() > 0 {
            lkv[0].ticked.store(true, Ordering::SeqCst);
            lkv[0].wk.wake_by_ref();
        }
    }
    pub fn notify_all(&self) {
        let lkv = match self.inner.ticks.lock() {
            Ok(v) => v,
            Err(_) => return,
        };
        for v in &*lkv {
            v.ticked.store(true, Ordering::SeqCst);
            v.wk.wake_by_ref();
        }
    }
    /* fn checks(&self, cx: &mut std::task::Context<'_>) -> impl Future<Output = i32> {
        async {123}.boxed()
    } */
    fn is_close(&self) -> bool {
        self.inner.closed.load(Ordering::SeqCst)
    }
    fn checks(&self, it: &Item) -> bool {
        if self.is_close() {
            return true;
        }
        if it.ticked.load(Ordering::SeqCst) {
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
        if this.is_close() {
            return std::task::Poll::Ready(Ok(()));
        }
        let mut lkv = match this.inner.ticks.lock() {
            Ok(v) => v,
            Err(_) => return std::task::Poll::Pending,
        };
        match &this.wk {
            None => this.wk = Some(cx.waker().clone()),
            Some(vs) => {
                if !vs.will_wake(cx.waker()) {
                    let mut i = 0;
                    for v in &*lkv {
                        if v.wk.will_wake(vs) {
                            lkv.remove(i);
                            break;
                        }
                        i += 1;
                    }
                    this.wk = Some(cx.waker().clone());
                }
            }
        }

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
