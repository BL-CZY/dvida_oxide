use alloc::sync::Arc;
use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use core::task::Waker;

// Slot state for the ring buffer
#[derive(Debug)]
struct Slot<T> {
    value: UnsafeCell<MaybeUninit<T>>,
    state: AtomicUsize, // 0 = empty, 1 = writing, 2 = ready, 3 = reading
}

impl<T> Slot<T> {
    const fn new() -> Self {
        Self {
            value: UnsafeCell::new(MaybeUninit::uninit()),
            state: AtomicUsize::new(0),
        }
    }
}

#[derive(Debug)]
struct WakerSlot {
    waker: UnsafeCell<MaybeUninit<Waker>>,
    state: AtomicUsize, // 0 = empty, 1 = writing, 2 = ready, 3 = reading
}

impl WakerSlot {
    const fn new() -> Self {
        Self {
            waker: UnsafeCell::new(MaybeUninit::uninit()),
            state: AtomicUsize::new(0),
        }
    }
}

#[derive(Debug)]
struct Channel<T, const CAPACITY: usize> {
    buffer: [Slot<T>; CAPACITY],
    wakers: [WakerSlot; CAPACITY],

    // Ring buffer indices for messages
    write_pos: AtomicUsize,
    read_pos: AtomicUsize,

    // Ring buffer indices for wakers
    waker_write_pos: AtomicUsize,
    waker_read_pos: AtomicUsize,

    sender_count: AtomicUsize,
    closed: AtomicBool,
}

impl<T, const CAPACITY: usize> Channel<T, CAPACITY> {
    fn new() -> Self {
        const fn slot_array<T, const N: usize>() -> [Slot<T>; N] {
            [const { Slot::new() }; N]
        }

        const fn waker_array<const N: usize>() -> [WakerSlot; N] {
            [const { WakerSlot::new() }; N]
        }

        Self {
            buffer: slot_array(),
            wakers: waker_array(),
            write_pos: AtomicUsize::new(0),
            read_pos: AtomicUsize::new(0),
            waker_write_pos: AtomicUsize::new(0),
            waker_read_pos: AtomicUsize::new(0),
            sender_count: AtomicUsize::new(1),
            closed: AtomicBool::new(false),
        }
    }

    fn try_send(&self, msg: T) -> Result<(), T> {
        if self.closed.load(Ordering::Acquire) {
            return Err(msg);
        }

        // Try to claim a write slot
        loop {
            let write = self.write_pos.load(Ordering::Acquire);
            let read = self.read_pos.load(Ordering::Acquire);

            // Check if buffer is full
            if write.wrapping_sub(read) >= CAPACITY {
                return Err(msg);
            }

            let slot = &self.buffer[write % CAPACITY];

            // Try to transition from empty (0) to writing (1)
            if slot
                .state
                .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                // Successfully claimed the slot
                unsafe {
                    (*slot.value.get()).write(msg);
                }

                // Mark as ready
                slot.state.store(2, Ordering::Release);

                // Advance write position
                self.write_pos.fetch_add(1, Ordering::Release);

                // Try to wake a receiver
                self.try_wake_receiver();

                return Ok(());
            }

            // Someone else is writing to this slot, try to advance
            self.write_pos
                .compare_exchange(write, write + 1, Ordering::AcqRel, Ordering::Acquire)
                .ok();
        }
    }

    fn try_recv(&self) -> Option<T> {
        loop {
            let read = self.read_pos.load(Ordering::Acquire);
            let write = self.write_pos.load(Ordering::Acquire);

            // Check if buffer is empty
            if read == write {
                return None;
            }

            let slot = &self.buffer[read % CAPACITY];

            // Try to transition from ready (2) to reading (3)
            if slot
                .state
                .compare_exchange(2, 3, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                // Successfully claimed the slot
                let msg = unsafe { (*slot.value.get()).assume_init_read() };

                // Mark as empty
                slot.state.store(0, Ordering::Release);

                // Advance read position
                self.read_pos.fetch_add(1, Ordering::Release);

                return Some(msg);
            }

            // Slot not ready yet, try next position
            self.read_pos
                .compare_exchange(read, read + 1, Ordering::AcqRel, Ordering::Acquire)
                .ok();
        }
    }

    fn register_waker(&self, waker: &Waker) {
        // Try to store the waker
        let mut attempts = 0;
        loop {
            if attempts >= CAPACITY {
                // Can't store waker, just return
                return;
            }

            let waker_write = self.waker_write_pos.load(Ordering::Acquire);
            let waker_slot = &self.wakers[waker_write % CAPACITY];

            if waker_slot
                .state
                .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                unsafe {
                    (*waker_slot.waker.get()).write(waker.clone());
                }
                waker_slot.state.store(2, Ordering::Release);
                self.waker_write_pos.fetch_add(1, Ordering::Release);
                return;
            }

            self.waker_write_pos
                .compare_exchange(
                    waker_write,
                    waker_write + 1,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .ok();

            attempts += 1;
        }
    }

    fn try_wake_receiver(&self) {
        loop {
            let waker_read = self.waker_read_pos.load(Ordering::Acquire);
            let waker_write = self.waker_write_pos.load(Ordering::Acquire);

            if waker_read == waker_write {
                return; // No wakers
            }

            let waker_slot = &self.wakers[waker_read % CAPACITY];

            if waker_slot
                .state
                .compare_exchange(2, 3, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                let waker = unsafe { (*waker_slot.waker.get()).assume_init_read() };
                waker_slot.state.store(0, Ordering::Release);
                self.waker_read_pos.fetch_add(1, Ordering::Release);
                waker.wake();
                return;
            }

            self.waker_read_pos
                .compare_exchange(
                    waker_read,
                    waker_read + 1,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .ok();
        }
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire) && self.sender_count.load(Ordering::Acquire) == 0
    }
}

impl<T, const CAPACITY: usize> Drop for Channel<T, CAPACITY> {
    fn drop(&mut self) {
        // Clean up any remaining messages
        while let Some(_) = self.try_recv() {}

        // Clean up wakers
        for waker_slot in &self.wakers {
            if waker_slot.state.load(Ordering::Acquire) == 2 {
                unsafe {
                    (*waker_slot.waker.get()).assume_init_drop();
                }
            }
        }
    }
}

unsafe impl<T: Send, const CAPACITY: usize> Send for Channel<T, CAPACITY> {}
unsafe impl<T: Send, const CAPACITY: usize> Sync for Channel<T, CAPACITY> {}

#[derive(Debug)]
pub struct LockFreeSender<T, const CAPACITY: usize> {
    channel: Arc<Channel<T, CAPACITY>>,
}

impl<T, const CAPACITY: usize> Clone for LockFreeSender<T, CAPACITY> {
    fn clone(&self) -> Self {
        self.channel.sender_count.fetch_add(1, Ordering::AcqRel);
        LockFreeSender {
            channel: self.channel.clone(),
        }
    }
}

impl<T, const CAPACITY: usize> Drop for LockFreeSender<T, CAPACITY> {
    fn drop(&mut self) {
        if self.channel.sender_count.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.channel.closed.store(true, Ordering::Release);
            // Wake all receivers
            loop {
                let waker_read = self.channel.waker_read_pos.load(Ordering::Acquire);
                let waker_write = self.channel.waker_write_pos.load(Ordering::Acquire);

                if waker_read == waker_write {
                    break;
                }

                self.channel.try_wake_receiver();
            }
        }
    }
}

impl<T, const CAPACITY: usize> LockFreeSender<T, CAPACITY> {
    pub fn send(&self, msg: T) -> Result<(), T> {
        self.channel.try_send(msg)
    }
}

#[derive(Debug)]
pub struct LockFreeReceiver<T, const CAPACITY: usize> {
    channel: Arc<Channel<T, CAPACITY>>,
}

impl<T, const CAPACITY: usize> LockFreeReceiver<T, CAPACITY> {
    pub fn recv(&self) -> RecvFuture<'_, T, CAPACITY> {
        RecvFuture { rx: self }
    }
}

pub struct RecvFuture<'a, T, const CAPACITY: usize> {
    rx: &'a LockFreeReceiver<T, CAPACITY>,
}

impl<'a, T, const CAPACITY: usize> core::future::Future for RecvFuture<'a, T, CAPACITY> {
    type Output = Option<T>;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        match self.rx.channel.try_recv() {
            Some(msg) => core::task::Poll::Ready(Some(msg)),
            None => {
                if self.rx.channel.is_closed() {
                    return core::task::Poll::Ready(None);
                }
                self.rx.channel.register_waker(cx.waker());

                // Check again after registering to avoid race
                match self.rx.channel.try_recv() {
                    Some(msg) => core::task::Poll::Ready(Some(msg)),
                    None => {
                        if self.rx.channel.is_closed() {
                            core::task::Poll::Ready(None)
                        } else {
                            core::task::Poll::Pending
                        }
                    }
                }
            }
        }
    }
}

pub fn lockfree_channel<T, const CAPACITY: usize>()
-> (LockFreeSender<T, CAPACITY>, LockFreeReceiver<T, CAPACITY>) {
    let channel = Arc::new(Channel::new());
    let tx = LockFreeSender {
        channel: channel.clone(),
    };
    let rx = LockFreeReceiver {
        channel: channel.clone(),
    };
    (tx, rx)
}
