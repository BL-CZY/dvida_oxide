use core::{
    fmt,
    ops::{Deref, DerefMut},
};

use alloc::boxed::Box;

unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}

#[derive(Debug, Clone)]
pub struct Buffer {
    pub inner: *mut u8,
    pub len: usize,
}

impl Buffer {
    pub fn len(&self) -> usize {
        self.len
    }
}

impl fmt::Display for Buffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // We create a slice to iterate over the bytes.
        // SAFETY: We assume 'inner' and 'len' are valid for the duration of fmt.
        let slice = unsafe { alloc::slice::from_raw_parts(self.inner, self.len) };

        write!(f, "Buffer({} bytes): [", self.len)?;
        for (i, byte) in slice.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            // Prints as 2-digit hex, e.g., "0A"
            write!(f, "{:02X}", byte)?;

            // Limit output so we don't spam the console if the buffer is huge
            if i > 16 && self.len > 32 {
                write!(f, " ...")?;
                break;
            }
        }
        write!(f, "]")
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

impl From<Buffer> for Box<[u8]> {
    fn from(val: Buffer) -> Self {
        unsafe {
            let slice_ptr = alloc::slice::from_raw_parts_mut(val.inner, val.len);
            Box::from_raw(slice_ptr)
        }
    }
}

macro_rules! from_slice {
    ($type:ty) => {
        impl From<&[$type]> for Buffer {
            fn from(value: &[$type]) -> Self {
                let len = value.len() * (size_of::<$type>() / size_of::<u8>());
                let ptr = value.as_ptr();

                Self {
                    inner: ptr as *mut u8,
                    len,
                }
            }
        }
    };
}

macro_rules! from_box {
    ($type:ty) => {
        impl From<Box<[$type]>> for Buffer {
            fn from(value: Box<[$type]>) -> Self {
                let len = value.len() * (size_of::<$type>() / size_of::<u8>());
                let ptr = Box::into_raw(value);

                Self {
                    inner: ptr as *mut u8,
                    len,
                }
            }
        }
    };
}

from_box!(u8);
from_box!(u16);
from_box!(u32);
from_box!(u64);

from_slice!(u8);
from_slice!(u16);
from_slice!(u32);
from_slice!(u64);
