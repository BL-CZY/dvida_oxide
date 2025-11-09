use core::task::Waker;

use alloc::{collections::vec_deque::VecDeque, sync::Arc};
use spin::Mutex;

#[derive(Default)]
struct UnboundedChannel<T> {
    buffer: VecDeque<T>,
    rx_wakers: VecDeque<Waker>,
    sender_count: u64,
}

pub struct UnboundedSender<T> {
    channel: Arc<Mutex<UnboundedChannel<T>>>,
}

impl<T> Clone for UnboundedSender<T> {
    fn clone(&self) -> Self {
        self.channel.lock().sender_count += 1;

        UnboundedSender {
            channel: self.channel.clone(),
        }
    }
}

impl<T> Drop for UnboundedSender<T> {
    fn drop(&mut self) {
        self.channel.lock().sender_count -= 1;
    }
}

impl<T> UnboundedSender<T> {
    pub fn send(&self, msg: T) {
        // get guard and push message
        let mut channel_guard = self.channel.lock();
        channel_guard.buffer.push_back(msg);

        // wake up the waker and do nothing if there isn't any
        if let Some(waker) = channel_guard.rx_wakers.pop_front() {
            waker.wake();
        }
    }
}

pub struct UnboundedReceiver<T> {
    channel: Arc<Mutex<UnboundedChannel<T>>>,
}

impl<T> UnboundedReceiver<T> {
    pub fn recv(&self) -> RecvFuture<'_, T> {
        // '_ will explicitly ask the compiler to infer the
        // lifetime here
        return RecvFuture { rx: self };
    }
}

pub struct RecvFuture<'a, T> {
    rx: &'a UnboundedReceiver<T>,
}

impl<'a, T> Future for RecvFuture<'a, T> {
    type Output = Option<T>;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let mut guard = self.rx.channel.lock();

        match guard.buffer.pop_front() {
            Some(msg) => core::task::Poll::Ready(Some(msg)),
            None => {
                // push the waker
                if guard.sender_count == 0 {
                    return core::task::Poll::Ready(None);
                }

                guard.rx_wakers.push_back(cx.waker().clone());
                core::task::Poll::Pending
            }
        }
    }
}

pub fn unbounded_channel<T>() -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    let channel: Arc<Mutex<UnboundedChannel<T>>> = Arc::new(Mutex::new(UnboundedChannel {
        sender_count: 1,
        buffer: VecDeque::new(),
        rx_wakers: VecDeque::new(),
    }));

    let tx = UnboundedSender {
        channel: channel.clone(),
    };

    let rx = UnboundedReceiver {
        channel: channel.clone(),
    };

    (tx, rx)
}
