use std::{
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Condvar, Mutex,
    },
    time::Duration,
};

use crate::Context;

#[derive(Clone)]
pub struct Waker {
    inner: crate::ArcMut<Inner>,
}

struct Inner {
    ctx: Context,
    lk: Mutex<bool>,
    cond: Condvar,
}

impl Waker {
    pub fn new(ctx: &Context) -> Self {
        Self {
            inner: crate::ArcMut::new(Inner {
                ctx: Context::background(Some(ctx.clone())),
                lk: Mutex::new(false),
                cond: Condvar::new(),
            }),
        }
    }
    pub fn done(&self) -> bool {
        self.inner.ctx.done()
    }
    pub fn close(&self) {
        self.inner.ctx.stop();
        self.inner.cond.notify_all();
    }
    pub fn wait(&self) -> io::Result<()> {
        if let Ok(mut lkv) = self.inner.lk.lock() {
            *lkv = false;
            while !*lkv {
                if self.inner.ctx.done() {
                    return Err(crate::ioerr("ctx is end", None));
                }
                match self.inner.cond.wait(lkv) {
                    Ok(v) => lkv = v,
                    Err(e) => return Err(crate::ioerr("cond wait err", None)),
                };
            }
        } else {
            return Err(crate::ioerr("lock err", None));
        }

        Ok(())
    }
    pub fn wait_timeout(&self, tm: Duration) -> io::Result<()> {
        if self.inner.ctx.done() {
            return Err(crate::ioerr("ctx is end", None));
        }
        if let Ok(mut lkv) = self.inner.lk.lock() {
            *lkv = false;
            if self.inner.ctx.done() {
                return Err(crate::ioerr("ctx is end", None));
            }
            if let Err(e) = self.inner.cond.wait_timeout(lkv, tm) {
                return Err(crate::ioerr("cond wait err", None));
            }
        } else {
            return Err(crate::ioerr("lock err", None));
        }

        Ok(())
    }
    pub fn notify_one(&self) {
        if !self.inner.ctx.done() {
            if let Ok(mut lkv) = self.inner.lk.lock() {
                *lkv = true;
                self.inner.cond.notify_one();
            }
        }
    }
    pub fn notify_all(&self) {
        if !self.inner.ctx.done() {
            if let Ok(mut lkv) = self.inner.lk.lock() {
                *lkv = true;
                self.inner.cond.notify_all();
            }
        }
    }
}
