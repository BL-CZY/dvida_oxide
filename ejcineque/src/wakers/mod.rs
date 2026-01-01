use alloc::vec::Vec;
use core::task::Waker;

use crate::sync::spin::SpinMutex;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref PRIMARY_IDE_WAKERS: SpinMutex<Vec<Waker>> = SpinMutex::new(Vec::new());
    pub static ref SECONDARY_IDE_WAKERS: SpinMutex<Vec<Waker>> = SpinMutex::new(Vec::new());
    pub static ref TIMER_WAKERS: SpinMutex<Vec<Waker>> = SpinMutex::new(Vec::new());
}
