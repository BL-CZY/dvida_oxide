#![no_std]
#![no_main]
#![feature(duration_from_nanos_u128)]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![feature(iter_array_chunks)]
#![test_runner(crate::terminal::test::run_tests)]
#![reexport_test_harness_main = "test_main"]
use core::{arch::asm, sync::atomic::AtomicBool};

use alloc::sync::Arc;
use once_cell_no_std::OnceCell;

extern crate alloc;

use arch::x86_64::{
    gdt::init_gdt,
    idt::init_idt,
    memory::{self, PAGE_SIZE, memmap::log_memmap},
};
#[allow(unused_imports)]
use dyn_mem::{KHEAP_PAGE_COUNT, allocator::init_kheap};
use ejcineque::{
    executor::{Executor, Spawner},
    futures::yield_now,
    sync::mutex::Mutex,
};
use limine::{
    BaseRevision,
    mp::RequestFlags,
    request::{MpRequest, RequestsEndMarker, RequestsStartMarker},
};

use limine::request::StackSizeRequest;

pub mod args;
pub mod time;

use crate::{
    arch::x86_64::{
        acpi::{
            apic::init_apic,
            find_madt, find_mcfg,
            mcfg::{iterate_pcie_entries, parse_mcfg},
            parse_rsdp,
        },
        memory::{
            MemoryMappings,
            frame_allocator::{BitmapAllocator, FRAME_ALLOCATOR, deallocator_task},
            page_table::initialize_page_table,
            per_cpu::setup_per_cpu_data,
        },
        mp::initialize_mp,
        pic::disable_pic,
        scheduler::{
            load_kernel_thread,
            syscall::{enable_syscalls, set_per_cpu_data_for_core},
        },
        timer::{calibrate_tsc, sync_tsc_lead},
    },
    args::parse_args,
    crypto::random::run_random,
    hal::storage::{identify_storage_devices, run_storage_devices},
    terminal::WRITER,
};

pub mod arch;
pub mod crypto;
pub mod drivers;
pub mod dyn_mem;
pub mod ejcineque;
pub mod hal;
pub mod terminal;

pub const STACK_SIZE: u64 = 0x100000;

#[used]
#[unsafe(link_section = ".requests")]
pub static STACK_SIZE_REQUEST: StackSizeRequest = StackSizeRequest::new().with_size(STACK_SIZE);

// will be locked all the time
pub static EXECUTOR: OnceCell<Arc<Executor>> = OnceCell::new();
pub static IS_EXECUTOR_READY: AtomicBool = AtomicBool::new(false);
pub static SPAWNER: OnceCell<Spawner> = OnceCell::new();

pub fn spawn(future: impl Future<Output = ()> + 'static + Send) {
    SPAWNER.get().expect("Failed to get spawner").spawn(future);
}

// this is the kernel entry point
async fn kernel_main(spawner: Spawner) {
    #[cfg(test)]
    test_main();

    log!("Kernel main launched");

    let args = parse_args();
    log!("Parsed args: {:?}", args);

    spawner.spawn(run_storage_devices(args));
    // we yield now to let the tasks actually initialize
    yield_now().await;

    log!("Storage drive tasks launched");

    spawner.spawn(run_random());
    yield_now().await;
    log!("Random number task launched");

    spawner.spawn(deallocator_task());
    yield_now().await;
    log!("Deallocator task launched");
}

pub static BSP_IDX: OnceCell<u32> = OnceCell::new();

/// Sets the base revision to the latest revision supported by the crate.
/// See specification for further info.
// Be sure to mark all limine requests with #[used], otherwise they may be removed by the compiler.
#[used]
// The .requests section allows limine to find the requests faster and more safely.
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::with_revision(4);

#[used]
#[unsafe(link_section = ".requests")]
pub static MP_REQUEST: MpRequest = MpRequest::new().with_flags(RequestFlags::empty());

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

    WRITER.lock().init_debug_terminal();

    log_memmap();

    let mp_response = read_mp!();

    let _ = BSP_IDX.set(mp_response.bsp_lapic_id());

    let MemoryMappings { kheap, bit_map } = memory::init();
    let _ = FRAME_ALLOCATOR
        .set(Mutex::new(BitmapAllocator {
            bitmap: bit_map,
            next: 0,
        }))
        .expect("Failed to set frame allocator");

    init_kheap(
        kheap.kheap_start,
        (KHEAP_PAGE_COUNT * PAGE_SIZE as u64 - 1) as usize,
    );

    unsafe { initialize_page_table() };

    log!("Page table initialized");

    let table_ptrs = parse_rsdp();

    let madt = find_madt(&table_ptrs).expect("No apic found");
    log!("madt ptr: {:?}", madt);
    let (_processors, mappings, mut local_apic, _io_apics) = init_apic(madt);

    setup_per_cpu_data(mp_response.cpus());
    set_per_cpu_data_for_core();

    init_gdt();

    disable_pic();

    init_idt(mappings);

    x86_64::instructions::interrupts::enable();
    log!("Interrupts enabled!");

    initialize_mp();

    local_apic.calibrate_timer();
    calibrate_tsc();

    sync_tsc_lead(mp_response.cpus().len() as u32);

    let mcfg = find_mcfg(&table_ptrs).expect("No mcfg found");
    let mcfg = parse_mcfg(mcfg);
    log!("mcfg table: {:?}", mcfg);

    let mut device_tree = iterate_pcie_entries(&mcfg.entries);

    identify_storage_devices(&mut device_tree);

    enable_syscalls();

    log!("{}", local_apic.dump());

    let executor: Executor = Executor::new(&mp_response.cpus());
    let spawner = executor.spawner();
    spawner.spawn(kernel_main(spawner.clone()));

    let _ = EXECUTOR
        .set(Arc::new(executor.clone()))
        .expect("Failed to set executor");

    let _ = SPAWNER.set(spawner).expect("Failed to set spawner");

    IS_EXECUTOR_READY.store(true, core::sync::atomic::Ordering::Release);

    load_kernel_thread();
}

#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
    iprintln!("{}", _info);
    log!("{}", _info);
    hcf();
}

pub fn hcf() -> ! {
    unsafe {
        asm!("cli");
        loop {
            asm!("hlt");
        }
    }
}
