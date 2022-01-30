use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use crate::ArcMut;

#[derive(Clone)]
pub struct Timer {
    inner: ArcMut<Inner>,
}

struct Inner {
    dur: Duration,
    tms: SystemTime,
}
impl Timer {
    pub fn new(dur: Duration) -> Self {
        Self {
            inner: ArcMut::new(Inner {
              dur: dur,
              tms: SystemTime::UNIX_EPOCH,
          }),
        }
    }
    pub fn reset(&self) {
        unsafe { self.inner.muts().tms = SystemTime::now() };
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

    pub fn tmout(&self)->bool{
      if let Ok(tm) = SystemTime::now().duration_since(self.inner.tms) {
          if tm >= self.inner.dur {
              return true;
          }
      }
      false
    }
}
