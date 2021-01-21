
#[non_exhaustive]
pub enum Instruction {
    BitmapDescriptor(usize),
    BitmapOutlineDescriptor(usize),
    ScalableDescriptor(usize),
}