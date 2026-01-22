use x86_64::VirtAddr;

use crate::pcie_offset_impl;

#[macro_export]
macro_rules! pcie_port_readonly {
    ($name:ident, $output_type:ty, | $self:ident | $addr:block) => {
        paste::paste! {
            pub fn [<read_$name>](&$self) -> $output_type {
                let address: *mut $output_type = $addr;
                unsafe { address.read_volatile() }
            }
        }
    };

    ($name:ident, $output_type:ty, | $self:ident | $addr:block, || $head:block, || $tail:block) => {
        paste::paste! {
            pub fn [<read_$name>](&$self) -> $output_type {
                $head;

                let address: *mut $output_type = $addr;
                let res = unsafe { address.read_volatile() };

                $tail;

                res
            }
        }
    };
}

#[macro_export]
macro_rules! pcie_port_writeonly {
    ($name:ident, $input_type:ty, | $self:ident | $addr:block) => {
        paste::paste! {
            pub fn [<write_$name>](&mut $self, input: $input_type) {
                let address: *mut $input_type = $addr;
                unsafe { address.write_volatile(input) }
            }
        }
    };

    ($name:ident, $input_type:ty, | $self:ident | $addr:block, || $head:block, || $tail:block) => {
        paste::paste! {
            pub fn [<write_$name>](&mut $self, input: $input_type) {
                $head;

                let address: *mut $input_type = $addr;
                unsafe { address.write_volatile(input) }

                $tail;
            }
        }
    };
}

#[macro_export]
macro_rules! pcie_port_readwrite {
    ($($args:tt)*) => {
        $crate::pcie_port_readonly!($($args)*);
        $crate::pcie_port_writeonly!($($args)*);
    };
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct PciHeaderPartial {
    pub vendor_id: u16,
    pub device_id: u16,
    pub command: u16,
    pub status: u16,
    pub revision_id: u8,
    pub prog_if: u8,
    pub subclass: u8,
    pub class_code: u8,
    pub cache_line_size: u8,
    pub latency_timer: u8,
    pub header_type: u8,
}

#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
pub struct CapabilityNodeHeader {
    pub cap_id: u8,
    pub next: u8,
}

impl CapabilityNodeHeader {
    pub const MSI: u8 = 0x5;
    pub const MSIX: u8 = 0x11;
}

#[derive(Debug, Copy, Clone)]
pub struct PciHeader {
    pub base: VirtAddr,
}

impl PciHeader {
    pcie_offset_impl! {
        <vendor_id,                0x00, "r",  u16>,
        <device_id,                0x02, "r",  u16>,
        <command,                  0x04, "rw", u16>,
        <status,                   0x06, "rw", u16>,
        <revision_id,              0x08, "r",  u8>,
        <prog_if,                  0x09, "r",  u8>,
        <subclass,                 0x0A, "r",  u8>,
        <class_code,               0x0B, "r",  u8>,
        <cache_line_size,          0x0C, "rw", u8>,
        <latency_timer,            0x0D, "rw", u8>,
        <header_type,              0x0E, "r",  u8>,
        <bist,                     0x0F, "rw", u8>,

        <bar0,                     0x10, "rw", u32>,
        <bar1,                     0x14, "rw", u32>,
        <bar2,                     0x18, "rw", u32>,
        <bar3,                     0x1C, "rw", u32>,
        <bar4,                     0x20, "rw", u32>,
        <bar5,                     0x24, "rw", u32>,

        <cardbus_cis_ptr,          0x28, "r",  u32>,
        <subsystem_vendor_id,      0x2C, "r",  u16>,
        <subsystem_id,             0x2E, "r",  u16>,
        <expansion_rom_base_addr,  0x30, "rw", u32>,
        <capabilities_ptr,         0x34, "r",  u8>,

        <interrupt_line,           0x3C, "rw", u8>,
        <interrupt_pin,            0x3D, "r",  u8>,
        <min_grant,                0x3E, "r",  u8>,
        <max_latency,              0x3F, "r",  u8>
    }
}

#[repr(u8)]
pub enum PciBaseClass {
    Unclassified = 0x00,
    MassStorage = 0x01,
    Network = 0x02,
    Display = 0x03,
    Multimedia = 0x04,
    Memory = 0x05,
    Bridge = 0x06,
    SimpleComm = 0x07,
    BaseSystemPeripheral = 0x08,
    InputDevice = 0x09,
    DockingStation = 0x0A,
    Processor = 0x0B,
    SerialBus = 0x0C,
    Wireless = 0x0D,
    IntelligentIO = 0x0E,
    SatelliteComm = 0x0F,
    Encryption = 0x10,
    SignalProcessing = 0x11,
    ProcessingAccelerator = 0x12,
    NonEssentialInstrumentation = 0x13,
    CoProcessor = 0x40,
    Unassigned = 0xFF,
}

#[repr(u8)]
pub enum MassStorageControllerSubClass {
    Sata = 0x06,
}

#[repr(u8)]
pub enum SataProgIf {
    Ahci = 0x01,
}

#[derive(Debug, Clone)]
pub struct PciDevice {
    pub address: VirtAddr,
    pub header_partial: PciHeaderPartial,
}
