use std::{
    sync::atomic::AtomicU64,
    time::{Duration, Instant},
};

use crate::ArcMut;

#[derive(Clone)]
pub struct Timer {
    inner: ArcMut<Inner>,
}

struct Inner {
    start_tm: Instant,
    dur: AtomicU64,
    tms: AtomicU64,
}
impl Timer {
    pub fn new(dur: Duration) -> Self {
        Self {
            inner: ArcMut::new(Inner {
                start_tm: Instant::now(),
                dur: AtomicU64::new(dur.as_nanos() as u64),
                tms: AtomicU64::new(0),
            }),
        }
    }
    pub fn reset(&self) {
        let dur = self.inner.start_tm.elapsed();
        self.inner
            .tms
            .store(dur.as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn reinit(&self) {
        self.inner
            .tms
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn set_dur(&self, dur: Duration) {
        self.inner
            .dur
            .store(dur.as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn get_dur(&self) -> Duration {
        Duration::from_nanos(self.inner.dur.load(std::sync::atomic::Ordering::Relaxed))
    }
    pub fn tick(&self) -> bool {
        if self.tmout() {
            self.reset();
            return true;
        }
        false
    }

    pub fn tmout(&self) -> bool {
        let tms = self.inner.tms.load(std::sync::atomic::Ordering::Relaxed);
        if tms <= 0 {
            return true;
        }
        let tmsd = self.inner.start_tm + Duration::from_nanos(tms);
        let tmx = Instant::now().duration_since(tmsd);
        if tmx >= self.get_dur() {
            return true;
        }
        false
    }

    pub fn tmdur(&self) -> Duration {
        let tms = self.inner.tms.load(std::sync::atomic::Ordering::Relaxed);
        if tms <= 0 {
            return Duration::ZERO;
        }
        let tmsd = self.inner.start_tm + Duration::from_nanos(tms);
        Instant::now().duration_since(tmsd)
    }
}
