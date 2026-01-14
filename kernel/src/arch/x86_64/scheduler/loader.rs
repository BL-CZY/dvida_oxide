use core::ops::DerefMut;

use alloc::{vec, vec::Vec};
use bytemuck::{Pod, Zeroable};
use x86_64::{
    PhysAddr, VirtAddr,
    registers::rflags::{self, RFlags},
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame,
        Size4KiB, mapper::MapToError,
    },
};

use crate::{
    arch::x86_64::{
        err::ErrNo,
        memory::{
            PAGE_SIZE, frame_allocator::FRAME_ALLOCATOR, get_hhdm_offset,
            page_table::create_page_table,
        },
        scheduler::{
            GPRegisterState, ThreadState,
            elf::{ElfFile, ElfProgramHeaderEntry, Flags, SegmentType},
            syscall::{PER_CPU_DATA, PerCPUData},
        },
    },
    crypto::random::random_number,
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

const HIGHER_HALF_START: u64 = 0xFFFF_8000_0000_0000;

pub async fn copy_data(
    offset: u64,
    fd: i64,
    entry: &ElfProgramHeaderEntry,
    num_pages: u64,
) -> Result<Vec<PhysFrame>, LoadErr> {
    let mut offset = offset;

    let mut remaining_size = entry.size_in_file;
    vfs_lseek(fd, crate::hal::vfs::Whence::SeekSet, entry.offset as i64).await?;

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

    Ok(phys_frames)
}

#[derive(Debug, Pod, Zeroable, Clone, Copy)]
#[repr(C)]
pub struct ThreadControlBlock {
    pub self_ptr: u64,
    pub dtv: u64,
    pub thread_id: u64,
    pub reserved: u64,
    pub x86_64_specific: u64,
    pub stack_canary: u64,
}

pub async fn handle_tls(
    page_table: &mut OffsetPageTable<'_>,
    tls_entry: &ElfProgramHeaderEntry,
    fd: i64,
) -> Result<VirtAddr, LoadErr> {
    let length = tls_entry.size_in_memory;
    let aligned_length = (length + tls_entry.alignment - 1) & !(tls_entry.alignment - 1);

    let mut frames = copy_data(
        0,
        fd,
        tls_entry,
        (aligned_length + PAGE_SIZE as u64 - 1) / PAGE_SIZE as u64,
    )
    .await?;

    let offset = aligned_length % PAGE_SIZE as u64;
    let remaining_size = PAGE_SIZE as u64 - offset;

    const TLS_START: u64 = 0x7FFF_FF00_0000;
    let tcb_loc = TLS_START + aligned_length;

    let tcb = ThreadControlBlock {
        self_ptr: tcb_loc,
        thread_id: 0,
        dtv: 0,
        x86_64_specific: 0,
        reserved: 0,
        stack_canary: ((random_number().await << 32) as u64 | random_number().await as u64) & !0xFF,
    };

    let tcb_buf = bytemuck::bytes_of(&tcb);

    if remaining_size < size_of::<ThreadControlBlock>() as u64 {
        frames.push(
            FRAME_ALLOCATOR
                .get()
                .expect("Failed to get allocator")
                .lock()
                .await
                .allocate_frame()
                .ok_or(LoadErr::NoEnoughMemory)?,
        );

        let ptr = (frames[frames.len() - 2].start_address().as_u64()
            + get_hhdm_offset().as_u64()
            + offset) as *mut u8;

        let mut buf = Buffer {
            inner: ptr,
            len: remaining_size as usize,
        };

        for i in 0..remaining_size as usize {
            buf[i] = tcb_buf[i];
        }

        let ptr = (frames[frames.len() - 1].start_address().as_u64() + get_hhdm_offset().as_u64())
            as *mut u8;

        let mut buf = Buffer {
            inner: ptr,
            len: tcb_buf.len() - remaining_size as usize,
        };

        for i in remaining_size as usize..tcb_buf.len() {
            buf[i - remaining_size as usize] = tcb_buf[i];
        }
    } else {
        let ptr = (frames[frames.len() - 1].start_address().as_u64()
            + get_hhdm_offset().as_u64()
            + offset) as *mut u8;
        let mut buf = Buffer {
            inner: ptr,
            len: tcb_buf.len(),
        };

        for i in 0..tcb_buf.len() {
            buf[i] = tcb_buf[i];
        }
    }

    let mut allocator = FRAME_ALLOCATOR
        .get()
        .expect("Failed to get allocator")
        .lock()
        .await;

    // map pages
    for (idx, frame) in frames.iter().enumerate() {
        let page: Page<Size4KiB> =
            Page::from_start_address(VirtAddr::new(TLS_START + idx as u64 * PAGE_SIZE as u64))
                .expect("Failed to create page");

        unsafe {
            let _ = page_table
                .map_to(
                    page,
                    *frame,
                    PageTableFlags::NO_EXECUTE
                        | PageTableFlags::WRITABLE
                        | PageTableFlags::PRESENT
                        | PageTableFlags::USER_ACCESSIBLE,
                    allocator.deref_mut(),
                )
                .map_err(LoadErr::MappingErr)?;
        };
    }

    Ok(VirtAddr::new(TLS_START + aligned_length))
}

pub async fn get_stack(page_table: &mut OffsetPageTable<'_>) -> Result<VirtAddr, LoadErr> {
    const STACK_START: u64 = STACK_GUARD_PAGE + PAGE_SIZE as u64;
    const STACK_GUARD_PAGE: u64 = 0x7FFF_FFFF_0000;
    const STACK_LEN: u64 = 16 * PAGE_SIZE as u64;

    let mut allocator = FRAME_ALLOCATOR
        .get()
        .expect("Failed to get the frame allocator")
        .lock()
        .await;

    let mut frames: heapless::Vec<PhysFrame<Size4KiB>, 16> = heapless::Vec::new();

    for _ in 0..15 {
        let frame = allocator
            .allocate_frame()
            .expect("Failed to get physical frame");
        frames.push(frame).expect("Failed to push");
    }

    for (idx, frame) in frames.iter().enumerate() {
        let page: Page<Size4KiB> =
            Page::from_start_address(VirtAddr::new(STACK_START + idx as u64 * PAGE_SIZE as u64))
                .expect("Failed to create page");

        unsafe {
            let _ = page_table
                .map_to(
                    page,
                    *frame,
                    PageTableFlags::NO_EXECUTE
                        | PageTableFlags::WRITABLE
                        | PageTableFlags::PRESENT
                        | PageTableFlags::USER_ACCESSIBLE,
                    allocator.deref_mut(),
                )
                .map_err(LoadErr::MappingErr)?;
        };
    }

    Ok(VirtAddr::new(STACK_GUARD_PAGE + STACK_LEN))
}

pub async fn load_elf(fd: i64, elf: ElfFile) -> Result<ThreadState, LoadErr> {
    let mut map_entries: Vec<MapEntry> = vec![];
    let page_table = unsafe { &mut *(create_page_table().await.as_mut_ptr() as *mut PageTable) };
    let mut offset_page_table = unsafe { OffsetPageTable::new(page_table, get_hhdm_offset()) };
    let mut tls_ptr = None;

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
            let offset = entry.size_in_memory % PAGE_SIZE as u64;

            let phys_frames = copy_data(offset, fd, entry, num_pages).await?;

            map_entries.push(MapEntry {
                entry: entry,
                frames: phys_frames,
            });
        } else if entry.segment_type == SegmentType::TLS as u32 {
            tls_ptr = Some(handle_tls(&mut offset_page_table, entry, fd).await?);
        } else {
            todo!()
        }
    }

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

    let stack_top = get_stack(&mut offset_page_table).await? - 8;

    let table_virt_addr = VirtAddr::from_ptr(page_table as *mut PageTable);
    let table_phys_addr = PhysAddr::new(table_virt_addr.as_u64() - get_hhdm_offset().as_u64());

    Ok(ThreadState {
        registers: GPRegisterState::default(),
        stack_pointer: stack_top,
        state: crate::arch::x86_64::scheduler::State::Paused {
            instruction_pointer: elf.header.entry_offset,
            rflags: rflags::read(),
        },
        thread_local_segment: tls_ptr.map_or(VirtAddr::zero(), |p| p),
        page_table_pointer: table_phys_addr,
        fpu_registers: None,
        simd_registers: None,
    })
}
