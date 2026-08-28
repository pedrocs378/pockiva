use gb_core::{
    CartridgeMetadata, CompatibilityMode, CoreError, Frame, MapperKind, SCREEN_HEIGHT, SCREEN_WIDTH,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimePhase {
    Empty,
    Loading,
    Paused,
    Running,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeErrorCode {
    FileInaccessible,
    InvalidRom,
    CgbOnly,
    UnsupportedMapper,
    CoreFailure,
    InvalidLifecycle,
    RuntimeUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeMapper {
    RomOnly,
    Mbc1,
    Mbc3,
    Mbc5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeCompatibility {
    Dmg,
    DmgCompatible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeError {
    pub code: RuntimeErrorCode,
    pub message: String,
}

impl RuntimeError {
    pub fn new(code: RuntimeErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl From<CoreError> for RuntimeError {
    fn from(error: CoreError) -> Self {
        match error {
            CoreError::InvalidRom(reason) => Self::new(
                RuntimeErrorCode::InvalidRom,
                format!("Invalid ROM: {reason}"),
            ),
            CoreError::UnsupportedCgbOnlyCartridge => Self::new(
                RuntimeErrorCode::CgbOnly,
                "Game Boy Color-only cartridges are not supported.",
            ),
            CoreError::UnsupportedMapper(mapper) => Self::new(
                RuntimeErrorCode::UnsupportedMapper,
                format!("Unsupported cartridge mapper: {mapper:#04x}."),
            ),
            CoreError::NotLoaded => {
                Self::new(RuntimeErrorCode::InvalidLifecycle, "No ROM is loaded.")
            }
            CoreError::InternalInvariant(reason) => Self::new(
                RuntimeErrorCode::CoreFailure,
                format!("Emulator core failure: {reason}"),
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RomSummary {
    pub title: String,
    pub file_name: String,
    pub rom_identity: String,
    pub mapper: RuntimeMapper,
    pub compatibility: RuntimeCompatibility,
}

impl RomSummary {
    pub fn from_metadata(metadata: CartridgeMetadata, file_name: String) -> Self {
        Self {
            title: metadata.title,
            file_name,
            rom_identity: metadata.rom_identity,
            mapper: metadata.mapper.into(),
            compatibility: metadata.compatibility.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSnapshot {
    pub phase: RuntimePhase,
    pub rom: Option<RomSummary>,
    pub error: Option<RuntimeError>,
}

impl RuntimeSnapshot {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            phase: RuntimePhase::Empty,
            rom: None,
            error: None,
        }
    }

    #[cfg(test)]
    fn loaded_for_test(
        phase: RuntimePhase,
        title: &str,
        file_name: &str,
        rom_identity: &str,
        mapper: RuntimeMapper,
        compatibility: RuntimeCompatibility,
    ) -> Self {
        Self {
            phase,
            rom: Some(RomSummary {
                title: title.into(),
                file_name: file_name.into(),
                rom_identity: rom_identity.into(),
                mapper,
                compatibility,
            }),
            error: None,
        }
    }
}

impl Default for RuntimeSnapshot {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeButton {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    Start,
    Select,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum RuntimeEvent {
    Snapshot { snapshot: RuntimeSnapshot },
}

pub type RuntimeResult<T> = Result<T, RuntimeError>;
pub const FRAME_HEADER_BYTE_LENGTH: usize = 12;
pub const FRAME_RGBA_BYTE_LENGTH: usize = SCREEN_WIDTH * SCREEN_HEIGHT * 4;
pub const FRAME_PACKET_BYTE_LENGTH: usize = FRAME_HEADER_BYTE_LENGTH + FRAME_RGBA_BYTE_LENGTH;

impl From<MapperKind> for RuntimeMapper {
    fn from(mapper: MapperKind) -> Self {
        match mapper {
            MapperKind::RomOnly => Self::RomOnly,
            MapperKind::Mbc1 => Self::Mbc1,
            MapperKind::Mbc3 => Self::Mbc3,
            MapperKind::Mbc5 => Self::Mbc5,
        }
    }
}

impl From<CompatibilityMode> for RuntimeCompatibility {
    fn from(compatibility: CompatibilityMode) -> Self {
        match compatibility {
            CompatibilityMode::Dmg => Self::Dmg,
            CompatibilityMode::DmgCompatible => Self::DmgCompatible,
        }
    }
}

#[must_use]
pub fn encode_frame_packet(frame: &Frame) -> Vec<u8> {
    let mut packet = Vec::with_capacity(FRAME_PACKET_BYTE_LENGTH);
    packet.extend_from_slice(&frame.sequence().to_le_bytes());
    packet.extend_from_slice(
        &u16::try_from(SCREEN_WIDTH)
            .expect("screen width fits u16")
            .to_le_bytes(),
    );
    packet.extend_from_slice(
        &u16::try_from(SCREEN_HEIGHT)
            .expect("screen height fits u16")
            .to_le_bytes(),
    );
    packet.extend_from_slice(frame.rgba());
    debug_assert_eq!(packet.len(), FRAME_PACKET_BYTE_LENGTH);
    packet
}

#[cfg(test)]
mod tests {
    use gb_core::Frame;
    use serde_json::json;

    use super::{
        FRAME_PACKET_BYTE_LENGTH, RuntimeButton, RuntimeCompatibility, RuntimeEvent, RuntimeMapper,
        RuntimePhase, RuntimeSnapshot, encode_frame_packet,
    };

    #[test]
    fn runtime_snapshot_serializes_for_the_frontend_contract() {
        let snapshot = RuntimeSnapshot::loaded_for_test(
            RuntimePhase::Paused,
            "Test Cart",
            "test.gb",
            "sha256:test",
            RuntimeMapper::RomOnly,
            RuntimeCompatibility::Dmg,
        );

        assert_eq!(
            serde_json::to_value(RuntimeEvent::Snapshot { snapshot })
                .expect("runtime event serializes"),
            json!({
                "type": "snapshot",
                "snapshot": {
                    "phase": "paused",
                    "rom": {
                        "title": "Test Cart",
                        "fileName": "test.gb",
                        "romIdentity": "sha256:test",
                        "mapper": "rom-only",
                        "compatibility": "dmg"
                    },
                    "error": null
                }
            })
        );
    }

    #[test]
    fn runtime_buttons_serialize_as_lowercase_values() {
        let buttons = [
            RuntimeButton::Up,
            RuntimeButton::Down,
            RuntimeButton::Left,
            RuntimeButton::Right,
            RuntimeButton::A,
            RuntimeButton::B,
            RuntimeButton::Start,
            RuntimeButton::Select,
        ];
        assert_eq!(
            serde_json::to_value(buttons).expect("buttons serialize"),
            json!(["up", "down", "left", "right", "a", "b", "start", "select"])
        );
    }

    #[test]
    fn frame_packet_has_the_stable_raw_binary_layout() {
        let rgba = vec![0x5a; 92_160];
        let frame = Frame::new(0x0102_0304_0506_0708, rgba.clone()).expect("valid frame");

        let packet = encode_frame_packet(&frame);

        assert_eq!(packet.len(), FRAME_PACKET_BYTE_LENGTH);
        assert_eq!(&packet[0..8], &[8, 7, 6, 5, 4, 3, 2, 1]);
        assert_eq!(&packet[8..10], &[160, 0]);
        assert_eq!(&packet[10..12], &[144, 0]);
        assert_eq!(&packet[12..], rgba);
    }
}
