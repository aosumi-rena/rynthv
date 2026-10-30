use std::collections::BTreeMap;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(try_from = "ModeRepresentation")]
pub struct PublicModeControl {
    #[serde(default, deserialize_with = "optional_number", skip_serializing_if = "Option::is_none")]
    pub pitch: Option<f64>,
    #[serde(default, deserialize_with = "optional_number", skip_serializing_if = "Option::is_none")]
    pub timbre: Option<f64>,
    #[serde(default, deserialize_with = "optional_number", skip_serializing_if = "Option::is_none")]
    pub pronunciation: Option<f64>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ModeRepresentation {
    Shared(f64),
    Components(ModeComponents),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModeComponents {
    #[serde(default, deserialize_with = "optional_number")]
    pitch: Option<f64>,
    #[serde(default, deserialize_with = "optional_number")]
    timbre: Option<f64>,
    #[serde(default, deserialize_with = "optional_number")]
    pronunciation: Option<f64>,
}

impl TryFrom<ModeRepresentation> for PublicModeControl {
    type Error = String;
    fn try_from(value: ModeRepresentation) -> Result<Self, String> {
        let value = match value {
            ModeRepresentation::Shared(value) => Self {
                pitch: Some(value), timbre: Some(value), pronunciation: Some(value),
            },
            ModeRepresentation::Components(value) => Self {
                pitch: value.pitch, timbre: value.timbre, pronunciation: value.pronunciation,
            },
        };
        value.validate_storage()?;
        Ok(value)
    }
}

pub type PublicModeMap = BTreeMap<String, PublicModeControl>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicModeComponent { Pitch, Timbre, Pronunciation }

impl PublicModeControl {
    pub fn component(&self, component: PublicModeComponent) -> Option<f64> {
        match component {
            PublicModeComponent::Pitch => self.pitch,
            PublicModeComponent::Timbre => self.timbre,
            PublicModeComponent::Pronunciation => self.pronunciation,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        for value in [self.pitch, self.timbre, self.pronunciation].into_iter().flatten() {
            if !value.is_finite() || !(0.0..=150.0).contains(&value) {
                return Err("Vocal-mode public controls must be finite in 0..150".into());
            }
        }
        Ok(())
    }
    pub fn validate_storage(&self)->Result<(),String> {
        if [self.pitch,self.timbre,self.pronunciation].into_iter().flatten().any(|v|!v.is_finite() || !(-300.0..=300.0).contains(&v)) {
            return Err("Stored vocal modes must be finite in -300..300".into());
        }
        Ok(())
    }
}

fn optional_number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<f64>, D::Error> {
    f64::deserialize(deserializer).map(Some)
}

#[cfg(test)]
mod public_mode_tests {
    use super::*;

    #[test]
    fn absence_and_explicit_zero_stay_distinct() {
        let absent: PublicModeControl = serde_json::from_str("{}").unwrap();
        let zero: PublicModeControl = serde_json::from_str(r#"{"pitch":0}"#).unwrap();
        assert_eq!(absent.pitch, None);
        assert_eq!(zero.pitch, Some(0.0));
        assert_ne!(absent, zero);
        assert_eq!(serde_json::to_string(&absent).unwrap(), "{}");
        assert_eq!(serde_json::to_string(&zero).unwrap(), r#"{"pitch":0.0}"#);
    }

    #[test]
    fn object_schema_rejects_null_and_unknown_fields() {
        for json in [r#"{"gain":25}"#, r#"{"pitch":null}"#, r#"{"pitch":"25"}"#] {
            assert!(serde_json::from_str::<PublicModeControl>(json).is_err());
        }
    }

    #[test]
    fn public_domain_validation_does_not_convert_or_collapse_components() {
        let value = PublicModeControl { pitch: Some(0.0), timbre: Some(25.0), pronunciation: Some(150.0) };
        value.validate().unwrap();
        assert_eq!(value.component(PublicModeComponent::Timbre), Some(25.0));
        for invalid in [-1.0, 150.01, f64::NAN, f64::INFINITY] {
            assert!(PublicModeControl { pitch: Some(invalid), ..Default::default() }.validate().is_err());
        }
    }
}
