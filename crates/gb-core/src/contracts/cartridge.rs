#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibilityMode {
    Dmg,
    DmgCompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapperKind {
    RomOnly,
    Mbc1,
    Mbc3,
    Mbc5,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartridgeMetadata {
    pub title: String,
    pub rom_identity: String,
    pub mapper: MapperKind,
    pub compatibility: CompatibilityMode,
    pub ram_size_bytes: usize,
    pub has_battery: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatteryState {
    format_version: u32,
    ram: Vec<u8>,
    mapper_data: Vec<u8>,
}

impl BatteryState {
    #[must_use]
    pub const fn new(format_version: u32, ram: Vec<u8>, mapper_data: Vec<u8>) -> Self {
        Self {
            format_version,
            ram,
            mapper_data,
        }
    }

    #[must_use]
    pub const fn format_version(&self) -> u32 {
        self.format_version
    }

    #[must_use]
    pub fn ram(&self) -> &[u8] {
        &self.ram
    }

    #[must_use]
    pub fn mapper_data(&self) -> &[u8] {
        &self.mapper_data
    }
}
