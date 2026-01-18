use alloc::string::{FromUtf16Error, String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use terminal::log;
use thiserror::Error;

use crate::crypto;
use crate::crypto::guid::Guid;

use super::storage::HalStorageDevice;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct GPTHeader {
    sig: [u8; 8],
    revision: u32,
    size: u32,
    header_crc32: u32,
    reserved: u32,
    loc: u64,
    backup_loc: u64,
    first_usable_block: u64,
    last_usable_block: u64,
    guid: Guid,
    array_start: u64,
    entry_num: u32,
    entry_size: u32,
    array_crc32: u32,
}

impl Default for GPTHeader {
    fn default() -> Self {
        GPTHeader {
            sig: *b"EFI PART",
            revision: 0x00010000,
            size: 0x5C,
            header_crc32: 0,
            reserved: 0,
            loc: 1,
            backup_loc: 0,
            first_usable_block: 0x22,
            last_usable_block: 0,
            guid: Guid::new(),
            array_start: 2,
            entry_num: 0x80,
            entry_size: 0x80,
            array_crc32: 0,
        }
    }
}

impl Into<Vec<u8>> for GPTHeader {
    fn into(self) -> Vec<u8> {
        self.to_buf()
    }
}

impl TryFrom<&[u8]> for GPTHeader {
    type Error = GPTErr;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() < 92 {
            return Err(GPTErr::BufferTooSmall);
        }

        Ok(GPTHeader {
            sig: value[0..8].try_into().unwrap(),
            revision: u32::from_le_bytes(value[8..12].try_into().unwrap()),
            size: u32::from_le_bytes(value[12..16].try_into().unwrap()),
            header_crc32: u32::from_le_bytes(value[16..20].try_into().unwrap()),
            reserved: 0,
            loc: u64::from_le_bytes(value[24..32].try_into().unwrap()),
            backup_loc: u64::from_le_bytes(value[32..40].try_into().unwrap()),
            first_usable_block: u64::from_le_bytes(value[40..48].try_into().unwrap()),
            last_usable_block: u64::from_le_bytes(value[48..56].try_into().unwrap()),
            guid: Guid::from_buf(value[56..72].try_into().unwrap()),
            array_start: u64::from_le_bytes(value[72..80].try_into().unwrap()),
            entry_num: u32::from_le_bytes(value[80..84].try_into().unwrap()),
            entry_size: u32::from_le_bytes(value[84..88].try_into().unwrap()),
            array_crc32: u32::from_le_bytes(value[88..92].try_into().unwrap()),
        })
    }
}

impl GPTHeader {
    pub fn try_from_buf(buf: &[u8]) -> Result<Self, GPTErr> {
        GPTHeader::try_from(buf)
    }

    pub fn to_buf(&self) -> Vec<u8> {
        let mut vec: Vec<u8> = vec![];
        vec.extend(&self.sig);
        vec.extend(&self.revision.to_le_bytes());
        vec.extend(&self.size.to_le_bytes());
        vec.extend(&self.header_crc32.to_le_bytes());
        vec.extend(&self.reserved.to_le_bytes());
        vec.extend(&self.loc.to_le_bytes());
        vec.extend(&self.backup_loc.to_le_bytes());
        vec.extend(&self.first_usable_block.to_le_bytes());
        vec.extend(&self.last_usable_block.to_le_bytes());
        vec.extend(&self.guid.to_buf());
        vec.extend(&self.array_start.to_le_bytes());
        vec.extend(&self.entry_num.to_le_bytes());
        vec.extend(&self.entry_size.to_le_bytes());
        vec.extend(&self.array_crc32.to_le_bytes());

        vec
    }

    pub fn to_buf_full(&self) -> Vec<u8> {
        let mut result = self.to_buf();
        while result.len() < 512 {
            result.push(0);
        }

        result
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct GPTEntry {
    pub type_guid: Guid,
    pub unique_guid: Guid,
    pub start_lba: u64,
    pub end_lba: u64,
    pub flags: u64,
    pub name: [u16; 36],
}

impl Default for GPTEntry {
    fn default() -> Self {
        GPTEntry {
            type_guid: Guid::default(),
            unique_guid: Guid::default(),
            start_lba: 0,
            end_lba: 0,
            flags: 0,
            name: [0u16; 36],
        }
    }
}

impl TryFrom<&[u8]> for GPTEntry {
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if !(value.len() / 128).is_power_of_two() {
            return Err(GPTErr::BadArrayEntrySize);
        }

        Ok(GPTEntry {
            type_guid: Guid::from_buf(value[0..16].try_into().unwrap()),
            unique_guid: Guid::from_buf(value[16..32].try_into().unwrap()),
            start_lba: u64::from_le_bytes(value[32..40].try_into().unwrap()),
            end_lba: u64::from_le_bytes(value[40..48].try_into().unwrap()),
            flags: u64::from_le_bytes(value[48..56].try_into().unwrap()),
            name: {
                let mut out = [0u16; 36];
                for (i, chunk) in value[56..56 + 72].chunks_exact(2).enumerate() {
                    out[i] = u16::from_be_bytes([chunk[0], chunk[1]]);
                }
                out
            },
        })
    }

    type Error = GPTErr;
}

impl Into<Vec<u8>> for GPTEntry {
    fn into(self) -> Vec<u8> {
        self.to_buf()
    }
}

impl GPTEntry {
    pub fn is_empty(&self) -> bool {
        self.start_lba == 0
    }

    pub fn empty() -> Self {
        Self {
            type_guid: Guid::new(),
            unique_guid: Guid::new(),
            start_lba: 0,
            end_lba: 0,
            flags: 0,
            name: [0u16; 36],
        }
    }

    pub fn get_name(&self) -> String {
        String::from_utf16_lossy(&self.name)
    }

    pub fn try_from_buf(buf: &[u8]) -> Result<Self, GPTErr> {
        buf.try_into()
    }

    pub fn to_buf(&self) -> Vec<u8> {
        let mut result = vec![];

        result.extend(self.type_guid.to_buf());
        result.extend(self.unique_guid.to_buf());
        result.extend(self.start_lba.to_le_bytes());
        result.extend(self.end_lba.to_le_bytes());
        result.extend(self.flags.to_le_bytes());
        result.extend(self.name.iter().map(|a| a.to_le_bytes()).flatten());

        result
    }
}

#[derive(Debug, Error)]
pub enum GPTErr {
    #[error("The buffer input is too small")]
    BufferTooSmall,
    #[error("A GPT table already exists")]
    GPTAlreadyExist,
    #[error("A GPT table doesn't exist")]
    GPTNonExist,
    #[error("The GPT table is corrupted")]
    GPTCorrupted,
    #[error("The Array size is bad")]
    BadArrayEntrySize,
    #[error("There is no free slot")]
    NoFreeSlot,
    #[error("Partition overlapped")]
    OverlappingPartition,
    #[error("LBA range is invalid")]
    InvalidLBARange,
    #[error("Entry index is invalid")]
    InvalidEntryIndex,
    #[error("Entry is already empty")]
    EntryAlreadyEmpty,
    #[error("The name is too long")]
    NameTooLong,
    #[error("Failed to serialize {0}")]
    SerializationErr(FromUtf16Error),
    #[error("Read/Write failed: {0}")]
    Io(String),
    #[error("Drive didn't respond")]
    DriveDidntRespond,
}

impl HalStorageDevice {
    async fn is_normal_present(&mut self) -> bool {
        log!("Checking primary GPT presence at LBA 1");
        let mut buf = [0u8; 512];
        if self.read_sectors_async(1, 1, &mut buf).await.is_err() {
            log!("Failed to read primary GPT sector");
            return false;
        }

        let present = buf.starts_with(b"EFI PART");
        if present {
            log!("Primary GPT signature found");
        } else {
            log!("Primary GPT signature not found");
        }

        present
    }

    async fn is_backup_present(&mut self) -> bool {
        log!("Checking backup GPT presence at LBA -1");
        let mut buf = [0u8; 512];
        if self.read_sectors_async(-1, 1, &mut buf).await.is_err() {
            log!("Failed to read backup GPT sector");
            return false;
        }

        let present = buf.starts_with(b"EFI PART");
        if present {
            log!("Backup GPT signature found");
        } else {
            log!("Backup GPT signature not found");
        }

        present
    }

    pub async fn is_gpt_present(&mut self) -> bool {
        log!("Checking if GPT is present (primary or backup)");
        let normal = self.is_normal_present().await;
        if normal {
            log!("GPT present on primary");
            return true;
        }

        let backup = self.is_backup_present().await;
        if backup {
            log!("GPT present on backup");
        } else {
            log!("No GPT detected on primary or backup");
        }

        normal || backup
    }

    fn create_pmbr_buf(&mut self) -> [u8; 512] {
        log!("Creating PMBR buffer");
        const PMBR_OFFSET: usize = 446;
        let mut result = [0u8; 512];

        result[PMBR_OFFSET + 1] = 0x0;
        result[PMBR_OFFSET + 2] = 0x2;
        result[PMBR_OFFSET + 3] = 0x0;
        result[PMBR_OFFSET + 4] = 0xEE;

        let (cylinder, head, sector) =
            crypto::lba_to_chs(self.sectors_per_track(), self.sector_count());
        log!(
            "PMBR CHS values cylinder={}, head={}, sector={}",
            cylinder,
            head,
            sector
        );

        if cylinder > 0xFF || head > 0xFF || sector > 0xFF {
            result[PMBR_OFFSET + 5] = 0xFF;
            result[PMBR_OFFSET + 6] = 0xFF;
            result[PMBR_OFFSET + 7] = 0xFF;
        } else {
            result[PMBR_OFFSET + 5] = cylinder as u8;
            result[PMBR_OFFSET + 6] = head as u8;
            result[PMBR_OFFSET + 7] = sector as u8;
        }

        result[PMBR_OFFSET + 8] = 0x1;
        result[PMBR_OFFSET + 9] = 0x0;
        result[PMBR_OFFSET + 10] = 0x0;
        result[PMBR_OFFSET + 11] = 0x0;

        if self.sector_count() > 0xFFFFFFFF {
            result[PMBR_OFFSET + 12] = 0xFF;
            result[PMBR_OFFSET + 13] = 0xFF;
            result[PMBR_OFFSET + 14] = 0xFF;
            result[PMBR_OFFSET + 15] = 0xFF;
        } else {
            let temp = self.sector_count() as u32;
            result[PMBR_OFFSET + 12] = temp.to_le_bytes()[0];
            result[PMBR_OFFSET + 13] = temp.to_le_bytes()[1];
            result[PMBR_OFFSET + 14] = temp.to_le_bytes()[2];
            result[PMBR_OFFSET + 15] = temp.to_le_bytes()[3];
        }

        result[510] = 0x55;
        result[511] = 0xAA;

        log!("PMBR buffer created with signature 0x55AA at end");

        result
    }

    fn create_unhashed_header(&mut self) -> GPTHeader {
        let hdr = GPTHeader {
            backup_loc: self.sector_count() - 1,
            last_usable_block: self.sector_count() - 34,
            ..Default::default()
        };

        log!(
            "Created unhashed GPT header: backup_loc={}, last_usable_block={}",
            hdr.backup_loc,
            hdr.last_usable_block
        );

        hdr
    }

    async fn write_pmbr(&mut self, pmbr: &[u8; 512]) -> Result<(), GPTErr> {
        log!("Writing PMBR to sector 0");
        self.write_sectors_async(0, 1, pmbr).await.map_err(|e| {
            log!("Failed to write PMBR: {}", e.to_string());
            GPTErr::Io(e.to_string())
        })?;

        log!("PMBR write completed");
        Ok(())
    }

    async fn write_table(&mut self, header: &[u8], array: &[u8]) -> Result<(), GPTErr> {
        log!("Writing GPT header to primary (LBA 1)");
        self.write_sectors_async(1, 1, header).await.map_err(|e| {
            log!("Failed to write GPT header to primary: {}", e.to_string());
            GPTErr::Io(e.to_string())
        })?;

        log!("Writing GPT array to primary (LBA 2..)");
        self.write_sectors_async(2, 32, array).await.map_err(|e| {
            log!("Failed to write GPT array to primary: {}", e.to_string());
            GPTErr::Io(e.to_string())
        })?;

        log!("Writing GPT header to backup");
        self.write_sectors_async(-1, 1, header).await.map_err(|e| {
            log!("Failed to write GPT header to backup: {}", e.to_string());
            GPTErr::Io(e.to_string())
        })?;

        log!("Writing GPT array to backup");
        self.write_sectors_async(-33, 32, array)
            .await
            .map_err(|e| {
                log!("Failed to write GPT array to backup: {}", e.to_string());
                GPTErr::Io(e.to_string())
            })?;

        log!("GPT table write completed (primary + backup)");
        Ok(())
    }

    pub async fn create_gpt(&mut self, force: bool) -> Result<(), GPTErr> {
        log!("Creating GPT: force={}", force);
        if !force && self.is_gpt_present().await {
            log!("GPT already exists and force is false; aborting create_gpt");
            return Err(GPTErr::GPTAlreadyExist);
        }

        let pmbr = self.create_pmbr_buf();
        let mut header = self.create_unhashed_header();
        let array = [0u8; 32 * 512];
        header.array_crc32 = crypto::crc32::full_crc(&array);
        header.header_crc32 = crypto::crc32::full_crc(&header.to_buf());

        log!(
            "PMBR and header CRC computed: array_crc={}, header_crc={}",
            header.array_crc32,
            header.header_crc32
        );

        self.write_pmbr(&pmbr).await?;
        self.write_table(&header.to_buf_full(), &array).await?;

        log!("GPT creation completed");
        Ok(())
    }

    fn is_valid_header(&self, buf: &[u8]) -> bool {
        let mut header: GPTHeader = match buf.try_into() {
            Ok(h) => h,
            Err(e) => {
                log!("Failed to parse GPT header from buffer: {}", e);
                return false;
            }
        };

        let crc = header.header_crc32;
        header.header_crc32 = 0;

        let ok = crypto::crc32::is_verified_crc32(&header.to_buf(), crc);
        log!("Header CRC validation result={}", ok);
        ok
    }

    pub async fn get_table(
        &mut self,
        lba: i64,
        is_backup: bool,
    ) -> Result<(GPTHeader, Vec<GPTEntry>), GPTErr> {
        log!("Reading GPT table at lba={} (is_backup={})", lba, is_backup);

        // Read header
        let mut header_buf = [0u8; 512];
        self.read_sectors_async(lba, 1, &mut header_buf)
            .await
            .map_err(|e| {
                log!(
                    "Failed to read GPT header at lba={}: {}",
                    lba,
                    e.to_string()
                );
                GPTErr::Io(e.to_string())
            })?;

        if !self.is_valid_header(&header_buf) {
            log!("Invalid GPT header detected at lba={}", lba);
            return Err(GPTErr::GPTCorrupted);
        }

        let result_header = GPTHeader::try_from(header_buf.as_slice())?;

        if !(result_header.entry_size / 128).is_power_of_two() {
            log!(
                "GPT entry size appears invalid: {}",
                result_header.entry_size
            );
            return Err(GPTErr::BadArrayEntrySize);
        }

        let arr_block_count: i64 = ((result_header.entry_num * result_header.entry_size / 512)
            + ((result_header.entry_num * result_header.entry_size) % 512 != 0) as u32)
            .into();

        let arr_lba: i64 = if is_backup {
            -1 - arr_block_count
        } else {
            result_header.array_start as i64
        };

        log!(
            "Reading GPT array at lba={} (blocks={})",
            arr_lba,
            arr_block_count
        );

        let arr_sectors = (result_header.entry_num * result_header.entry_size / 512) as u16;
        let mut arr_buf = vec![0u8; arr_sectors as usize * 512];
        self.read_sectors_async(arr_lba, arr_sectors, &mut arr_buf)
            .await
            .map_err(|e| {
                log!(
                    "Failed to read GPT array at lba={}: {}",
                    arr_lba,
                    e.to_string()
                );
                GPTErr::Io(e.to_string())
            })?;

        if !crypto::crc32::is_verified_crc32(&arr_buf, result_header.array_crc32) {
            log!(
                "GPT array CRC mismatch: expected={} (lba={})",
                result_header.array_crc32,
                arr_lba
            );
            return Err(GPTErr::GPTCorrupted);
        }

        let result_array: Vec<GPTEntry> = arr_buf
            .chunks(result_header.entry_size as usize)
            .map(|slice| GPTEntry::try_from_buf(slice).unwrap())
            .collect();

        log!(
            "Successfully read GPT header and array (entries={})",
            result_header.entry_num
        );
        Ok((result_header, result_array))
    }

    pub async fn read_gpt(&mut self) -> Result<(GPTHeader, Vec<GPTEntry>), GPTErr> {
        log!("Reading GPT (primary + backup)");
        if !self.is_gpt_present().await {
            log!("No GPT present when attempting to read");
            return Err(GPTErr::GPTNonExist);
        }

        let primary_result = self.get_table(1, false).await;
        let backup_result = self.get_table(-1, true).await;

        if let Ok((primary_header, primary_array)) = primary_result.as_ref()
            && let Ok((backup_header, backup_array)) = backup_result.as_ref()
        {
            if primary_header != backup_header || primary_array != backup_array {
                log!("Primary table differs from backup; synchronization needed");
                // TODO sync this
            }

            log!("Primary and backup GPT match (or acceptable)");
            return Ok((*primary_header, primary_array.to_vec()));
        } else if let Ok((primary_header, primary_array)) = primary_result.as_ref()
            && let Err(e) = backup_result.as_ref()
        {
            log!("Primary ok but backup corrupted: {:?}", e);
            return Ok((*primary_header, primary_array.to_vec()));
        } else if let Err(e) = primary_result
            && let Ok((secondary_header, secondary_array)) = backup_result
        {
            log!("Backup ok but primary corrupted: {:?}", e);
            return Ok((secondary_header, secondary_array));
        } else {
            log!("Both primary and backup GPT are corrupted");
            return Err(GPTErr::GPTCorrupted);
        }
    }

    pub async fn add_entry(
        &mut self,
        name: [u16; 36],
        start_lba: u64,
        end_lba: u64,
        type_guid: Guid,
        flags: u64,
    ) -> Result<u32, GPTErr> {
        log!(
            "Adding GPT entry: start={}, end={}, flags={}",
            start_lba,
            end_lba,
            flags
        );
        if !self.is_gpt_present().await {
            log!("No GPT present when attempting to add entry");
            return Err(GPTErr::GPTNonExist);
        }

        let (mut header, mut entries) = self.read_gpt().await?;

        // Find first empty slot
        let empty_index = entries
            .iter()
            .position(|entry| entry.is_empty())
            .ok_or(GPTErr::NoFreeSlot)?;

        log!("Found empty GPT slot at index={}", empty_index);

        // Check for overlapping partitions
        for (i, entry) in entries.iter().enumerate() {
            if i == empty_index || entry.is_empty() {
                continue;
            }

            let entry_start = entry.start_lba;
            let entry_end = entry.end_lba;

            // Check if ranges overlap
            if (start_lba >= entry_start && start_lba <= entry_end)
                || (end_lba >= entry_start && end_lba <= entry_end)
                || (start_lba <= entry_start && end_lba >= entry_end)
            {
                log!(
                    "Requested partition overlaps existing one at index={}: {}-{}",
                    i,
                    entry_start,
                    entry_end
                );
                return Err(GPTErr::OverlappingPartition);
            }
        }

        // Validate LBA range
        if start_lba < header.first_usable_block || end_lba > header.last_usable_block {
            log!(
                "Requested LBA range {}-{} outside usable range {}-{}",
                start_lba,
                end_lba,
                header.first_usable_block,
                header.last_usable_block
            );
            return Err(GPTErr::InvalidLBARange);
        }

        if start_lba >= end_lba {
            log!(
                "Invalid LBA range: start >= end ({} >= {})",
                start_lba,
                end_lba
            );
            return Err(GPTErr::InvalidLBARange);
        }

        // Generate unique partition GUID
        let unique_guid = Guid::new();

        // Create new entry
        let new_entry = GPTEntry {
            type_guid,
            unique_guid,
            start_lba,
            end_lba,
            flags,
            name: name,
        };

        entries[empty_index] = new_entry;

        // Serialize entries to buffer (each entry is 128 bytes)
        let mut array_buf = vec![0u8; (header.entry_num * header.entry_size) as usize];
        for (i, entry) in entries.iter().enumerate() {
            let entry_buf = entry.to_buf();
            let start = i * header.entry_size as usize;
            let copy_len = entry_buf.len().min(header.entry_size as usize);
            array_buf[start..start + copy_len].copy_from_slice(&entry_buf[..copy_len]);
            // Remaining bytes stay as zeros (padding)
        }

        // Update header CRCs
        header.array_crc32 = crypto::crc32::full_crc(&array_buf);
        header.header_crc32 = 0; // Must be zero before calculating
        header.header_crc32 = crypto::crc32::full_crc(&header.to_buf());

        log!(
            "Writing updated GPT table with new entry at index={}",
            empty_index
        );
        // Write updated table
        self.write_table(&header.to_buf_full(), &array_buf).await?;

        log!("Successfully added GPT entry at index={}", empty_index);
        Ok(empty_index as u32)
    }

    pub async fn delete_entry(&mut self, index: u32) -> Result<(), GPTErr> {
        log!("Deleting GPT entry at index={}", index);
        if !self.is_gpt_present().await {
            log!("No GPT present when attempting to delete entry");
            return Err(GPTErr::GPTNonExist);
        }

        let (mut header, mut entries) = self.read_gpt().await?;

        // Validate index
        if index >= header.entry_num {
            log!("Invalid entry index: {} >= {}", index, header.entry_num);
            return Err(GPTErr::InvalidEntryIndex);
        }

        let entry = &entries[index as usize];

        // Check if entry is already empty
        if entry.is_empty() {
            log!("Entry at index={} is already empty", index);
            return Err(GPTErr::EntryAlreadyEmpty);
        }

        log!("Clearing GPT entry at index={}", index);
        // Clear the entry
        entries[index as usize] = GPTEntry::empty();

        // Serialize entries to buffer (each entry is 128 bytes)
        let mut array_buf = vec![0u8; (header.entry_num * header.entry_size) as usize];
        for (i, entry) in entries.iter().enumerate() {
            let entry_buf = entry.to_buf();
            let start = i * header.entry_size as usize;
            let copy_len = entry_buf.len().min(header.entry_size as usize);
            array_buf[start..start + copy_len].copy_from_slice(&entry_buf[..copy_len]);
            // Remaining bytes stay as zeros (padding)
        }

        // Update header CRCs
        header.array_crc32 = crypto::crc32::full_crc(&array_buf);
        header.header_crc32 = 0; // Must be zero before calculating
        header.header_crc32 = crypto::crc32::full_crc(&header.to_buf());

        log!("Writing updated GPT table after delete");
        // Write updated table
        self.write_table(&header.to_buf_full(), &array_buf).await?;

        log!("Successfully deleted GPT entry at index={}", index);
        Ok(())
    }

    // TODO: edit entry
}

#[cfg(test)]
mod tests {
    use crate::{end_test, ignore, test_name};

    #[test_case]
    #[allow(unreachable_code)]
    fn gptheader() {
        ignore!();
        test_name!("gpt header serialization");
        end_test!();
    }

    #[test_case]
    #[allow(unreachable_code)]
    fn gpt_present() {
        ignore!();
        test_name!("tests for is_gpt_present");
        end_test!();
    }
}
