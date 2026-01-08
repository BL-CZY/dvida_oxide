use core::ops::{Deref, DerefMut};

use alloc::boxed::Box;

unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}

#[derive(Debug)]
pub struct Buffer {
    inner: *mut u8,
    len: usize,
}

impl Buffer {
    pub fn len(&self) -> usize {
        self.len
    }
}

impl Deref for Buffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { alloc::slice::from_raw_parts_mut(self.inner, self.len) }
    }
}

impl DerefMut for Buffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { alloc::slice::from_raw_parts_mut(self.inner, self.len) }
    }
}

impl From<Box<[u8]>> for Buffer {
    fn from(value: Box<[u8]>) -> Self {
        let len = value.len();
        let ptr = Box::into_raw(value);

        Self {
            inner: ptr as *mut u8,
            len,
        }
    }
}

impl Into<Box<[u8]>> for Buffer {
    fn into(self) -> Box<[u8]> {
        unsafe {
            let slice_ptr = alloc::slice::from_raw_parts_mut(self.inner, self.len);
            Box::from_raw(slice_ptr)
        }
    }
}
