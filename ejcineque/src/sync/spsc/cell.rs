use core::{cell::RefCell, task::Waker};

pub struct SpscCell<T> {
    inner: RefCell<Option<T>>,
    waker: RefCell<Option<Waker>>,
}

pub struct SpscCellGetter<T> {
    cell: RefCell<SpscCell<T>>,
}

impl<'a, T> SpscCellGetter<'a, T> {
    pub fn get(&self) -> SpscCellGetFuture<'a, T> {
        SpscCellGetFuture { cell: self.cell }
    }
}

pub struct SpscCellSetter<'a, T> {
    cell: &'a SpscCell<T>,
}

impl<'a, T> SpscCellSetter<'a, T> {
    pub fn set(&self, value: T) {
        *self.cell.inner.borrow_mut() = Some(value);
        if let Some(ref waker) = *self.cell.waker.borrow() {
            waker.wake_by_ref();
        }
    }
}

pub struct SpscCellGetFuture<'a, T> {
    cell: &'a SpscCell<T>,
}

impl<'a, T> Future for SpscCellGetFuture<'a, T> {
    type Output = T;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let mut value = self.cell.inner.borrow_mut();
        match *value {
            Some(_) => {
                let res = value.take().unwrap();
                core::task::Poll::Ready(res)
            }
            None => {
                *self.cell.waker.borrow_mut() = Some(cx.waker().clone());
                core::task::Poll::Pending
            }
        }
    }
}
