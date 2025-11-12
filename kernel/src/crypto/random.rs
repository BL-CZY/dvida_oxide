use ejcineque::sync::mpsc::unbounded::{UnboundedSender, unbounded_channel};
use once_cell_no_std::OnceCell;
use terminal::log;

use crate::time::Rtc;

/// Algorithm adapted from https://en.wikipedia.org/wiki/Mersenne_Twister 11/12/2025

pub static RANDOM_SENDER: OnceCell<UnboundedSender<UnboundedSender<u32>>> = OnceCell::new();

/// we are using Mersenne Twister here, or MT19937

pub const W: usize = 32;
pub const N: usize = 624;
pub const M: usize = 32;
pub const R: usize = 31;

pub const A: u32 = 0x9908B0DF;

pub const U: usize = 11;
pub const D: usize = 0xFFFFFFFF;

pub const S: u32 = 7;
pub const B: u32 = 0x9D2C5680;

pub const T: usize = 15;
pub const C: u32 = 0xEFC60000;

pub const L: usize = 18;
pub const F: u32 = 1812433253;

struct RandState {
    state_array: [u32; N],
    index: isize,
}

fn init() -> RandState {
    let mut res = RandState {
        state_array: [0; N],
        index: 0,
    };

    let mut seed = (Rtc::datetime_to_unix_timestamp(
        &Rtc::new()
            .read_datetime()
            .expect("Cannot get current time as seed for random"),
    ) & 0xFFFFFFFF) as u32;

    for i in 0..N {
        res.state_array[i] = seed;
        // Knuth TAOCP Vol2. 3rd Ed. P.106 for multiplier.
        seed = F * (seed ^ (seed >> (W - 2))) + i as u32;
    }

    return res;
}

fn random_u32(state: &mut RandState) -> u32 {
    let mut k = state.index;
    if k < 0 {
        k += N as isize;
    }

    let mut j = k - (N as isize - 1);
    if j < 0 {
        j += N as isize;
    }

    let mut x = state.state_array[k as usize] & 0xFFFF0000 | state.state_array[j as usize] & 0xFFFF;

    let mut x_a = x >> 1;
    if x & 0x1 == 0x1 {
        x_a ^= A;
    }

    j = k - (N - M) as isize;
    if j < 0 {
        j += N as isize;
    }

    // compute the next value
    x = state.state_array[j as usize] ^ x_a;
    state.state_array[k as usize + 1] = x;

    if k >= N as isize {
        k = 0;
    }
    state.index = k;

    let mut y = x ^ (x >> U);
    y = y ^ ((y << S) & B);
    y = y ^ ((y << T) & C);

    y ^ (y >> L)
}

pub async fn run_random() {
    let mut state = init();
    let (tx, rx) = unbounded_channel::<UnboundedSender<u32>>();

    let _ = RANDOM_SENDER
        .set(tx.clone())
        .expect("Cannot set global random sender");

    log!("Random initialization complete");

    while let Some(sender) = rx.recv().await {
        let res = random_u32(&mut state);
        sender.send(res);
    }
}

pub async fn random_number() -> u32 {
    let sender = RANDOM_SENDER.get().expect("No Sender found").clone();

    let (tx, rx) = unbounded_channel::<u32>();

    sender.send(tx);

    if let Some(num) = rx.recv().await {
        num
    } else {
        0
    }
}
