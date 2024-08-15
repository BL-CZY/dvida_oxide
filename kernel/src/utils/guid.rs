pub struct Guid {
    /// the entire guid in little endian
    whole: u128,
    first: u32,
    second: u16,
    third: u16,
    // the last two chunks of it in big endian
    last: u64, // u16 & u48
}

impl Guid {
    pub fn from_buf(buf: &[u8; 16]) -> Self {
        Guid {
            whole: u128::from_le_bytes(*buf).try_into().unwrap(),
            first: u32::from_le_bytes(buf[0..4].try_into().unwrap()),
            second: u16::from_le_bytes(buf[4..6].try_into().unwrap()),
            third: u16::from_le_bytes(buf[6..8].try_into().unwrap()),
            last: u64::from_be_bytes(buf[8..16].try_into().unwrap()),
        }
    }

    pub fn to_buf(&self) -> [u8; 16] {
        let first = self.first.to_le_bytes();
        let second = self.second.to_le_bytes();
        let third = self.third.to_le_bytes();
        let last = self.last.to_be_bytes();
        let mut res: [u8; 16] = [0; 16];

        for (index, byte) in first.iter().enumerate() {
            res[index] = *byte;
        }

        res[4] = second[0];
        res[5] = second[1];
        res[6] = third[0];
        res[7] = third[1];

        for (index, byte) in last.iter().enumerate() {
            res[8 + index] = *byte;
        }

        res
    }

    pub fn new() -> Self {
        Guid {
            // TODO dummy
            whole: 0,
            first: 0,
            second: 0,
            third: 0,
            last: 0,
        }
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
