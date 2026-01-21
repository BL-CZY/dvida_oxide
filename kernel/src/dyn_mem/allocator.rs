use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::NonNull,
};

use crate::ejcineque::sync::spin::SpinMutex;
use crate::log;
use linked_list_allocator::Heap;
use x86_64::instructions::interrupts::without_interrupts;

#[global_allocator]
static ALLOCATOR: HeapAllocator = HeapAllocator::new();

pub fn init_kheap(heap_bottom: *mut u8, heap_size: usize) {
    ALLOCATOR.init(heap_bottom, heap_size);

    log!("Kheap initialization finished");
}

pub struct HeapAllocator {
    heap: SpinMutex<Heap>,
}

impl HeapAllocator {
    pub const fn new() -> Self {
        Self {
            heap: SpinMutex::new_const(Heap::empty()),
        }
    }

    pub fn init(&self, bottom: *mut u8, size: usize) {
        without_interrupts(|| unsafe {
            self.heap.lock().init(bottom, size);
        });
    }
}

unsafe impl GlobalAlloc for HeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        without_interrupts(|| {
            self.heap
                .lock()
                .allocate_first_fit(layout)
                .expect("heap is empty")
                .as_ptr()
        })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        without_interrupts(|| unsafe {
            self.heap
                .lock()
                .deallocate(NonNull::new(ptr).expect("Corrupted metadata"), layout);
        })
    }
}
