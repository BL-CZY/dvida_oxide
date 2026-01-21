use alloc::vec::Vec;
use core::task::Waker;

use crate::ejcineque::sync::spin::SpinMutex;
use lazy_static::lazy_static;
// use spin::Mutex;

// lazy_static! {
//     pub static ref PRIMARY_IDE_WAKERS: Mutex<Vec<Waker>> = Mutex::new(Vec::new());
//     pub static ref SECONDARY_IDE_WAKERS: Mutex<Vec<Waker>> = Mutex::new(Vec::new());
//     pub static ref TIMER_WAKERS: Mutex<Vec<Waker>> = Mutex::new(Vec::new());
// }

lazy_static! {
    pub static ref PRIMARY_IDE_WAKERS: SpinMutex<Vec<Waker>> = SpinMutex::new(Vec::new());
    pub static ref SECONDARY_IDE_WAKERS: SpinMutex<Vec<Waker>> = SpinMutex::new(Vec::new());
    pub static ref TIMER_WAKERS: SpinMutex<Vec<Waker>> = SpinMutex::new(Vec::new());
}
