use core::pin::Pin;

use alloc::{boxed::Box, vec, vec::Vec};

#[allow(unused)]
fn foo() {}

pub struct MultiRace<T> {
    futures: Vec<Pin<Box<dyn Future<Output = T>>>>,
}

impl<T> Default for MultiRace<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> MultiRace<T> {
    pub fn new() -> Self {
        MultiRace { futures: vec![] }
    }

    pub fn add(mut self, future: impl Future<Output = T> + 'static) -> Self {
        self.futures.push(Box::pin(future));
        self
    }
}

impl<T> Future for MultiRace<T> {
    type Output = T;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        for future in self.futures.iter_mut() {
            match future.as_mut().poll(cx) {
                core::task::Poll::Ready(res) => return core::task::Poll::Ready(res),
                core::task::Poll::Pending => continue,
            }
        }

        core::task::Poll::Pending
    }
}
