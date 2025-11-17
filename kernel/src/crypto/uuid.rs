use core::fmt;

use crate::crypto::random::random_number;

/// Generates a random UUID v4
pub async fn uuid_v4() -> Uuid {
    // Get 4 random u32 values (128 bits total)
    let r1 = random_number().await;
    let r2 = random_number().await;
    let r3 = random_number().await;
    let r4 = random_number().await;

    // Convert to bytes
    let mut bytes = [0u8; 16];
    bytes[0..4].copy_from_slice(&r1.to_be_bytes());
    bytes[4..8].copy_from_slice(&r2.to_be_bytes());
    bytes[8..12].copy_from_slice(&r3.to_be_bytes());
    bytes[12..16].copy_from_slice(&r4.to_be_bytes());

    // Set version (4) in the most significant 4 bits of byte 6
    bytes[6] = (bytes[6] & 0x0f) | 0x40;

    // Set variant (RFC 4122) in the most significant 2 bits of byte 8
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    Uuid { bytes }
}

/// UUID structure
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Uuid {
    bytes: [u8; 16],
}

impl Uuid {
    /// Returns the UUID as a byte array
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.bytes
    }

    /// Returns the UUID as a hyphenated string
    pub fn to_hyphenated_string(&self) -> UuidString {
        UuidString::from_uuid(self)
    }
}

/// A stack-allocated UUID string (36 bytes for hyphenated format)
pub struct UuidString {
    bytes: [u8; 36],
}

impl UuidString {
    fn from_uuid(uuid: &Uuid) -> Self {
        let mut bytes = [0u8; 36];
        let hex = b"0123456789abcdef";

        let mut i = 0;
        for (idx, &byte) in uuid.bytes.iter().enumerate() {
            bytes[i] = hex[(byte >> 4) as usize];
            bytes[i + 1] = hex[(byte & 0x0f) as usize];
            i += 2;

            // Add hyphens at positions 8, 13, 18, 23
            if idx == 3 || idx == 5 || idx == 7 || idx == 9 {
                bytes[i] = b'-';
                i += 1;
            }
        }

        Self { bytes }
    }

    /// Returns the string as a byte slice
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.to_hyphenated_string();
        // Safety: we know these are valid ASCII hex digits and hyphens
        let str_slice = unsafe { core::str::from_utf8_unchecked(s.as_bytes()) };
        f.write_str(str_slice)
    }
}

impl fmt::Debug for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Uuid({})", self)
    }
}
