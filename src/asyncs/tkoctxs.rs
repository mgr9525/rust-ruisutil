use std::future::Future;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Weak,
};
use std::task::{Context as TaskContext, Poll};
use std::time::{Duration, Instant};

use crate::asyncs::tkocncel;

#[derive(Clone)]
pub struct Context {
    token: tkocncel::CancellationToken,
    // 记录 Context 创建的绝对时间点
    time_start: Instant,
    timeout_dur: Option<Duration>,
    tmout_cncl: Arc<AtomicBool>,
}

impl Context {
    pub fn new() -> Self {
        Self {
            token: tkocncel::CancellationToken::new(),
            time_start: Instant::now(),
            timeout_dur: None,
            tmout_cncl: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn new_timeout(tmd: Duration) -> Self {
        Self {
            token: tkocncel::CancellationToken::new(),
            time_start: Instant::now(),
            timeout_dur: Some(tmd),
            tmout_cncl: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn child(&self) -> Self {
        Self {
            token: self.token.child_token(),
            time_start: Instant::now(), // 子上下文重新计时（通常子任务有自己的超时或继承父的剩余时间，这里按新任务算）
            timeout_dur: None,
            tmout_cncl: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn child_timeout(&self, tmd: Duration) -> Self {
        Self {
            token: self.token.child_token(),
            time_start: Instant::now(), // 子上下文重新计时
            timeout_dur: Some(tmd),
            tmout_cncl: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn tmout_cancel(self, v: bool) -> Self {
        self.tmout_cncl.store(v, Ordering::SeqCst);
        self
    }

    pub fn cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    pub fn cancel(&self) {
        self.token.cancel();
    }
    pub fn done_err(&self) -> std::io::Result<()> {
        if self.cancelled() {
            Err(crate::ioerr(
                "ctx end",
                Some(std::io::ErrorKind::Interrupted),
            ))
        } else {
            Ok(())
        }
    }

    pub fn future(&self) -> ContextFuture {
        ContextFuture::new(self)
    }
    /// 获取取消信号的 Future
    pub fn cancelled_future(&self) -> tkocncel::WaitForCancellationFuture<'_> {
        self.token.cancelled()
    }
    pub fn cancelled_ownedfut(&self) -> tkocncel::WaitForCancellationFutureOwned {
        self.token.clone().cancelled_owned()
    }

    /// 【核心修改】
    /// 计算从 time_start 到现在的剩余时间。
    /// 如果时间已过，返回一个立即完成的 Future。
    /// 如果没有超时设置，返回 pending。
    pub fn timeout_future(&self) -> Either {
        if let Some(dur) = self.timeout_dur {
            let elapsed = self.time_start.elapsed();

            if elapsed >= dur {
                // 时间已经过了，返回一个立即完成的 Future (Ready)
                Either::Expired
            } else {
                // 时间没过，睡“剩余”的时间
                let remaining = dur - elapsed;
                Either::Sleep(Box::pin(tokio::time::sleep(remaining)))
            }
        } else {
            // 没有超时设置，返回永不完成
            Either::Pending
        }
    }

    pub fn wait_fut<'a, F, T>(&'a self, fut: F) -> impl Future<Output = CtxWaitRes<T>> + 'a
    where
        T: 'a,
        F: Future<Output = T> + 'a,
    {
        // #[cfg(debug_assertions)]
        // log::debug!("ctx.wait_fut1 fut.szof={}", std::mem::size_of_val(&fut));
        let fut = Box::pin(fut);
        // #[cfg(debug_assertions)]
        // log::debug!("ctx.wait_fut2 fut.szof={}", std::mem::size_of_val(&fut));
        self.wait_fut_box(fut)
    }

    /*pub fn wait_fut_tmout<'a, F, T>(
        &'a self,
        tmout: Duration,
        fut: F,
    ) -> impl Future<Output = CtxWaitRes<T>> + 'a
    where
        T: 'a,
        F: Future<Output = T> + 'a,
    {
        // #[cfg(debug_assertions)]
        // log::debug!("ctx.wait_fut1 fut.szof={}", std::mem::size_of_val(&fut));
        let fut = Box::pin(fut);
        // #[cfg(debug_assertions)]
        // log::debug!("ctx.wait_fut2 fut.szof={}", std::mem::size_of_val(&fut));
        self.wait_fut_box_tmout(tmout, fut)
    } */

    pub async fn wait_fut_box<F, T>(&self, fut: Pin<Box<F>>) -> CtxWaitRes<T>
    where
        F: Future<Output = T>,
    {
        // #[cfg(debug_assertions)]
        // log::debug!("ctx.wait_fut_box fut.szof={}", std::mem::size_of_val(&fut));
        // 宏会自动 pin fut
        tokio::select! {
            _ = self.cancelled_future() => {
                CtxWaitRes::Cancel
            },
            _ = self.timeout_future() => {
                if self.tmout_cncl.load(Ordering::SeqCst) {
                    self.cancel();
                }
                CtxWaitRes::Timeout
            },
            v = fut => {
                CtxWaitRes::Ok(v)
            },
        }
    }
    /* pub async fn wait_fut_box_tmout<F, T>(&self, tmout: Duration, fut: Pin<Box<F>>) -> CtxWaitRes<T>
    where
        F: Future<Output = T>,
    {
        let fut = tokio::time::timeout(tmout, fut);
        // #[cfg(debug_assertions)]
        // log::debug!("ctx.wait_fut_box fut.szof={}", std::mem::size_of_val(&fut));
        // 宏会自动 pin fut
        tokio::select! {
            _ = self.cancelled_future() => {
                CtxWaitRes::Cancel
            },
            v = fut => {
                match v {
                    Err(_) => {
                        if self.tmout_cncl.load(Ordering::SeqCst) {
                            self.cancel();
                        }
                        CtxWaitRes::Timeout
                    },
                    Ok(v) => {
                        CtxWaitRes::Ok(v)
                    }
                }
            },
        }
    } */
}

pub enum CtxWaitRes<T> {
    Ok(T),
    Cancel,
    Timeout,
}

impl<T> CtxWaitRes<T> {
    pub fn is_ok(&self) -> bool {
        match self {
            CtxWaitRes::Ok(_) => true,
            _ => false,
        }
    }

    pub fn io_rsto(self) -> std::io::Result<T> {
        match self {
            CtxWaitRes::Ok(v) => Ok(v),
            CtxWaitRes::Cancel => Err(crate::ioerr(
                "ctx cancel",
                Some(std::io::ErrorKind::Interrupted),
            )),
            CtxWaitRes::Timeout => Err(crate::ioerr(
                "ctx timeout",
                Some(std::io::ErrorKind::TimedOut),
            )),
        }
    }

    pub fn io_rst<D>(self) -> std::io::Result<D>
    where
        T: Into<std::io::Result<D>>,
    {
        match self {
            CtxWaitRes::Ok(v) => match v.into() {
                Ok(v) => Ok(v),
                Err(e) => Err(e),
            },
            CtxWaitRes::Cancel => Err(crate::ioerr(
                "ctx cancel",
                Some(std::io::ErrorKind::Interrupted),
            )),
            CtxWaitRes::Timeout => Err(crate::ioerr(
                "ctx timeout",
                Some(std::io::ErrorKind::TimedOut),
            )),
        }
    }
    pub fn io_rsts<D, S>(self, nms: S) -> std::io::Result<D>
    where
        T: Into<std::io::Result<D>>,
        S: AsRef<str>,
    {
        match self {
            CtxWaitRes::Ok(v) => match v.into() {
                Ok(v) => Ok(v),
                Err(e) => Err(std::io::Error::new(
                    e.kind(),
                    format!("'{}' work err: {}", nms.as_ref(), e),
                )),
            },
            CtxWaitRes::Cancel => Err(crate::ioerr(
                format!("'{}' ctx cancel", nms.as_ref()),
                Some(std::io::ErrorKind::Interrupted),
            )),
            CtxWaitRes::Timeout => Err(crate::ioerr(
                format!("'{}' ctx timeout", nms.as_ref()),
                Some(std::io::ErrorKind::TimedOut),
            )),
        }
    }
}

// 优化后的 Either 枚举，支持三种状态：Left, Right, Pending
enum Either {
    Expired,
    Sleep(Pin<Box<tokio::time::Sleep>>),
    Pending, // 专门用于表示无超时时的 pending 状态
}

impl Future for Either {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<()> {
        let this = self.get_mut();
        match this {
            Either::Expired => Poll::Ready(()),
            Either::Sleep(slp) => slp.as_mut().poll(cx),
            Either::Pending => Poll::Pending,
        }
    }
}

impl From<Option<Context>> for Context {
    fn from(prt: Option<Context>) -> Self {
        match prt {
            Some(v) => v.child(),
            None => Self::new(),
        }
    }
}
impl From<&Option<Context>> for Context {
    fn from(prt: &Option<Context>) -> Self {
        match prt {
            Some(v) => v.child(),
            None => Self::new(),
        }
    }
}

pub struct ContextFuture {
    ctx: Context,
    cancel_fut: Pin<Box<tkocncel::WaitForCancellationFutureOwned>>,
    timeout_fut: Pin<Box<Either>>,
}
impl ContextFuture {
    fn new(ctx: &Context) -> Self {
        Self {
            ctx: ctx.clone(),
            cancel_fut: Box::pin(ctx.cancelled_ownedfut()),
            timeout_fut: Box::pin(ctx.timeout_future()),
        }
    }
}
impl Future for ContextFuture {
    type Output = CtxWaitRes<()>;
    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if this.cancel_fut.as_mut().poll(cx).is_ready() {
            return Poll::Ready(CtxWaitRes::Cancel);
        }
        if this.timeout_fut.as_mut().poll(cx).is_ready() {
            if this.ctx.tmout_cncl.load(Ordering::SeqCst) {
                this.ctx.cancel();
            }
            return Poll::Ready(CtxWaitRes::Timeout);
        }
        Poll::Pending
    }
}
