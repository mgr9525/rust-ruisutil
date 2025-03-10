use std::time::{Duration, Instant};

use crate::ArcMut;

#[derive(Clone)]
pub struct Timer {
    inner: ArcMut<Inner>,
}

struct Inner {
    dur: Duration,
    tms: Option<Instant>,
}
impl Timer {
    pub fn new(dur: Duration) -> Self {
        Self {
            inner: ArcMut::new(Inner {
                dur: dur,
                tms: None,
            }),
        }
    }
    pub fn reset(&self) {
        unsafe { self.inner.muts().tms = Some(Instant::now()) };
    }
    pub fn reinit(&self) {
        unsafe { self.inner.muts().tms = None };
    }
    pub fn tick(&self) -> bool {
        if self.tmout() {
            self.reset();
            return true;
        }
        false
    }

    pub fn tmout(&self) -> bool {
        let tms = match self.inner.tms {
            Some(tms) => tms,
            None => return true,
        };
        let tmx = Instant::now().duration_since(tms);
        if tmx >= self.inner.dur {
            return true;
        }
        false
    }

    pub fn tmdur(&self) -> Duration {
        match self.inner.tms {
            Some(tms) => Instant::now().duration_since(tms),
            None => Duration::ZERO,
        }
    }
}
