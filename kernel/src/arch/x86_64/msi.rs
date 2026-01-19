use bitfield::bitfield;

bitfield! {
    pub struct MessageAddressRegister(u32);
    impl Debug;

    pub destination_id, set_destination_id: 19, 12;
    pub redirection_hint, set_redirection_hint: 3, 3;
    pub destination_mode, set_destination_mode: 2, 2;
}

impl MessageAddressRegister {
    pub fn new() -> Self {
        MessageAddressRegister(0xFEE00000)
    }
}

impl Default for MessageAddressRegister {
    fn default() -> Self {
        Self::new()
    }
}

bitfield! {
    pub struct MessageDataRegister(u32);
    impl Debug;

    pub vector, set_vector: 7, 0;
    pub delivery_mode, set_delivery_mode: 10, 8;
    pub level_for_trigger_mode, set_level_for_trigger_mode: 14, 14;
    pub trigger_mode, set_trigger_mode: 15, 15;
}
