use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

#[derive(Clone)]
pub struct Timer {
    ptr: u64,
    inner: Arc<Inner>,
}

struct Inner {
    dur: Duration,
    tms: SystemTime,
}
impl Timer {
    pub fn new(dur: Duration) -> Self {
        let inr = Arc::new(Inner {
            dur: dur,
            tms: SystemTime::UNIX_EPOCH,
        });

        Self {
            ptr: (&*inr) as *const Inner as u64,
            inner: inr,
        }
    }
    unsafe fn inners<'a>(&'a self) -> &'a mut Inner {
        &mut *(self.ptr as *mut Inner)
    }
    pub fn reset(&self) {
        unsafe { self.inners().tms = SystemTime::now() };
    }
    pub fn tick(&self) -> bool {
        if let Ok(tm) = SystemTime::now().duration_since(self.inner.tms) {
            if tm >= self.inner.dur {
                self.reset();
                return true;
            }
        }
        false
    }
}
