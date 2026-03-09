use std::future::Future;
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct Context {
    token: CancellationToken,
    // 记录 Context 创建的绝对时间点
    time_start: Instant,
    timeout_dur: Option<Duration>,
}

impl Context {
    pub fn new() -> Self {
        Self {
            token: CancellationToken::new(),
            time_start: Instant::now(),
            timeout_dur: None,
        }
    }

    // 修正：tokio_util 的方法是 child()，不是 child_token()
    fn create_child_token(&self) -> CancellationToken {
        self.token.child_token()
    }

    pub fn with_timeout(&self, tmd: Duration) -> Self {
        Self {
            token: self.create_child_token(),
            time_start: Instant::now(), // 子上下文重新计时
            timeout_dur: Some(tmd),
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
            time_start: Instant::now(),
            timeout_dur: Some(tmd),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    pub fn cancel(&self) {
        self.token.cancel();
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
        // 宏会自动 pin fut
        tokio::select! {
            _ = self.cancelled_future() => {
                Err(crate::ioerr("ctx cancel", Some(std::io::ErrorKind::Interrupted)))
            },
            _ = self.timeout_future() => {
                Err(crate::ioerr("ctx timeout", Some(std::io::ErrorKind::TimedOut)))
            },
            v = fut => {
                v
            },
        }
    }
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
