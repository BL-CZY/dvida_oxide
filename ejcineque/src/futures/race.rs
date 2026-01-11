use core::pin::Pin;

use alloc::boxed::Box;

pub enum Either<T, D>
where
    T: Send + Sync,
    D: Send + Sync,
{
    Left(T),
    Right(D),
}

pub struct Race<T, D>
where
    T: Send + Sync,
    D: Send + Sync,
{
    left_future: Pin<Box<dyn Future<Output = T> + Send + Sync>>,
    right_future: Pin<Box<dyn Future<Output = D> + Send + Sync>>,
}

impl<T, D> Future for Race<T, D>
where
    T: Send + Sync,
    D: Send + Sync,
{
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
    left: impl Future<Output = T> + 'static + Send + Sync,
    right: impl Future<Output = D> + 'static + Send + Sync,
) -> Either<T, D>
where
    T: Send + Sync,
    D: Send + Sync,
{
    let race = Race {
        left_future: Box::pin(left),
        right_future: Box::pin(right),
    };

    race.await
}
