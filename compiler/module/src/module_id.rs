#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModuleId(pub u32);

impl ModuleId {
    pub const ROOT: ModuleId = ModuleId(0);
}
