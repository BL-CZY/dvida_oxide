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
        write!(f, "{}", self.to_string())
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

        let whole = data1 << 96 | data2 << 80 | data3 << 64 | data4 as u128;

        let data4: [u8; 8] = data4.to_be_bytes();

        Some(Self {
            whole,
            data1: data1 as u32,
            data2: data2 as u16,
            data3: data3 as u16,
            data4,
        })
    }

    pub fn from_u128(val: u128) -> Self {
        Self {
            whole: val,
            data1: (val >> 96) as u32,
            data2: (val >> 80) as u16,
            data3: (val >> 64) as u16,
            data4: (val as u64).to_be_bytes(),
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
