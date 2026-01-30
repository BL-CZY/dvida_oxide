
    pub async fn create_gpt(&self, force: bool) -> Result<(), GPTErr> {
        log!("Creating GPT: force={}", force);
        if !force && self.is_gpt_present().await {
            log!("GPT already exists and force is false; aborting create_gpt");
            return Err(GPTErr::GPTAlreadyExist);
        }

        let pmbr = self.create_pmbr_buf().await;
        let mut header = self.create_unhashed_header().await;
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


    async fn write_table(&self, header: &[u8], array: &[u8]) -> Result<(), GPTErr> {
        let header: Buffer = header.into();
        let array: Buffer = array.into();

        log!("Writing GPT header to primary (LBA 1)");
        self.write_sectors_async(1, header.clone())
            .await
            .map_err(|e| {
                log!("Failed to write GPT header to primary: {}", e.to_string());
                GPTErr::Io(e.to_string())
            })?;

        log!("Writing GPT array to primary (LBA 2..)");
        self.write_sectors_async(2, array.clone())
            .await
            .map_err(|e| {
                log!("Failed to write GPT array to primary: {}", e.to_string());
                GPTErr::Io(e.to_string())
            })?;

        log!("Writing GPT header to backup");
        self.write_sectors_async(-1, header.clone())
            .await
            .map_err(|e| {
                log!("Failed to write GPT header to backup: {}", e.to_string());
                GPTErr::Io(e.to_string())
            })?;

        log!("Writing GPT array to backup");
        self.write_sectors_async(-33, array.clone())
            .await
            .map_err(|e| {
                log!("Failed to write GPT array to backup: {}", e.to_string());
                GPTErr::Io(e.to_string())
            })?;

        log!("GPT table write completed (primary + backup)");
        Ok(())
    }

    async fn create_pmbr_buf(&self) -> [u8; 512] {
        log!("Creating PMBR buffer");
        const PMBR_OFFSET: usize = 446;
        let mut result = [0u8; 512];

        result[PMBR_OFFSET + 1] = 0x0;
        result[PMBR_OFFSET + 2] = 0x2;
        result[PMBR_OFFSET + 3] = 0x0;
        result[PMBR_OFFSET + 4] = 0xEE;

        let (cylinder, head, sector) =
            crypto::lba_to_chs(self.sectors_per_track, self.sector_count);
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

        if self.sector_count > 0xFFFFFFFF {
            result[PMBR_OFFSET + 12] = 0xFF;
            result[PMBR_OFFSET + 13] = 0xFF;
            result[PMBR_OFFSET + 14] = 0xFF;
            result[PMBR_OFFSET + 15] = 0xFF;
        } else {
            let temp = self.sector_count as u32;
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

    async fn create_unhashed_header(&self) -> GPTHeader {
        let hdr = GPTHeader {
            backup_loc: self.sector_count - 1,
            last_usable_block: self.sector_count - 34,
            ..Default::default()
        };

        log!(
            "Created unhashed GPT header: backup_loc={}, last_usable_block={}",
            hdr.backup_loc,
            hdr.last_usable_block
        );

        hdr
    }

    async fn write_pmbr(&self, pmbr: &[u8; 512]) -> Result<(), GPTErr> {
        log!("Writing PMBR to sector 0");
        let pmbr: Buffer = pmbr.as_slice().into();

        self.write_sectors_async(0, pmbr).await.map_err(|e| {
            log!("Failed to write PMBR: {}", e.to_string());
            GPTErr::Io(e.to_string())
        })?;

        log!("PMBR write completed");
        Ok(())
    }

    pub async fn add_entry(
        &self,
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

    pub async fn delete_entry(&self, index: u32) -> Result<(), GPTErr> {
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

