use std::{
    future::Future,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

#[derive(Clone)]
pub struct WakerFut {
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

impl WakerFut {
    pub fn new() -> Self {
        // let (sx, rx) = channel::unbounded::<()>();
        Self {
            inner: crate::ArcMut::new(Inner {
                closed: AtomicBool::new(false),
                ticks: Mutex::new(Vec::new()),
            }),
        }
    }
    async fn close(&self) {
        if self.inner.closed.load(Ordering::SeqCst) {
            return;
        }
        self.inner.closed.store(true, Ordering::SeqCst);
    }
    pub fn notify_one(&self) {
        if self.inner.closed.load(Ordering::SeqCst) {
            return;
        }

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
        if self.inner.closed.load(Ordering::SeqCst) {
            return;
        }

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
    fn checks(&self, it: &Item) -> bool {
        if self.inner.closed.load(Ordering::SeqCst) {
            return true;
        }
        if it.ticked.load(Ordering::SeqCst) {
            return true;
        }
        false
    }
}

impl Future for WakerFut {
    type Output = ();
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut lkv = match self.inner.ticks.lock() {
            Ok(v) => v,
            Err(_) => return std::task::Poll::Pending,
        };

        let mut i = 0;
        for v in &*lkv {
            if v.wk.will_wake(cx.waker()) {
                if self.checks(v) {
                    lkv.remove(i);
                    return std::task::Poll::Ready(());
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
