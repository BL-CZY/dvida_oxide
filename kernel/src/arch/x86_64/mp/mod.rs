use bitfield::bitfield;
use limine::{
    mp::{Cpu, RequestFlags},
    request::MpRequest,
};

use crate::{
    arch::x86_64::{acpi::apic::get_local_apic, gdt::init_gdt, idt::load_idt},
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

#[used]
#[unsafe(link_section = ".requests")]
static MP_REQUEST: MpRequest = MpRequest::new().with_flags(RequestFlags::empty());

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
    init_gdt();
    load_idt();

    let mut local_apic = get_local_apic();
    local_apic.calibrate_timer(0);

    x86_64::instructions::interrupts::enable();
    log!("Interrupts enabled on core: {:?}!", cpu.id);

    loop {
        unsafe { core::arch::asm!("hlt") }
    }
}
