use alloc::{collections::btree_map::BTreeMap, vec::Vec};
use bytemuck::{Pod, Zeroable};
use terminal::log;
use x86_64::VirtAddr;

use crate::arch::x86_64::{
    acpi::AcpiSdtHeader,
    memory::get_hhdm_offset,
    pcie::{PciDevice, PciHeaderPartial},
};

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C, packed)]
pub struct McfgHeader {
    header: AcpiSdtHeader,
    reserve: u64,
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C, packed)]
pub struct McfgEntry {
    pub base_addr: u64,
    pub pci_segment_group_number: u16,
    pub start_pci_bus_number: u8,
    pub end_pci_bus_number: u8,
    pub reserved: u32,
}

#[derive(Debug, Clone)]
pub struct McfgTable {
    pub header: AcpiSdtHeader,
    pub entries: Vec<McfgEntry>,
}

pub fn parse_mcfg(mut ptr: VirtAddr) -> McfgTable {
    let header = unsafe { *(ptr.as_ptr() as *const AcpiSdtHeader) };

    let num_entry = (header.length - size_of::<McfgHeader>() as u32) / 16;

    ptr += size_of::<McfgHeader>() as u64;

    let mut entries: Vec<McfgEntry> = Vec::new();

    for _ in 0..num_entry {
        let entry = unsafe { *(ptr.as_ptr() as *const McfgEntry) };
        entries.push(entry);

        ptr += 16;
    }

    let result = McfgTable { header, entries };

    result
}

const BUS_DEVICE_COUNT: u64 = 32;
const DEVICE_FUNCTION_COUNT: u64 = 8;

pub fn check_function(
    address: VirtAddr,
    devices: &mut BTreeMap<u8, BTreeMap<u8, BTreeMap<u8, PciDevice>>>,
) {
    let header: PciHeaderPartial =
        unsafe { (address.as_ptr() as *const PciHeaderPartial).read_volatile() };

    if header.vendor_id != 0xFFFF {
        let device = PciDevice {
            address,
            header_partial: header,
        };

        devices
            .entry(header.class_code)
            .or_insert_with(|| {
                let mut map = BTreeMap::new();
                map.insert(header.subclass, BTreeMap::new());
                map
            })
            .entry(header.subclass)
            .or_insert_with(|| {
                let mut map = BTreeMap::new();
                map.insert(header.prog_if, device.clone());
                map
            })
            .entry(header.prog_if)
            .or_insert(device);
    }
}

pub fn iterate_pcie_buses(
    entry: &McfgEntry,
    devices: &mut BTreeMap<u8, BTreeMap<u8, BTreeMap<u8, PciDevice>>>,
) {
    let base = get_hhdm_offset() + entry.base_addr;

    for (bus_no, _) in (entry.start_pci_bus_number..=entry.end_pci_bus_number).enumerate() {
        let bus_no = bus_no as u64;

        for device_no in 0..BUS_DEVICE_COUNT {
            for function_no in 0..DEVICE_FUNCTION_COUNT {
                let address = base + ((bus_no << 20) + (device_no << 15) + (function_no << 12));

                check_function(address, devices);
            }
        }
    }
}

pub fn iterate_pcie_entries(
    entries: &[McfgEntry],
) -> BTreeMap<u8, BTreeMap<u8, BTreeMap<u8, PciDevice>>> {
    let mut res: BTreeMap<u8, BTreeMap<u8, BTreeMap<u8, PciDevice>>> = BTreeMap::new();

    for entry in entries.iter() {
        iterate_pcie_buses(entry, &mut res);
    }

    log!("Found devices: {:#?}", res);
    res
}
