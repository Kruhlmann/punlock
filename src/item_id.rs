use std::convert::TryFrom;

use serde::{Deserialize, Serialize};

use crate::statics::UUID_V4_REGEX;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct UuidV4(String);

impl UuidV4 {
    fn is_valid(value: &str) -> bool {
        UUID_V4_REGEX.is_match(value)
    }
}

impl AsRef<str> for UuidV4 {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for UuidV4 {
    type Error = anyhow::Error;

    fn try_from(value: String) -> anyhow::Result<Self> {
        if Self::is_valid(&value) {
            Ok(Self(value))
        } else {
            anyhow::bail!("invalid item id {}", value)
        }
    }
}

impl TryFrom<&str> for UuidV4 {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> anyhow::Result<Self> {
        Self::try_from(value.to_string())
    }
}

impl From<UuidV4> for String {
    fn from(val: UuidV4) -> Self {
        val.0
    }
}

impl std::fmt::Display for UuidV4 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_uuids() {
        for case in [
            "11111111-2222-3333-4444-555555555555",
            "2f1e4b6a-9c3d-4a7e-8b2f-1c5d6e7f8a90",
            "00000000-0000-0000-0000-000000000000",
            "ABCDEF01-2345-6789-ABCD-EF0123456789",
        ] {
            let id = UuidV4::try_from(case).expect("should accept valid uuid");
            assert_eq!(id.as_ref(), case);
        }
    }

    #[test]
    fn rejects_invalid_uuids() {
        for case in [
            "",
            "item-uuid",
            "11111111222233334444555555555555",
            "11111111-2222-3333-4444-55555555555",
            "11111111-2222-3333-4444-5555555555555",
            "11111111-2222-3333-4444-55555555555g",
            "11111111 2222 3333 4444 555555555555",
            "z1111111-2222-3333-4444-555555555555",
        ] {
            assert!(
                UuidV4::try_from(case).is_err(),
                "should reject invalid uuid: {:?}",
                case
            );
        }
    }

    #[test]
    fn display_and_into_string_round_trip() {
        let id = UuidV4::try_from("11111111-2222-3333-4444-555555555555").unwrap();
        assert_eq!(id.to_string(), "11111111-2222-3333-4444-555555555555");
        let as_string: String = id.into();
        assert_eq!(as_string, "11111111-2222-3333-4444-555555555555");
    }

    #[test]
    fn deserializes_valid_uuid_from_string() {
        let id: UuidV4 = serde_json::from_str("\"11111111-2222-3333-4444-555555555555\"").unwrap();
        assert_eq!(id.as_ref(), "11111111-2222-3333-4444-555555555555");
    }

    #[test]
    fn deserialization_rejects_invalid_uuid() {
        assert!(serde_json::from_str::<UuidV4>("\"not-a-uuid\"").is_err());
    }
}
