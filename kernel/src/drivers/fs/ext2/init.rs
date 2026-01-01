use alloc::boxed::Box;
use dvida_serialize::DvDeserialize;
use terminal::log;

use crate::{
    drivers::fs::ext2::SuperBlock,
    hal::{gpt::GPTEntry, storage::read_sectors},
};

pub async fn identify_ext2(drive_id: usize, entry: &GPTEntry) -> Option<SuperBlock> {
    let mut buf: Box<[u8]> = Box::new([0u8; 1024]);

    if entry.start_lba - entry.end_lba < 3 {
        log!("Failed to identify ext2 because the GPT entry is too small");
        return None;
    }

    match read_sectors(drive_id, buf, (entry.start_lba + 2) as i64).await {
        Ok(b) => buf = b,
        Err(err) => {
            log!("Failed to identify ext2 because of read error: {}", err);
            return None;
        }
    }

    let super_block =
        match SuperBlock::deserialize(dvida_serialize::Endianness::Little, buf.as_mut()) {
            Ok(res) => res.0,
            Err(e) => {
                log!(
                    "Failed to identify ext2 because of deserialization error: {:?}",
                    e
                );
                return None;
            }
        };

    log!("Read Superblock: {:?}", super_block);

    if super_block.s_magic == 0xEF53 {
        log!("Found superblock");
        Some(super_block)
    } else {
        log!("Didn't find superblock");
        None
    }
}
