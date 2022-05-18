use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

pub struct ArcMut<T> {
    ptrs: u64,
    inner: Arc<T>,
}
impl<T> Clone for ArcMut<T> {
    fn clone(&self) -> Self {
        Self {
            ptrs: self.ptrs,
            inner: self.inner.clone(),
        }
    }
}
impl<T> Deref for ArcMut<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &*self.inner
    }
}
impl<T> DerefMut for ArcMut<T> {
    // type Target = T;

    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.muts() }
    }
}
impl<T> ArcMut<T> {
    pub fn new(t: T) -> Self {
        let inr = Arc::new(t);
        Self::from(inr)
        /* Self {
            ptrs: (&*inr) as *const T as u64,
            inner: inr,
        } */
    }
    pub unsafe fn muts<'a>(&'a self) -> &'a mut T {
        &mut *(self.ptrs as *mut T)
    }
    pub fn ptr(&self) -> u64 {
        self.ptrs
    }

    pub fn arc_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }
    pub fn into_raw(&self) -> *const T {
        Arc::into_raw(self.inner.clone())
    }
    pub unsafe fn from_raw(p: *const T) -> Self {
        let inr = Arc::from_raw(p);
        Self::from(inr)
    }
}

impl<T> From<Arc<T>> for ArcMut<T> {
    fn from(inr: Arc<T>) -> Self {
        Self {
            ptrs: (&*inr) as *const T as u64,
            inner: inr,
        }
    }
}

/*
pub struct ArcMutBox<T> {
  ptrs: u64,
  inner: Arc<Box<T>>,
}
impl<T> Clone for ArcMutBox<T> {
  fn clone(&self) -> Self {
      Self {
          ptrs: self.ptrs,
          inner: self.inner.clone(),
      }
  }
}
impl<T> Deref for ArcMutBox<T> {
  type Target = T;

  fn deref(&self) -> &T {
      &self.inner
  }
}
impl<T> DerefMut for ArcMutBox<T> {
  // type Target = T;

  fn deref_mut(&mut self) -> &mut T {
      unsafe { self.muts() }
  }
}
impl<T> ArcMutBox<T> {
  pub fn new(t: T) -> Self {
      let inr = Arc::new(Box::new(t));
      Self {
          ptrs: (&**inr) as *const T as u64,
          inner: inr,
      }
  }
  pub unsafe fn muts<'a>(&'a self) -> &'a mut T {
      &mut *(self.ptrs as *mut T)
  }
  pub fn ptr(&self) -> u64 {
      self.ptrs
  }
} */
