use bitfield::bitfield;
use limine::mp::Cpu;

use crate::{
    IS_EXECUTOR_READY, MP_REQUEST,
    arch::x86_64::{
        acpi::apic::get_local_apic,
        gdt::init_gdt,
        idt::load_idt,
        scheduler::{
            load_kernel_thread,
            syscall::{enable_syscalls, set_per_cpu_data_for_core},
        },
        timer::sync_tsc_follow,
    },
    log,
};

bitfield! {
    pub struct InterruptCmdRegister(u64);
    impl Debug;

    pub vector, set_vector: 7, 0;
    pub delivery_mode, set_delivery_mode: 10, 8;
    pub destination_mode, set_destination_moed: 11, 11;
    pub delivery_status, set_delivery_status: 12, 12;
    pub level, get_level: 14, 14;
    pub trigger_mode, set_trigger_mode: 15, 15;
    pub destination_shorthand, set_destination_shorthand: 19, 18;
    pub destination, set_destination: 63, 56;
}

#[macro_export]
macro_rules! read_mp {
    () => {
        MP_REQUEST.get_response().expect("No MP response")
    };
}

pub fn initialize_mp() {
    let response = MP_REQUEST.get_response().expect("No MP response");

    for cpu in response.cpus() {
        if cpu.id != response.bsp_lapic_id() {
            cpu.goto_address.write(ap_init);
        }
    }
}

extern "C" fn ap_init(cpu: &Cpu) -> ! {
    log!("Initializing core: {:?}", cpu.id);

    set_per_cpu_data_for_core();
    init_gdt();

    load_idt();

    x86_64::instructions::interrupts::enable();
    log!("Interrupts enabled on core: {:?}!", cpu.id);

    let mut local_apic = get_local_apic();
    local_apic.calibrate_timer();
    local_apic.write_task_priority(0);
    // this enables lapic
    local_apic
        .write_spurious_interrupt_vector(local_apic.read_spurious_interrupt_vector() | (0x1 << 8));

    sync_tsc_follow();

    log!("{}", local_apic.dump());

    enable_syscalls();

    while !IS_EXECUTOR_READY.load(core::sync::atomic::Ordering::Acquire) {
        core::hint::spin_loop();
    }

    log!("Load kernel thread");

    load_kernel_thread();
}
