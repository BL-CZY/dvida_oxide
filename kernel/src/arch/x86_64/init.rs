use crate::{IS_EXECUTOR_READY, SPAWNER};
use alloc::sync::Arc;
use limine::{mp::RequestFlags, request::MpRequest};

use crate::{
    BSP_IDX, EXECUTOR,
    arch::x86_64::{
        gdt::init_gdt,
        idt::init_idt,
        memory::{self, PAGE_SIZE, memmap::log_memmap},
    },
    log, read_mp,
};

use crate::dyn_mem::{KHEAP_PAGE_COUNT, allocator::init_kheap};
use crate::ejcineque::{
    executor::{Executor, Spawner},
    futures::yield_now,
    sync::mutex::Mutex,
};

#[used]
#[unsafe(link_section = ".requests")]
pub static MP_REQUEST: MpRequest = MpRequest::new().with_flags(RequestFlags::empty());

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

// this is the kernel entry point
async fn kernel_main(spawner: Spawner) {
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

pub fn init() -> ! {
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
