use core::{alloc::Layout, sync::atomic::AtomicU64};

use lazy_static::lazy_static;
use x86_64::structures::paging::FrameAllocator;

use crate::{
    arch::x86_64::memory::{frame_allocator::FRAME_ALLOCATOR, get_hhdm_offset},
    hal::buffer::Buffer,
};

pub const PAGE_SIZE: usize = 4096;
pub const SECTOR_SIZE: usize = 512;

lazy_static! {
    pub static ref DISK_IO_BUFFER_POOL_PAGE_SIZE: DiskIOBufferPool<PAGE_SIZE> =
        DiskIOBufferPool::new();
    pub static ref DISK_IO_BUFFER_POOL_SECTOR_SIZE: DiskIOBufferPool<SECTOR_SIZE> =
        DiskIOBufferPool::new();
}

pub struct DiskIOBufferPool<const N: usize> {
    buffers: [u64; 64],
    mask: AtomicU64,
}

impl<const N: usize> Default for DiskIOBufferPool<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> DiskIOBufferPool<N> {
    const SIZE: usize = N;

    pub fn new() -> Self {
        assert!(PAGE_SIZE.is_multiple_of(N));
        assert!(N <= PAGE_SIZE);
        assert!(N.is_power_of_two());

        let mut frame_allocator = FRAME_ALLOCATOR
            .get()
            .expect("Failed to get frame allocator")
            .spin_acquire_lock();

        let bytes_count = Self::SIZE * 64;
        let frame_count = bytes_count.div_ceil(PAGE_SIZE);

        let mut buffers = [0u64; 64];

        let mut idx = 0;
        for _ in 0..frame_count {
            let frame = frame_allocator
                .allocate_frame(&mut None)
                .expect("No frame left");

            let addr = get_hhdm_offset().as_u64() + frame.start_address().as_u64();

            for i in 0..PAGE_SIZE / Self::SIZE {
                if idx >= 64 {
                    break;
                }

                buffers[idx] = addr + (i * Self::SIZE) as u64;

                idx += 1;
            }
        }

        Self {
            buffers,
            mask: AtomicU64::new(0),
        }
    }

    pub fn get_buffer(&'static self) -> DiskIOBufferPoolHandle<N> {
        let mut result: Option<u8> = None;
        let _ = self.mask.fetch_update(
            core::sync::atomic::Ordering::AcqRel,
            core::sync::atomic::Ordering::Acquire,
            |val| {
                let i = val.trailing_ones() as u8;
                if i < 64 {
                    result = Some(i);
                    Some(val | 0x1 << i)
                } else {
                    Some(val)
                }
            },
        );

        let inner = match result {
            Some(idx) => self.buffers[idx as usize],
            None => {
                unsafe {
                    // if the buffer pool is full allocate a new one
                    // used unsafe since the assert in new already checked
                    let layout = Layout::from_size_align_unchecked(N, N);
                    
                    alloc::alloc::alloc(layout) as u64
                }
            }
        };

        DiskIOBufferPoolHandle {
            pool: self,
            idx: result,
            inner,
        }
    }
}

pub struct DiskIOBufferPoolHandle<const N: usize> {
    pool: &'static DiskIOBufferPool<N>,
    idx: Option<u8>,
    inner: u64,
}

impl<const N: usize> DiskIOBufferPoolHandle<N> {
    pub fn get_buffer(&self) -> Buffer {
        Buffer {
            inner: self.inner as *mut u8,
            len: N,
        }
    }
}

impl<const N: usize> Drop for DiskIOBufferPoolHandle<N> {
    fn drop(&mut self) {
        if let Some(idx) = self.idx {
            let _ = self.pool.mask.fetch_update(
                core::sync::atomic::Ordering::AcqRel,
                core::sync::atomic::Ordering::Acquire,
                |val| Some(val & !(0x1 << idx)),
            );
        } else {
            // used unsafe because in buffer pools' new it's already checked
            unsafe {
                alloc::alloc::dealloc(
                    self.inner as *mut u8,
                    alloc::alloc::Layout::from_size_align_unchecked(N, N),
                );
            }
        }
    }
}
