use async_cancellation_token::{CancellationTokenSource, Cancelled};
// 注意：确保你的 Cargo.toml 中有 async-cancel-token = "..."
// 如果包名不同，请调整 import。通常该 crate 导出 CancellationToken。
// 假设 CancelToken 是你封装的类型或者是 crate 导出的类型。
// 这里为了演示，假设 CancelToken 对应 async_cancellation_token::CancellationToken
use async_cancellation_token::CancellationToken as CancelToken;

use async_std::task;
use futures::future::{FutureExt, Pending, Ready};
use futures::select;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};
use std::time::{Duration, Instant};

// 辅助函数
fn ioerr(msg: &str, kind: Option<io::ErrorKind>) -> io::Error {
    io::Error::new(kind.unwrap_or(io::ErrorKind::Other), msg)
}

#[derive(Clone)]
pub struct Context {
    src: CancellationTokenSource,
    token: CancelToken,
    time_start: Instant,
    timeout_dur: Option<Duration>,
}

impl Context {
    pub fn new() -> Self {
        // 需要创建一个源来生成 token，或者直接创建根 token
        // async-cancel-token 通常用法: let source = CancellationTokenSource::new(); let token = source.token();
        // 但如果是简单的 CancellationToken::new() 也可以，取决于具体 crate 版本 API
        // 假设可以直接 new，如果不能，需要在外部管理 Source
        let source = CancellationTokenSource::new();
        Self {
            src: source.clone(),
            token: source.token(),
            time_start: Instant::now(),
            timeout_dur: None,
        }
    }

    fn create_child_token(&self) -> CancelToken {
        self.src.token()
        // self.token.()
        // panic!("unimplemented");
    }

    pub fn new_timeout(tmd: Duration) -> Self {
        let source = CancellationTokenSource::new();
        Self {
            src: source.clone(),
            token: source.token(),
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
            src: self.src.clone(),
            token: self.create_child_token(),
            time_start: Instant::now(),
            timeout_dur: None,
        }
    }

    pub fn child_timeout(&self, tmd: Duration) -> Self {
        Self {
            src: self.src.clone(),
            token: self.create_child_token(),
            time_start: Instant::now(),
            timeout_dur: Some(tmd),
        }
    }

    pub fn cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    pub fn cancel(&self) {
        self.src.cancel();
    }

    pub fn cancelled_future(&self) -> impl Future<Output = ()> + '_ {
        self.token.cancelled()
    }

    /// 修复点：明确类型，处理 Pending 的泛型
    pub fn timeout_future(&self) -> impl Future<Output = ()> + '_ {
        if let Some(dur) = self.timeout_dur {
            let elapsed = self.time_start.elapsed();

            if elapsed >= dur {
                // 时间已过，返回 Ready
                // 显式指定类型以消除歧义
                Either::Left(futures::future::ready(()))
            } else {
                let remaining = dur - elapsed;
                Either::Right(task::sleep(remaining))
            }
        } else {
            // 无超时，返回 Pending<()>
            Either::Pending
        }
    }

    pub async fn wait_futs<F, T>(&self, fut: F) -> io::Result<T>
    where
        F: Future<Output = io::Result<T>>,
    {
        let cancel_fut = self.cancelled_future();
        let timeout_fut = self.timeout_future();
        let mut main_fut = Box::pin(fut.fuse());

        /* select! {
            _ = cancel_fut => {
                Err(ioerr("ctx cancel", Some(io::ErrorKind::Interrupted)))
            },
            _ = timeout_fut => {
                Err(ioerr("ctx timeout", Some(io::ErrorKind::TimedOut)))
            },
            v = main_fut => {
                v
            },
        } */
        panic!("unimplemented");
    }
}

/// 优化后的 Either 枚举
/// 必须持有 Pending 的具体实例，因为 Pending<()> 是一个状态机
enum Either<L, R> {
    Left(L),
    Right(R),
    Pending,
}

impl<L, R, T> Future for Either<L, R>
where
    L: Future<Output = T>,
    R: Future<Output = T>,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<T> {
        // 【关键修复】：安全地将 Pin<&mut Self> 投影到内部字段
        // 我们不能使用 get_mut()，因为这会破坏 Pin 的保证（如果 L 或 R 是 !Unpin）
        // 我们使用 unsafe map_unchecked_mut，因为我们知道我们只是改变了引用的目标，
        // 而没有移动数据，且内部字段的 Pin 状态与外部保持一致。
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
