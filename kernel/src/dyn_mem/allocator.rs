use linked_list_allocator::LockedHeap;
use terminal::log;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub fn init_kheap(heap_bottom: *mut u8, heap_size: usize) {
    unsafe {
        ALLOCATOR.lock().init(heap_bottom, heap_size);
    }

    log!("Kheap initialization finished");
}
