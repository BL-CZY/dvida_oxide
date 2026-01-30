use core::fmt;

use alloc::{format, string::String};

#[derive(PartialEq, Eq, Clone, Copy, Default, Debug, PartialOrd)]
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

impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl Guid {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{end_test, test_name};

    #[test_case]
    fn guid() {
        test_name!("guid struct");
        let buf: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let guid = Guid::from_buf(&buf);
        assert_eq!(buf, guid.to_buf());
        end_test!();
    }
}
