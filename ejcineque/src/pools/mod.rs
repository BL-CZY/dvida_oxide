use crate::sync::spin::SpinMutex;
use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, AtomicU64},
};

use lazy_static::lazy_static;

lazy_static! {
    /// never used in IRQs and ISRs
    pub static ref DISK_IO_BUFFER_POOL: DiskIOBufferPool = DiskIOBufferPool {
        inner: SpinMutex::new(InnerDiskIOBufferPool::new())
    };
}

unsafe impl Send for DiskIOBufferPool {}
unsafe impl Sync for DiskIOBufferPool {}

pub struct DiskIOBufferPool {
    inner: SpinMutex<InnerDiskIOBufferPool>,
}

pub struct InnerDiskIOBufferPool {
    sector_pool: UnsafeCell<[[u8; 512]; 64]>,
    block_pool: UnsafeCell<[[u8; 1024]; 64]>,
    sector_pool_mask: u64,
    block_pool_mask: u64,
    is_locked: AtomicBool,
}

impl InnerDiskIOBufferPool {
    pub fn new() -> Self {
        InnerDiskIOBufferPool {
            sector_pool: [[0u8; 512]; 64].into(),
            block_pool: [[0u8; 1024]; 64].into(),
            sector_pool_mask: 0,
            block_pool_mask: 0,
            is_locked: AtomicBool::new(false),
        }
    }

    pub fn get_block_buf(&mut self) -> DiskIOBuffer<'_> {
        todo!()
    }

    pub fn get_sector_buf(&mut self) -> DiskIOBuffer<'_> {
        for i in 0..64 {
            if self.sector_pool_mask & (1 << i) == 0 {
                self.sector_pool_mask = self.sector_pool_mask | (1 << i);
            }
        }
        todo!()
    }
}

pub struct DiskIOBuffer<'a> {
    pub inner: Buffer<'a>,
}

unsafe impl<'a> Send for Buffer<'a> {}
unsafe impl<'a> Sync for Buffer<'a> {}

pub struct Buffer<'a>(pub UnsafeCell<&'a mut [u8]>);

impl<'a> Deref for Buffer<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { *self.0.get() }
    }
}

impl<'a> DerefMut for Buffer<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { *self.0.get() }
    }
}

impl<'a> Clone for Buffer<'a> {
    fn clone(&self) -> Self {
        Self(unsafe { (*self.0.get()).into() })
    }
}
