use alloc::vec::Vec;

static mut I_TABLE: [u32; 256] = [0; 256];

pub fn initialize_crc32() {
    let i_polynomial: i32 = 0x04C11DB7;

    for i_codes in 0..=0xFFu32 {
        unsafe {
            I_TABLE[i_codes as usize] = reflect(i_codes, 8) << 24;

            for _ in 0..8 {
                let temp = if I_TABLE[i_codes as usize] & (1 << 31) != 0 {
                    i_polynomial
                } else {
                    0
                };

                I_TABLE[i_codes as usize] = (I_TABLE[i_codes as usize] << 1) ^ temp as u32;
            }

            I_TABLE[i_codes as usize] = reflect(I_TABLE[i_codes as usize], 32);
        }
    }
}

fn reflect(mut i_reflect: u32, c_char: i8) -> u32 {
    let mut i_val: u32 = 0;

    for i_pos in 0..(c_char + 1) {
        if (i_reflect & 0x1) != 0 {
            i_val |= 0x1 << (c_char - i_pos as i8);
        }
        i_reflect >>= 1;
    }

    i_val
}

fn partial_crc(i_crc: &mut u32, s_data: &Vec<u8>) {
    for data in s_data.iter() {
        *i_crc = (*i_crc >> 8) ^ unsafe { I_TABLE[((*i_crc & 0xFF) ^ (*data) as u32) as usize] };
    }
}

pub fn is_verified_crc32(arr: &Vec<u8>, crc32: u32) -> bool {
    if crc32 == full_crc(arr) {
        return true;
    }
    false
}

pub fn full_crc(s_data: &Vec<u8>) -> u32 {
    let mut ul_crc: u32 = 0xFFFFFFFF;
    partial_crc(&mut ul_crc, s_data);

    ul_crc ^ 0xFFFFFFFF
}
