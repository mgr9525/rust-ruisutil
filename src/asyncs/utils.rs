use std::future::Future;
use std::{collections::HashMap, sync::Arc};

pub async fn waitctx(ctx: &crate::Context) {
    while !ctx.done() {
        super::sleep(std::time::Duration::from_millis(50)).await;
    }
}
pub async fn waitctxs(ctx: &crate::Context, tms: std::time::Duration) {
    let cx = crate::Context::with_timeout(Some(ctx.clone()), tms);
    waitctx(&cx).await
}

/// Creates a future from a function that returns `Poll`.
pub fn poll_fn<T, F: FnMut(&mut std::task::Context<'_>) -> T>(f: F) -> PollFn<F> {
    PollFn(f)
}

/// The future created by `poll_fn`.
pub struct PollFn<F>(F);

impl<F> Unpin for PollFn<F> {}

impl<T, F: FnMut(&mut std::task::Context<'_>) -> std::task::Poll<T>> Future for PollFn<F> {
    type Output = T;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        (self.0)(cx)
    }
}

pub fn async_fn<'a, T>(
    cx: &mut std::task::Context<'_>,
    f: impl Future<Output = T> + Send + 'a,
) -> std::task::Poll<T> {
    // let inner = Box::pin(f);
    // inner.as_mut().poll(cx)
    std::pin::pin!(f).poll(cx)
}

/// 不适合高并发,谨慎使用
pub struct AsyncFnFuture<'a, T> {
    fut: Option<std::pin::Pin<Box<dyn Future<Output = T> + Send + Sync + 'a>>>,
    futs: Option<
        Arc<
            std::sync::Mutex<Option<std::pin::Pin<Box<dyn Future<Output = T> + Send + Sync + 'a>>>>,
        >,
    >,
}
impl<'a, T> AsyncFnFuture<'a, T> {
    pub fn new(synced: bool) -> Self {
        Self {
            fut: None,
            futs: if synced {
                Some(Arc::new(std::sync::Mutex::new(None)))
            } else {
                None
            },
        }
    }
    pub fn polls(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<T>> {
        if let Some(ft) = &self.futs {
            let mut lkv = match ft.try_lock() {
                Ok(lkv) => lkv,
                Err(_) => {
                    cx.waker().wake_by_ref();
                    return std::task::Poll::Pending;
                }
            };
            if lkv.is_none() {
                // self.wkr = Some(cx.waker().clone());
                return std::task::Poll::Ready(Err(crate::ioerr("no future", None)));
            }
            let rst = std::pin::pin!(lkv.as_mut().unwrap()).poll(cx);
            match rst {
                std::task::Poll::Ready(v) => {
                    *lkv = None;
                    std::task::Poll::Ready(Ok(v))
                }
                std::task::Poll::Pending => std::task::Poll::Pending,
            }
        } else {
            if self.fut.is_none() {
                // self.wkr = Some(cx.waker().clone());
                // return std::task::Poll::Pending;
                // self.fut = Some(Box::pin(fc()));
                return std::task::Poll::Ready(Err(crate::ioerr("no future", None)));
            }
            let rst = std::pin::pin!(self.fut.as_mut().unwrap()).poll(cx);
            match rst {
                std::task::Poll::Ready(v) => {
                    self.fut = None;
                    std::task::Poll::Ready(Ok(v))
                }
                std::task::Poll::Pending => std::task::Poll::Pending,
            }
        }
    }
    pub fn setpoll<F>(&mut self, fc: impl FnOnce() -> F) -> std::io::Result<()>
    where
        F: Future<Output = T> + Send + Sync + 'a,
    {
        if let Some(ft) = &self.futs {
            let mut lkv = match ft.try_lock() {
                Ok(lkv) => lkv,
                Err(_) => {
                    return Err(crate::ioerr("lock futs error", None));
                }
            };
            *lkv = Some(Box::pin(fc()));
        } else {
            self.fut = Some(Box::pin(fc()));
        }
        Ok(())
    }
}

/* pub struct AsyncMapFuture<'a, T> {
    curmax: usize,
    durout: std::time::Duration,
    tmr: crate::Timer,
    futmps: Arc<std::sync::Mutex<Vec<AsyncMapItem<'a, T>>>>,
}
struct AsyncMapItem<'a, T> {
    ln: i32,
    tms: std::time::Instant,
    wkr: std::task::Waker,
    fut: std::pin::Pin<Box<dyn Future<Output = T> + Send + Sync + 'a>>,
}
impl<'a, T> AsyncMapFuture<'a, T> {
    pub fn new(curmax: usize, duroutms: std::time::Duration) -> Self {
        Self {
            curmax: curmax,
            durout: duroutms,
            tmr: crate::Timer::new(std::time::Duration::from_millis(100)),
            futmps: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
    pub fn polls(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<T>> {
        // let key = cx.waker().clone().data() as usize;
        let mut lkv = match self.futmps.try_lock() {
            Ok(lkv) => lkv,
            Err(_) => {
                cx.waker().wake_by_ref();
                return std::task::Poll::Pending;
            }
        };
        let mut idx: Option<usize> = None;
        for (i, v) in lkv.iter().enumerate() {
            if v.wkr.will_wake(cx.waker()) {
                // lkv.remove(i);
                // return std::pin::pin!(it.fut).poll(cx);
                idx = Some(i);
                break;
            }
        }
        if lkv.len() >= self.curmax {
            if self.tmr.tick() {
                let mut ixs = Vec::new();
                for (i, v) in lkv.iter_mut().enumerate() {
                    if v.tms.elapsed() > self.durout {
                        v.wkr.wake_by_ref();
                        v.ln += 1;
                        // 1秒
                        if v.ln > 10 {
                            ixs.push(i);
                        }
                    }
                }
                for i in ixs {
                    lkv.remove(i);
                }
            }
            cx.waker().wake_by_ref();
            return std::task::Poll::Pending;
        }
        if let Some(i) = idx {
            let rst = std::pin::pin!(lkv[i].fut.as_mut()).poll(cx);
            match rst {
                std::task::Poll::Ready(v) => {
                    lkv.remove(i);
                    std::task::Poll::Ready(Ok(v))
                }
                std::task::Poll::Pending => std::task::Poll::Pending,
            }
        } else {
            std::task::Poll::Ready(Err(crate::ioerr("poll error,no future", None)))
        }
    }

    pub fn setpoll<F>(
        &mut self,
        cx: &mut std::task::Context<'_>,
        fc: impl FnOnce() -> F,
    ) -> std::io::Result<()>
    where
        F: Future<Output = T> + Send + Sync + 'a,
    {
        let mut lkv = match self.futmps.try_lock() {
            Ok(lkv) => lkv,
            Err(_) => {
                return Err(crate::ioerr("lock futmps error", None));
            }
        };
        let fut = Box::pin(fc());
        lkv.push(AsyncMapItem {
            ln: 0,
            tms: std::time::Instant::now(),
            wkr: cx.waker().clone(),
            fut: fut,
        });
        Ok(())
    }
} */
