use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

#[derive(Clone)]
pub struct ArcMutBox<T> {
    ptrs: u64,
    inner: Arc<T>,
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
        unsafe { self.inners() }
    }
}
impl<T> ArcMutBox<T> {
    pub fn new(t: T) -> Self {
        let inr = Arc::new(t);
        Self {
            ptrs: (&*inr) as *const T as u64,
            inner: inr,
        }
    }
    pub unsafe fn inners<'a>(&'a self) -> &'a mut T {
        &mut *(self.ptrs as *mut T)
    }
    pub fn ptr(&self)->u64{
      self.ptrs
    }
}
