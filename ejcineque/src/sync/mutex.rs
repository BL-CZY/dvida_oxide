use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    ptr::null_mut,
    sync::atomic::AtomicU8,
    task::Waker,
};

use alloc::borrow::ToOwned;
use x86_64::instructions::interrupts::without_interrupts;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, PartialOrd, Ord, Eq)]
enum MutexState {
    Unlocked = 0,
    Locked = 1,
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, PartialOrd, Ord, Eq)]
enum MutexLinkedListState {
    Unlocked = 0,
    Locked = 1,
}

pub struct MutexWakerNode {
    next: *mut MutexWakerNode,
    prev: *mut MutexWakerNode,
    waker: Waker,
}
/// uses a circular linked list for wakers
pub struct Mutex<T> {
    inner: UnsafeCell<T>,

    /// new wakes go here
    wakers_list_head: UnsafeCell<*mut MutexWakerNode>,
    /// wakers get popped from here
    wakers_list_tail: UnsafeCell<*mut MutexWakerNode>,
    wakers_list_state: AtomicU8,

    state: AtomicU8,
}

impl<T> Mutex<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner: inner.into(),
            wakers_list_head: null_mut::<MutexWakerNode>().into(),
            wakers_list_tail: null_mut::<MutexWakerNode>().into(),
            wakers_list_state: AtomicU8::new(MutexLinkedListState::Unlocked as u8),
            state: AtomicU8::new(MutexState::Unlocked as u8),
        }
    }

    fn lock_wakers_list(&self) {
        let mut counter: usize = 1000;
        loop {
            if self
                .wakers_list_state
                .compare_exchange(
                    MutexLinkedListState::Unlocked as u8,
                    MutexLinkedListState::Locked as u8,
                    core::sync::atomic::Ordering::Acquire,
                    core::sync::atomic::Ordering::Relaxed,
                )
                .is_ok()
            {
                break;
            }

            counter -= 1;
            if counter <= 0 {
                panic!("Potential deadlock detected");
            }
        }
    }

    fn unlock_wakers_list(&self) {
        self.wakers_list_state.store(
            MutexLinkedListState::Unlocked as u8,
            core::sync::atomic::Ordering::Release,
        );
    }

    pub fn lock<'a>(&'a self) -> MutexFuture<'a, T> {
        MutexFuture {
            mutex: self,
            node: None,
        }
    }

    pub fn try_lock<'a>(&'a self) -> Option<MutexGuard<'a, T>> {
        if self
            .state
            .compare_exchange(
                MutexState::Unlocked as u8,
                MutexState::Locked as u8,
                core::sync::atomic::Ordering::Acquire,
                core::sync::atomic::Ordering::Relaxed,
            )
            .is_ok()
        {
            return Some(MutexGuard { mutex: self });
        }

        None
    }
}

pub struct MutexFuture<'a, T> {
    mutex: &'a Mutex<T>,
    node: Option<MutexWakerNode>,
}

impl<'a, T> Future for MutexFuture<'a, T> {
    type Output = MutexGuard<'a, T>;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        if let Some(res) = self.mutex.try_lock() {
            return core::task::Poll::Ready(res);
        }

        let this = unsafe { self.get_unchecked_mut() };

        without_interrupts(|| {
            this.mutex.lock_wakers_list();

            if this.node.is_none() {
                let node = MutexWakerNode {
                    next: unsafe { *this.mutex.wakers_list_head.get() },
                    prev: unsafe { *this.mutex.wakers_list_tail.get() },
                    waker: cx.waker().to_owned(),
                };

                this.node = Some(node);

                // now the location of node is constant
                match this.node {
                    Some(ref mut node) => {
                        // if the list is empty
                        if node.next == null_mut() || node.prev == null_mut() {
                            unsafe {
                                node.next = node as *mut MutexWakerNode;
                                node.prev = node as *mut MutexWakerNode;

                                *this.mutex.wakers_list_head.get() = node as *mut MutexWakerNode;
                                *this.mutex.wakers_list_tail.get() = node as *mut MutexWakerNode;
                            }
                        } else {
                            unsafe {
                                node.next = *this.mutex.wakers_list_head.get();
                                *this.mutex.wakers_list_head.get() = node as *mut MutexWakerNode;

                                node.prev = *this.mutex.wakers_list_tail.get();

                                // doesnt use read because it will create a new copy
                                (*node.next).prev = node as *mut MutexWakerNode;
                                (*node.prev).next = node as *mut MutexWakerNode;
                            }
                        }
                    }
                    None => {}
                }
            }

            this.mutex.unlock_wakers_list();
        });

        core::task::Poll::Pending
    }
}

pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.inner.get() }
    }
}

impl<'a, T> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.inner.get() }
    }
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.mutex.state.store(
            MutexState::Unlocked as u8,
            core::sync::atomic::Ordering::Release,
        );

        without_interrupts(|| {
            self.mutex.lock_wakers_list();

            unsafe {
                let tail_ptr_ptr = self.mutex.wakers_list_tail.get();
                let head_ptr_ptr = self.mutex.wakers_list_head.get();

                if *tail_ptr_ptr != null_mut() && *head_ptr_ptr != null_mut() {
                    let node = *self.mutex.wakers_list_tail.get();

                    if (*node).prev == node {
                        *self.mutex.wakers_list_head.get() = null_mut::<MutexWakerNode>().into();
                        *self.mutex.wakers_list_tail.get() = null_mut::<MutexWakerNode>().into();
                    } else {
                        *self.mutex.wakers_list_tail.get() = (*node).prev;
                        (*(*node).prev).next = *self.mutex.wakers_list_head.get();
                        (*(*self.mutex.wakers_list_head.get())).prev = (*node).prev;
                    }

                    (*node).waker.wake_by_ref();
                }
            }

            self.mutex.unlock_wakers_list();
        });
    }
}
