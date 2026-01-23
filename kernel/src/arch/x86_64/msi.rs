use bitfield::bitfield;
use x86_64::VirtAddr;

use crate::{
    arch::x86_64::acpi::apic::{IoApicDeliveryMode, IoApicInterruptTriggerMode},
    pcie_offset_impl,
};

bitfield! {
    /// behavior:
    /// rh = 0 -> send to destination id
    /// rh = 1, dm = 0 -> send to destination id
    /// rh = 1, dm = 1 -> logical
    pub struct MessageAddressRegister(u32);
    impl Debug;

    pub destination_id, set_destination_id: 19, 12;
    pub redirection_hint, set_redirection_hint: 3, 3;
    pub destination_mode, set_destination_mode: 2, 2;
}

impl Default for MessageAddressRegister {
    fn default() -> Self {
        let mut res = MessageAddressRegister(0xFEE00000);
        res.set_redirection_hint(0);
        res
    }
}

bitfield! {
    pub struct MessageDataRegister(u32);
    impl Debug;

    pub vector, set_vector: 7, 0;
    pub delivery_mode, set_delivery_mode: 10, 8;
    pub level_for_trigger_mode, set_level_for_trigger_mode: 14, 14;
    pub trigger_mode, set_trigger_mode: 15, 15;
}

impl Default for MessageDataRegister {
    fn default() -> Self {
        let mut res = Self(0);
        res.set_delivery_mode(IoApicDeliveryMode::FIXED as u32);
        res.set_level_for_trigger_mode(IoApicInterruptTriggerMode::EDGE_SENSITIVE as u32);
        res
    }
}

bitfield! {
    pub struct MsiControl(u16);
    impl Debug;
    pub enable, set_enable: 0;
    pub multiple_message_capable, _: 3, 1;
    pub multiple_message_enable, set_multiple_message_enable: 6, 4;
    pub address_64, _: 7;
    pub masking, _: 8;
}

#[derive(Debug, Clone)]
pub struct PcieMsiCapNode {
    pub base: VirtAddr,
}

impl PcieMsiCapNode {
    pcie_offset_impl!(
        <message_control_register, 0x2, "rw", u16>,
        <message_addr_register, 0x4, "rw">,

        // if 64 bit is not enabled this is not used
        <message_upper_addr_register, 0x8, "rw">,

        // if 64 bit is not enabled this is used
        <message_data_register, 0x8, "rw">,
        // if 64 bit is enabled this is used
        <message_data_register_64_bit, 0xc, "rw">
    );
}
