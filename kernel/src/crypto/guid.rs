use core::fmt;

use alloc::{format, string::String};

#[derive(PartialEq, Eq, Clone, Copy, Default, PartialOrd)]
pub struct Guid {
    /// the entire guid in little endian
    pub whole: u128,
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    // the last two chunks of it in big endian
    pub data4: [u8; 8], // u16 & u48
}

impl Ord for Guid {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.whole.cmp(&other.whole)
    }
}

impl fmt::Debug for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} whole: 0x{:x}", self.to_string(), self.whole)
    }
}

impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl Guid {
    pub fn from_str(val: &str) -> Option<Self> {
        let mut parts = val.splitn(5, '-');

        let data1 = u32::from_str_radix(parts.next()?, 16).ok()? as u128;
        let data2 = u16::from_str_radix(parts.next()?, 16).ok()? as u128;
        let data3 = u16::from_str_radix(parts.next()?, 16).ok()? as u128;

        let data4_first = u64::from_str_radix(parts.next()?, 16).ok()?;
        let data4_second = u64::from_str_radix(parts.next()?, 16).ok()?;
        let data4 = data4_first << 48 | data4_second;

        let data4: [u8; 8] = data4.to_be_bytes();

        let data1_raw: [u8; 4] = (data1 as u32).to_le_bytes();
        let data2_raw: [u8; 2] = (data2 as u16).to_le_bytes();
        let data3_raw: [u8; 2] = (data3 as u16).to_le_bytes();

        let whole = u128::from_le_bytes([
            data1_raw[0],
            data1_raw[1],
            data1_raw[2],
            data1_raw[3],
            data2_raw[0],
            data2_raw[1],
            data3_raw[0],
            data3_raw[1],
            data4[0],
            data4[1],
            data4[2],
            data4[3],
            data4[4],
            data4[5],
            data4[6],
            data4[7],
        ]);

        Some(Self {
            whole,
            data1: data1 as u32,
            data2: data2 as u16,
            data3: data3 as u16,
            data4,
        })
    }

    pub fn from_bytes(val: [u8; 16]) -> Self {
        let data1 = u32::from_le_bytes([val[0], val[1], val[2], val[3]]);
        let data2 = u16::from_le_bytes([val[4], val[5]]);
        let data3 = u16::from_le_bytes([val[6], val[7]]);
        let data4 = [
            val[8], val[9], val[10], val[11], val[12], val[13], val[14], val[15],
        ];
        let whole = u128::from_le_bytes(val);

        Self {
            whole,
            data1,
            data2,
            data3,
            data4,
        }
    }

    pub fn to_string(&self) -> String {
        format!(
            "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            self.data1,
            self.data2,
            self.data3,
            self.data4[0],
            self.data4[1],
            self.data4[2],
            self.data4[3],
            self.data4[4],
            self.data4[5],
            self.data4[6],
            self.data4[7]
        )
    }
}
