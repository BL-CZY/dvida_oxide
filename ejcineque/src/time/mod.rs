use core::task::Poll;

use crate::wakers::TIMER_WAKERS;

unsafe impl Send for SleepFuture {}
unsafe impl Sync for SleepFuture {}

pub struct SleepFuture {
    tick_count: u32,
}

impl Future for SleepFuture {
    type Output = ();

    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        self.tick_count -= 1;

        if self.tick_count <= 0 {
            Poll::Ready(())
        } else {
            x86_64::instructions::interrupts::without_interrupts(|| {
                TIMER_WAKERS.lock().push(cx.waker().clone());
            });
            Poll::Pending
        }
    }
}

fn wait_int(tick_count: u32) -> SleepFuture {
    SleepFuture {
        tick_count: tick_count + 1,
    }
}

pub async fn wait(tick_count: u32) {
    wait_int(tick_count).await;
}
