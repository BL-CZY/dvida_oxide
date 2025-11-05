use dvida_serialize::DvDeSer;

#[derive(DvDeSer)]
pub struct SuperBlock {
    number: u32,
    size: u32,
}

