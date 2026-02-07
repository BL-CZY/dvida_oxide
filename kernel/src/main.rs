#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![feature(iter_array_chunks)]
#![test_runner(crate::terminal::test::run_tests)]
#![reexport_test_harness_main = "test_main"]
use core::{arch::asm, sync::atomic::AtomicBool};

use once_cell_no_std::OnceCell;

#[cfg(target_arch = "x86_64")]
extern crate alloc;
use limine::{
    BaseRevision,
    request::{RequestsEndMarker, RequestsStartMarker},
};

use limine::request::StackSizeRequest;

#[cfg(target_arch = "x86_64")]
use crate::ejcineque::executor::{Executor, Spawner};

pub mod arch;
#[cfg(target_arch = "x86_64")]
pub mod args;
#[cfg(target_arch = "x86_64")]
pub mod crypto;
#[cfg(target_arch = "x86_64")]
pub mod drivers;
#[cfg(target_arch = "x86_64")]
pub mod dyn_mem;
#[cfg(target_arch = "x86_64")]
pub mod ejcineque;
#[cfg(target_arch = "x86_64")]
pub mod hal;
pub mod terminal;
#[cfg(target_arch = "x86_64")]
pub mod time;

pub const STACK_SIZE: u64 = 0x100000;

#[used]
#[unsafe(link_section = ".requests")]
pub static STACK_SIZE_REQUEST: StackSizeRequest = StackSizeRequest::new().with_size(STACK_SIZE);

// will be locked all the time
#[cfg(target_arch = "x86_64")]
pub static EXECUTOR: OnceCell<Arc<Executor>> = OnceCell::new();
pub static IS_EXECUTOR_READY: AtomicBool = AtomicBool::new(false);
#[cfg(target_arch = "x86_64")]
pub static SPAWNER: OnceCell<Spawner> = OnceCell::new();

#[cfg(target_arch = "x86_64")]
pub fn spawn(future: impl Future<Output = ()> + 'static + Send) {
    SPAWNER.get().expect("Failed to get spawner").spawn(future);
}

pub static BSP_IDX: OnceCell<u32> = OnceCell::new();

/// Sets the base revision to the latest revision supported by the crate.
/// See specification for further info.
// Be sure to mark all limine requests with #[used], otherwise they may be removed by the compiler.
#[used]
// The .requests section allows limine to find the requests faster and more safely.
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::with_revision(4);

/// Define the stand and end markers for Limine requests.
#[used]
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();

#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

// #[inline(never)]
// fn force_overflow(n: u64) {
//     let large_array = [0u8; STACK_SIZE as usize]; // Allocate space on the stack to speed up the crash
//     core::hint::black_box(&large_array);
//     core::hint::black_box(force_overflow(n + 1));
// }

#[unsafe(no_mangle)]
unsafe extern "C" fn _start() -> ! {
    // All limine requests must also be referenced in a called function, otherwise they may be
    // removed by the linker.

    assert!(BASE_REVISION.is_supported());

    arch::init();
}

#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
    iprintln!("{}", _info);
    #[cfg(target_arch = "x86_64")]
    log!("{}", _info);
    hcf();
}

pub fn hcf() -> ! {
    unsafe {
        #[cfg(target_arch = "x86_64")]
        asm!("cli");
        loop {
            core::hint::spin_loop();
        }
    }
}
