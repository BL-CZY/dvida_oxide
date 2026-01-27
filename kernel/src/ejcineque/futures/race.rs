use core::pin::Pin;

use crate::log;

pub enum Either<T, D>
where
    T: Send + Sync,
    D: Send + Sync,
{
    Left(T),
    Right(D),
}

pub struct Race<'a, T, D>
where
    T: Send + Sync,
    D: Send + Sync,
{
    // Box<dyn trait> asks for static by default
    left_future: Pin<&'a mut (dyn Future<Output = T> + Send + Sync)>,
    right_future: Pin<&'a mut (dyn Future<Output = D> + Send + Sync)>,
}

impl<'a, T, D> Future for Race<'a, T, D>
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
            core::task::Poll::Ready(res) => {
                log!("polled 1");
                return core::task::Poll::Ready(Either::Left(res));
            }
        }

        match self.right_future.as_mut().poll(cx) {
            core::task::Poll::Pending => {}
            core::task::Poll::Ready(res) => {
                log!("polled 2");
                return core::task::Poll::Ready(Either::Right(res));
            }
        }

        core::task::Poll::Pending
    }
}

#[macro_export]
/// this macro takes in a list of variables and does the following things:
/// 1. fixes them on the stack of the current function call
/// 2. converts them into a Pin
macro_rules! pin_mut {
    ($($x:ident),* $(,)?) => {
        $(
            // now this value is fixed on the stack
            let mut $x = $x;
            #[allow(unused_mut)]
            let mut $x = unsafe {
                core::pin::Pin::new_unchecked(&mut $x)
            };
        )*
    };
}

pub async fn race<'futures, T, D>(
    left: impl Future<Output = T> + Send + Sync + 'futures,
    right: impl Future<Output = D> + Send + Sync + 'futures,
) -> Either<T, D>
where
    T: Send + Sync,
    D: Send + Sync,
{
    // pinning it here won't be an issue because local variables are stored in local fields after
    // compiling this function into a struct
    pin_mut!(left, right);

    let race = Race {
        left_future: left,
        right_future: right,
    };

    race.await
}
