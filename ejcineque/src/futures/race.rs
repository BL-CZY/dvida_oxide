use core::pin::Pin;

use alloc::boxed::Box;

pub enum Either<T, D> {
    Left(T),
    Right(D),
}

pub struct Race<T, D> {
    left_future: Pin<Box<dyn Future<Output = T>>>,
    right_future: Pin<Box<dyn Future<Output = D>>>,
}

impl<T, D> Future for Race<T, D> {
    type Output = Either<T, D>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        match self.left_future.as_mut().poll(cx) {
            core::task::Poll::Pending => {}
            core::task::Poll::Ready(res) => return core::task::Poll::Ready(Either::Left(res)),
        }

        match self.right_future.as_mut().poll(cx) {
            core::task::Poll::Pending => {}
            core::task::Poll::Ready(res) => return core::task::Poll::Ready(Either::Right(res)),
        }

        core::task::Poll::Pending
    }
}

pub async fn race<T, D>(
    left: impl Future<Output = T> + 'static,
    right: impl Future<Output = D> + 'static,
) -> Either<T, D> {
    let race = Race {
        left_future: Box::pin(left),
        right_future: Box::pin(right),
    };

    race.await
}
