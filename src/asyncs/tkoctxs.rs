use std::future::Future;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Weak,
};
use std::task::{Context as TaskContext, Poll};
use std::time::{Duration, Instant};

use parking_lot::RwLock;

#[derive(Clone)]
pub struct Context {
    token: CancelToken,
    // 记录 Context 创建的绝对时间点
    time_start: Instant,
    timeout_dur: Option<Duration>,
}

#[derive(Clone)]
struct CancelToken {
    inner: Arc<CancelTokenInner>,
}

struct CancelTokenInner {
    // 取消状态是热路径：Context::cancelled() 只读这个原子位，不进入锁。
    cancelled: AtomicBool,
    // 为每个等待 cancelled_future() 的 Future 分配稳定 id，便于更新/删除对应 waker。
    next_waiter_id: AtomicUsize,
    // 当前还在 Pending 的 cancelled_future()。cancel() 时取出并逐个唤醒。
    waiters: RwLock<Vec<(usize, std::task::Waker)>>,
    // 父 token 只弱引用子 token，避免 Context/CancelToken 克隆后形成强引用环。
    children: RwLock<Vec<Weak<CancelTokenInner>>>,
}

impl CancelToken {
    fn new() -> Self {
        Self {
            inner: Arc::new(CancelTokenInner {
                cancelled: AtomicBool::new(false),
                next_waiter_id: AtomicUsize::new(1),
                waiters: RwLock::new(Vec::new()),
                children: RwLock::new(Vec::new()),
            }),
        }
    }

    fn child_token(&self) -> Self {
        let child = Self::new();

        // child 注册和父级 cancel() 传播共用 children 写锁，避免父级取消过程中漏掉新子节点。
        let mut children = self.inner.children.write();
        if self.is_cancelled() {
            drop(children);
            child.cancel();
        } else {
            children.push(Arc::downgrade(&child.inner));
        }

        child
    }

    fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Acquire)
    }

    fn cancel(&self) {
        // 取消是单向状态机：false -> true 只允许一个调用者执行传播和唤醒。
        if self.inner.cancelled.swap(true, Ordering::AcqRel) {
            return;
        }

        {
            // 先传播子 token，再唤醒本 token 的 waiter；这样被唤醒后观察到的子树状态已经一致。
            let mut children = self.inner.children.write();
            for child in children.drain(..) {
                if let Some(child) = child.upgrade() {
                    Self { inner: child }.cancel();
                }
            }
        }

        let waiters = {
            let mut waiters = self.inner.waiters.write();
            std::mem::take(&mut *waiters)
        };
        for (_, waker) in waiters {
            waker.wake();
        }
    }

    fn cancelled(&self) -> CancelledFuture {
        CancelledFuture {
            token: self.clone(),
            waiter_id: None,
        }
    }

    fn insert_waiter(&self, waker: std::task::Waker) -> Option<usize> {
        let mut waiters = self.inner.waiters.write();
        // 拿到 waiters 写锁后再查一次取消位，避免 cancel() 刚发生时把 waker 注册成永远没人唤醒的 waiter。
        if self.is_cancelled() {
            return None;
        }

        let id = self.inner.next_waiter_id.fetch_add(1, Ordering::Relaxed);
        waiters.push((id, waker));
        Some(id)
    }

    fn update_waiter(&self, id: usize, waker: &std::task::Waker) {
        let mut waiters = self.inner.waiters.write();
        if let Some((_, old_waker)) = waiters.iter_mut().find(|(waiter_id, _)| *waiter_id == id) {
            if !old_waker.will_wake(waker) {
                *old_waker = waker.clone();
            }
        }
    }

    fn remove_waiter(&self, id: usize) {
        let mut waiters = self.inner.waiters.write();
        if let Some(idx) = waiters.iter().position(|(waiter_id, _)| *waiter_id == id) {
            waiters.swap_remove(idx);
        }
    }
}

struct CancelledFuture {
    token: CancelToken,
    waiter_id: Option<usize>,
}

impl Future for CancelledFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        if self.token.is_cancelled() {
            self.waiter_id = None;
            return Poll::Ready(());
        }

        // 第一次 poll 注册 waker，后续 poll 只在执行器更换 waker 时更新原记录。
        if let Some(id) = self.waiter_id {
            self.token.update_waiter(id, cx.waker());
        } else if let Some(id) = self.token.insert_waiter(cx.waker().clone()) {
            self.waiter_id = Some(id);
        } else {
            return Poll::Ready(());
        }

        if self.token.is_cancelled() {
            // 取消可能发生在注册 waker 之后、返回 Pending 之前；这里再次检查来消除竞态。
            self.waiter_id = None;
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

impl Drop for CancelledFuture {
    fn drop(&mut self) {
        // select! 丢弃未完成分支时，需要撤销注册，避免 waiters 中保留失效 waker。
        if let Some(id) = self.waiter_id.take() {
            self.token.remove_waiter(id);
        }
    }
}

impl Context {
    pub fn new() -> Self {
        Self {
            token: CancelToken::new(),
            time_start: Instant::now(),
            timeout_dur: None,
        }
    }

    fn create_child_token(&self) -> CancelToken {
        self.token.child_token()
    }

    pub fn new_timeout(tmd: Duration) -> Self {
        Self {
            token: CancelToken::new(),
            time_start: Instant::now(),
            timeout_dur: Some(tmd),
        }
    }

    pub fn prt_with_timeout(v: &Option<Self>, tmd: Duration) -> Self {
        match v {
            Some(v) => v.child_timeout(tmd),
            None => Self::new_timeout(tmd),
        }
    }

    pub fn child(&self) -> Self {
        Self {
            token: self.create_child_token(),
            time_start: Instant::now(), // 子上下文重新计时（通常子任务有自己的超时或继承父的剩余时间，这里按新任务算）
            timeout_dur: None,
        }
    }

    pub fn child_timeout(&self, tmd: Duration) -> Self {
        Self {
            token: self.create_child_token(),
            time_start: Instant::now(), // 子上下文重新计时
            timeout_dur: Some(tmd),
        }
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

    /// 获取取消信号的 Future
    pub fn cancelled_future(&self) -> impl Future<Output = ()> + '_ {
        self.token.cancelled()
    }

    /// 【核心修改】
    /// 计算从 time_start 到现在的剩余时间。
    /// 如果时间已过，返回一个立即完成的 Future。
    /// 如果没有超时设置，返回 pending。
    pub fn timeout_future(&self) -> impl Future<Output = ()> + '_ {
        if let Some(dur) = self.timeout_dur {
            let elapsed = self.time_start.elapsed();

            if elapsed >= dur {
                // 时间已经过了，返回一个立即完成的 Future (Ready)
                Either::Left(std::future::ready(()))
            } else {
                // 时间没过，睡“剩余”的时间
                let remaining = dur - elapsed;
                Either::Right(tokio::time::sleep(remaining))
            }
        } else {
            // 没有超时设置，返回永不完成
            Either::Pending
        }
    }

    pub async fn wait_futs<F, T>(&self, fut: F) -> std::io::Result<T>
    where
        F: Future<Output = std::io::Result<T>>,
    {
        match self.wait_fut(fut).await {
            CtxWaitRes::Ok(v) => v,
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
    pub async fn wait_fut<F, T>(&self, fut: F) -> CtxWaitRes<T>
    where
        F: Future<Output = T>,
    {
        // 宏会自动 pin fut
        tokio::select! {
            _ = self.cancelled_future() => {
                CtxWaitRes::Cancel
            },
            _ = self.timeout_future() => {
                self.token.cancel();
                CtxWaitRes::Timeout
            },
            v = fut => {
                CtxWaitRes::Ok(v)
            },
        }
    }

    // 大future使用这个,避免爆栈
    pub async fn wait_box_fut<F, T>(&self, fut: F) -> CtxWaitRes<T>
    where
        F: Future<Output = T>,
    {
        self.wait_fut(Box::pin(fut)).await
    }
}

pub enum CtxWaitRes<T> {
    Ok(T),
    Cancel,
    Timeout,
}

// 优化后的 Either 枚举，支持三种状态：Left, Right, Pending
enum Either<L, R> {
    Left(L),
    Right(R),
    Pending, // 专门用于表示无超时时的 pending 状态
}

impl<L, R, T> Future for Either<L, R>
where
    L: Future<Output = T>,
    R: Future<Output = T>,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<T> {
        unsafe {
            match self.get_unchecked_mut() {
                Either::Left(l) => Pin::new_unchecked(l).poll(cx),
                Either::Right(r) => Pin::new_unchecked(r).poll(cx),
                Either::Pending => Poll::Pending,
            }
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
