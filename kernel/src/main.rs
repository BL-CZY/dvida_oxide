#![no_std]
#![no_main]
#![feature(duration_from_nanos_u128)]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::terminal::test::run_tests)]
#![reexport_test_harness_main = "test_main"]
use core::arch::asm;

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
    request::{RequestsEndMarker, RequestsStartMarker},
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
        handlers::setup_rsp0_stack,
        memory::{
            MemoryMappings,
            frame_allocator::{BitmapAllocator, FRAME_ALLOCATOR, deallocator_task},
            page_table::initialize_page_table,
        },
        pic::disable_pic,
        scheduler::{
            load_kernel_thread,
            syscall::{enable_syscalls, setup_stack_for_syscall_handler},
        },
    },
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
pub static EXECUTOR: OnceCell<Arc<Mutex<Executor>>> = OnceCell::new();
pub static SPAWNER: OnceCell<Spawner> = OnceCell::new();

// this is the kernel entry point
async fn kernel_main(spawner: Spawner) {
    #[cfg(test)]
    test_main();

    log!("Kernel main launched");

    spawner.spawn(run_storage_devices());
    // we yield now to let the tasks actually initialize
    yield_now().await;

    log!("Storage drive tasks launched");

    spawner.spawn(run_random());
    yield_now().await;
    log!("Random number task launched");

    // let args = parse_args();

    // spawner.spawn(spawn_vfs_task(args.root_drive, args.root_entry));
    // yield_now().await;
    // log!("VFS task launched");

    spawner.spawn(deallocator_task());
    yield_now().await;
    log!("Deallocator task launched");

    // use alloc::vec;
    // use hal::buffer::Buffer;
    // let buffer: Buffer = vec![0u32; 128].into_boxed_slice().into();
    // hal::storage::read_sectors_by_idx(0, buffer.clone(), 1)
    //     .await
    //     .unwrap();

    // log!("{}", buffer);

    let gpt_reader = hal::gpt::GptReader::new(0);
    log!("{:?}", gpt_reader.read_gpt().await);
}
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

    WRITER.lock().init_debug_terminal();

    log_memmap();
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

    init_gdt();
    disable_pic();

    let table_ptrs = parse_rsdp();

    let madt = find_madt(&table_ptrs).expect("No apic found");
    log!("madt ptr: {:?}", madt);
    let (_processors, mappings, mut local_apic, _io_apics) = init_apic(madt);

    init_idt(mappings);

    x86_64::instructions::interrupts::enable();
    log!("Interrupts enabled!");

    local_apic.calibrate_timer(mappings[0]);

    let mcfg = find_mcfg(&table_ptrs).expect("No mcfg found");
    let mcfg = parse_mcfg(mcfg);
    log!("mcfg table: {:?}", mcfg);

    let mut device_tree = iterate_pcie_entries(&mcfg.entries);

    identify_storage_devices(&mut device_tree);

    enable_syscalls();

    let executor: Executor = Executor::new();
    let spawner = executor.spawner();
    executor.spawn(kernel_main(spawner.clone()));

    let _ = EXECUTOR
        .set(Arc::new(Mutex::new(executor.clone())))
        .expect("Failed to set executor");

    let _ = SPAWNER.set(spawner).expect("Failed to set spawner");

    setup_rsp0_stack();

    // let kernel_task_stack_start = setup_stack_for_kernel_task().as_u64();
    setup_stack_for_syscall_handler();
    load_kernel_thread();

    // jump_to_kernel_task(kernel_task_stack_start);
}

// pub fn jump_to_kernel_task(stack_top: u64) -> ! {
//     unsafe {
//         core::arch::asm!("mov rsp, {0}", "xor rbp, rbp", "call {1}", in(reg) stack_top, in(reg) kernel_thread_entry_point as u64, options(noreturn));
//     }
// }
//
// #[unsafe(no_mangle)]
// extern "C" fn kernel_thread_entry_point() -> ! {
//     EXECUTOR
//         .get()
//         .expect("Failed to get the executor")
//         .spin_acquire_lock()
//         .run();
//
//     hcf();
// }

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
