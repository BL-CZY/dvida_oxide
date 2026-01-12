use core::ops::DerefMut;

use alloc::{vec, vec::Vec};
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageSize, PageTable, PageTableFlags,
        PhysFrame, Size4KiB, mapper::MapToError,
    },
};

use crate::{
    arch::x86_64::{
        err::ErrNo,
        memory::{
            PAGE_SIZE,
            frame_allocator::{self, FRAME_ALLOCATOR},
            get_hhdm_offset,
            page_table::create_page_table,
        },
        scheduler::elf::{ElfFile, ElfProgramHeaderEntry, Flags, SegmentType},
    },
    drivers::fs::ext2::allocator,
    hal::{
        buffer::Buffer,
        vfs::{vfs_lseek, vfs_read},
    },
};

pub enum LoadErr {
    VfsErr(ErrNo),
    NoEnoughMemory,
    MappingErr(MapToError<Size4KiB>),
    Corrupted,
}

impl From<ErrNo> for LoadErr {
    fn from(value: ErrNo) -> Self {
        Self::VfsErr(value)
    }
}

pub struct MapEntry<'a> {
    pub entry: &'a ElfProgramHeaderEntry,
    pub frames: Vec<PhysFrame>,
}

const HIGHER_HALF_START: u64 = 0xFFFF800000000000;

pub async fn load_elf(fd: i64, elf: ElfFile) -> Result<PhysAddr, LoadErr> {
    let mut map_entries: Vec<MapEntry> = vec![];

    for entry in elf.program_header_table.iter() {
        if entry.segment_type == SegmentType::Null as u32
            || entry.segment_type == SegmentType::Note as u32
        {
        } else if entry.segment_type == SegmentType::Load as u32 {
            if entry.vaddr + entry.size_in_memory >= HIGHER_HALF_START {
                return Err(LoadErr::Corrupted);
            }

            let start = entry.vaddr & !(PAGE_SIZE as u64 - 1);
            let end = (entry.vaddr + entry.size_in_memory + PAGE_SIZE as u64 - 1)
                & !(PAGE_SIZE as u64 - 1);

            let num_pages = (end - start + PAGE_SIZE as u64 - 1) / PAGE_SIZE as u64;
            let mut phys_frames: Vec<PhysFrame> = vec![];

            for _ in 0..num_pages {
                let frame = FRAME_ALLOCATOR
                    .get()
                    .expect("Failed to get the allocator")
                    .lock()
                    .await
                    .allocate_frame()
                    .ok_or(LoadErr::NoEnoughMemory)?;

                phys_frames.push(frame);
            }

            let hhdm = get_hhdm_offset();
            let mut remaining_size = entry.size_in_file;

            vfs_lseek(fd, crate::hal::vfs::Whence::SeekSet, entry.offset as i64).await?;

            let mut offset = entry.vaddr % PAGE_SIZE as u64;

            for frame in phys_frames.iter() {
                let addr = hhdm + frame.start_address().as_u64();

                if remaining_size == 0 {
                    let mut buffer = Buffer {
                        inner: addr.as_mut_ptr(),
                        len: PAGE_SIZE as usize,
                    };
                    buffer.fill(0);
                    continue;
                }

                let buffer = if remaining_size >= PAGE_SIZE as u64 - offset {
                    let buffer = Buffer {
                        inner: (addr.as_u64() + offset) as *mut u8,
                        len: PAGE_SIZE as usize - offset as usize,
                    };

                    buffer
                } else {
                    let mut buffer = Buffer {
                        inner: (addr.as_u64() + offset) as *mut u8,
                        len: PAGE_SIZE as usize - remaining_size as usize - offset as usize,
                    };

                    buffer.fill(0);

                    let buffer = Buffer {
                        inner: (addr.as_u64() + remaining_size + offset) as *mut u8,
                        len: remaining_size as usize + offset as usize,
                    };

                    buffer
                };

                let bytes_read = vfs_read(fd, buffer.clone()).await?;
                if bytes_read < buffer.len() as i64 {
                    return Err(LoadErr::Corrupted);
                }

                if remaining_size >= PAGE_SIZE as u64 - offset {
                    remaining_size -= PAGE_SIZE as u64 - offset;
                } else {
                    remaining_size = 0;
                }

                offset = 0;
            }

            map_entries.push(MapEntry {
                entry: entry,
                frames: phys_frames,
            });
        } else {
            todo!()
        }
    }

    let page_table = unsafe { &mut *(create_page_table().await.as_mut_ptr() as *mut PageTable) };

    let mut offset_page_table = unsafe { OffsetPageTable::new(page_table, get_hhdm_offset()) };
    let mut frame_allocator = FRAME_ALLOCATOR
        .get()
        .expect("Failed to get allocator")
        .lock()
        .await;

    for map_entry in map_entries.iter() {
        let start = map_entry.entry.vaddr;
        let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;

        // TODO: enable no execute
        if map_entry.entry.flags & Flags::Writable as u32 != 0 {
            flags = flags | PageTableFlags::WRITABLE;
        }

        if map_entry.entry.flags & Flags::Executable as u32 == 0 {
            flags = flags | PageTableFlags::NO_EXECUTE;
        }

        for (idx, phys_frame) in map_entry.frames.iter().into_iter().enumerate() {
            unsafe {
                let _ = offset_page_table
                    .map_to(
                        Page::containing_address(VirtAddr::new(
                            start + idx as u64 * PAGE_SIZE as u64,
                        )),
                        *phys_frame,
                        flags,
                        frame_allocator.deref_mut(),
                    )
                    .map_err(|e| LoadErr::MappingErr(e))?;
            }
        }
    }

    let table_virt_addr = VirtAddr::from_ptr(page_table as *mut PageTable);
    let table_phys_addr = PhysAddr::new(table_virt_addr.as_u64() - get_hhdm_offset().as_u64());

    Ok(table_phys_addr)
}
