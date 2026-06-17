
use parking_lot::RwLock;

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