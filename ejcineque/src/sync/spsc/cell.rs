use alloc::sync::Arc;

use crate::sync::spin::SpinMutex;
use core::task::Waker;

pub struct SpscCell<T> {
    inner: Option<T>,
    waker: Option<Waker>,
}

pub struct SpscCellGetter<T> {
    cell: Arc<SpinMutex<SpscCell<T>>>,
}

impl<'a, T> SpscCellGetter<T> {
    pub fn get(self) -> SpscCellGetFuture<T> {
        SpscCellGetFuture { cell: self.cell }
    }
}

pub struct SpscCellSetter<T> {
    cell: Arc<SpinMutex<SpscCell<T>>>,
}

impl<T> SpscCellSetter<T> {
    pub fn set(self, value: T) {
        let mut cell = self.cell.lock();
        cell.inner = Some(value);

        if let Some(ref waker) = cell.waker {
            waker.wake_by_ref();
        }
    }
}

pub struct SpscCellGetFuture<T> {
    cell: Arc<SpinMutex<SpscCell<T>>>,
}

impl<T> Future for SpscCellGetFuture<T> {
    type Output = T;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let mut cell = self.cell.lock();
        let value = &mut cell.inner;
        match value {
            Some(_) => {
                let res = value.take().unwrap();
                core::task::Poll::Ready(res)
            }
            None => {
                self.cell.lock().waker = Some(cx.waker().clone());
                core::task::Poll::Pending
            }
        }
    }
}

pub fn spsc_cells<T>() -> (SpscCellGetter<T>, SpscCellSetter<T>) {
    let cell: Arc<SpinMutex<SpscCell<T>>> = Arc::new(SpinMutex::new(SpscCell {
        inner: None,
        waker: None,
    }));

    (
        SpscCellGetter { cell: cell.clone() },
        SpscCellSetter { cell: cell.clone() },
    )
}
