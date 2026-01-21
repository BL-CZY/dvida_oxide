use lazy_static::lazy_static;
use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts};
use spin::Mutex;

use crate::iprint;

lazy_static! {
    static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
        Mutex::new(Keyboard::new(
            ScancodeSet1::new(),
            layouts::Us104Key,
            HandleControl::Ignore
        ));
}

pub fn read_scancode(scancode: u8) {
    let mut keyboard = KEYBOARD.lock();
    if let Ok(Some(key_evt)) = keyboard.add_byte(scancode) {
        if let Some(decoded_key) = keyboard.process_keyevent(key_evt) {
            match decoded_key {
                DecodedKey::Unicode(character) => iprint!("{}", character),
                DecodedKey::RawKey(key) => iprint!("{:?}", key),
            }
        }
    }
}
