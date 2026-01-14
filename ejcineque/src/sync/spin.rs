use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::AtomicBool,
};

unsafe impl<T> Send for SpinMutex<T> {}
unsafe impl<T> Sync for SpinMutex<T> {}

pub struct SpinMutex<T> {
    inner: UnsafeCell<T>,
    is_locked: AtomicBool,
}

impl<T> SpinMutex<T> {
    pub fn new(val: T) -> Self {
        SpinMutex {
            inner: UnsafeCell::new(val),
            is_locked: AtomicBool::new(false),
        }
    }

    pub const fn new_const(val: T) -> Self {
        SpinMutex {
            inner: UnsafeCell::new(val),
            is_locked: AtomicBool::new(false),
        }
    }

    pub fn lock<'a>(&'a self) -> SpinMutexGuard<'a, T> {
        let mut count = 10_000_000;

        while self.is_locked.load(core::sync::atomic::Ordering::Relaxed)
            || self
                .is_locked
                .compare_exchange(
                    false,
                    true,
                    core::sync::atomic::Ordering::Acquire,
                    core::sync::atomic::Ordering::Relaxed,
                )
                .is_err()
        {
            count -= 1;
            if count == 0 {
                panic!("potential deadlock detected!")
            }

            core::hint::spin_loop();
        }

        SpinMutexGuard { mutex: self }
    }
}

pub struct SpinMutexGuard<'a, T> {
    mutex: &'a SpinMutex<T>,
}

impl<'a, T> Drop for SpinMutexGuard<'a, T> {
    fn drop(&mut self) {
        self.mutex
            .is_locked
            .store(false, core::sync::atomic::Ordering::Release);
    }
}

impl<'a, T> DerefMut for SpinMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.inner.get() }
    }
}

impl<'a, T> Deref for SpinMutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.inner.get() }
    }
}
