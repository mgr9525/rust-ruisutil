use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

pub struct ArcMutBox<T> {
    ptrs: u64,
    inner: Arc<T>,
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
        let inr = Arc::new(t);
        Self {
            ptrs: (&*inr) as *const T as u64,
            inner: inr,
        }
    }
    pub unsafe fn muts<'a>(&'a self) -> &'a mut T {
        &mut *(self.ptrs as *mut T)
    }
    pub fn ptr(&self) -> u64 {
        self.ptrs
    }
}
