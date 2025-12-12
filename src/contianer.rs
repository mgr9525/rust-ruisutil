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
impl<T> PartialEq for ArcMut<T> {
    fn eq(&self, other: &Self) -> bool {
        self.ptrs == other.ptrs
    }
}
impl<T> Deref for ArcMut<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
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
    /* pub unsafe fn into_raws(&self) -> *const T {
        let ptr = Arc::into_raw(self.inner.clone());
        Arc::increment_strong_count(ptr);
        ptr
    } */
    pub unsafe fn from_raw(p: *const T) -> std::io::Result<Self> {
        if p.is_null() {
            Err(crate::ioerr("ptr is null", None))
        } else {
            let ac = Arc::from_raw(p);
            Ok(Self::from(ac))
        }
    }
    pub unsafe fn from_raws(p: *const T) -> std::io::Result<Self> {
        if p.is_null() {
            Err(crate::ioerr("ptr is null", None))
        } else {
            Arc::increment_strong_count(p);
            let ac = Arc::from_raw(p);
            Ok(Self::from(ac))
        }
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