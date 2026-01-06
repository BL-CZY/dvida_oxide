use crate::sync::spin::SpinMutex;
use core::{cell::UnsafeCell, task::Waker};

pub struct SpscCell<T> {
    inner: SpinMutex<Option<T>>,
    waker: SpinMutex<Option<Waker>>,
}

pub struct SpscCellGetter<T> {
    cell: UnsafeCell<SpscCell<T>>,
}

impl<'a, T> SpscCellGetter<T> {
    pub fn get(self) -> SpscCellGetFuture<T> {
        SpscCellGetFuture { cell: self.cell }
    }
}

pub struct SpscCellSetter<T> {
    cell: UnsafeCell<SpscCell<T>>,
}

impl<T> SpscCellSetter<T> {
    pub fn set(self, value: T) {
        let cell = self.cell.get();
        *unsafe { (*cell).inner.lock() } = Some(value);

        if let Some(ref waker) = *unsafe { (*cell).waker.lock() } {
            waker.wake_by_ref();
        }
    }
}

pub struct SpscCellGetFuture<T> {
    cell: UnsafeCell<SpscCell<T>>,
}

impl<T> Future for SpscCellGetFuture<T> {
    type Output = T;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let cell = self.cell.get();
        let mut value = unsafe { (*cell).inner.lock() };
        match *value {
            Some(_) => {
                let res = value.take().unwrap();
                core::task::Poll::Ready(res)
            }
            None => {
                let mut guard = unsafe { (*cell).waker.lock() };
                *guard = Some(cx.waker().clone());
                core::task::Poll::Pending
            }
        }
    }
}
