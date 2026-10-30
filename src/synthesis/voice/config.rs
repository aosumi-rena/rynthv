use std::collections::BTreeMap;

use serde::Serialize;

use super::{Result, VoiceError, VoiceFamily, archive::ResourceId};

pub(super) const CONFIGURATION_ID: ResourceId = ResourceId([
    0x21, 0x19, 0x4a, 0xe9, 0x56, 0x77, 0x3e, 0x78, 0x8d, 0x0e, 0x8c, 0xe2, 0xd3, 0x26, 0xbc, 0x44,
]);

#[derive(Clone, Debug, Serialize)]
pub struct VoiceConfiguration {
    pub family: VoiceFamily,
    pub model_version: u8,
    pub f0_enhancement: Option<f32>,
    pub name: String,
    pub version: String,
    pub vendor: String,
    pub default_language: String,
    pub supported_languages: Vec<String>,
    pub styles: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hidden_styles: Vec<String>,
    #[serde(skip)]
    available_styles: Vec<String>,
    #[serde(skip)]
    styles_declared: bool,
    pub speakers: Vec<super::Speaker>,
    pub additional_fields: usize,
    pub parser_scope: &'static str,
    #[serde(skip)]
    _uninterpreted: BTreeMap<String, String>,
}

impl VoiceConfiguration {
    pub(super) fn from_materialized(name:String,version:String,vendor:String,language:String,f0:f32)->Result<Self> {
        let mut fields=BTreeMap::from([
            (".type".into(),"mu".into()),("f0model-configs".into(),"_ _ _ _ _ _ _ _".into()),
            (".name".into(),name),(".version".into(),version),(".vendor".into(),if vendor.is_empty(){"Flat".into()}else{vendor}),
            (".language".into(),language),(".feature_f0_enhancement".into(),f0.to_string()),
        ]);
        if fields[".version"].is_empty() {fields.insert(".version".into(),"0".into());}
        Self::from_nofs(fields)
    }

    pub fn additional_metadata(&self) -> &BTreeMap<String, String> { &self._uninterpreted }

    pub fn has_declared_vocal_modes(&self) -> bool { self.styles_declared }

    pub fn vocal_mode_names(&self) -> &[String] {
        &self.available_styles
    }

    pub fn is_hidden_vocal_mode(&self, name: &str) -> bool {
        self.hidden_styles.iter().any(|style| style == name)
    }

    pub(super) fn parse(bytes: &[u8]) -> Result<Self> {
        let text = std::str::from_utf8(bytes)
            .map_err(|_| VoiceError::new("Voice configuration is not UTF-8"))?;
        let mut fields = BTreeMap::new();

        for line in text.lines() {
            let (key, value) = line
                .split_once(' ')
                .filter(|(key, value)| !key.is_empty() && (!value.is_empty() || *key == "style_config"))
                .ok_or_else(|| VoiceError::new("Unsupported voice configuration line grammar"))?;
            if fields.insert(key.to_owned(), value.to_owned()).is_some() {
                return Err(VoiceError::new(
                    "Duplicate voice configuration field is unsupported",
                ));
            }
        }
        let model_version = match fields.remove("model-version").as_deref() {
            None | Some("1") => 1,
            Some("2") => 2,
            _ => return Err(VoiceError::new("Unsupported voice model-version")),
        };
        let name = take(&mut fields, "name")?;
        let version = take(&mut fields, "version")?;
        let vendor = take(&mut fields, "vendor")?;
        let default_language = take(&mut fields, "default_language")?;
        let supported_languages = take(&mut fields, "supported_languages")?
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();

        let declared_styles = fields.remove("style_config");
        let styles_declared = declared_styles.is_some();
        let styles: Vec<String> = match declared_styles {
            Some(styles) if model_version == 2 && styles.is_empty() => Vec::new(),
            Some(styles) => styles.split(',').map(str::to_owned).collect(),
            None => Vec::new(),
        };
        let speakers = fields
            .remove("subspeaker_config")
            .map(|text| super::Speaker::parse(&text))
            .transpose()?
            .unwrap_or_default();
        if !supported_languages.contains(&default_language) || styles.iter().any(String::is_empty) {
            return Err(VoiceError::new("Invalid voice language or style metadata"));
        }
        if styles.iter().any(|style| style.contains('=')) {
            return Err(VoiceError::new(
                "Extended voice style configuration grammar is not implemented",
            ));
        }
        if model_version == 2 && styles.iter().enumerate().any(|(i, name)| styles[..i].contains(name)) {
            return Err(VoiceError::new("Duplicate declared vocal-mode name"));
        }
        Ok(Self {
            family: if model_version == 1 {
                VoiceFamily::Sv2Compatible
            } else {
                VoiceFamily::Sv2
            },
            model_version,
            f0_enhancement: None,
            name,
            version,
            vendor,
            default_language,
            supported_languages,
            available_styles: styles.clone(),
            styles,
            hidden_styles: Vec::new(),
            styles_declared,
            speakers,
            additional_fields: fields.len(),
            parser_scope: "observed model-version 1/2 key-space-value grammar; extended style grammar and duplicate policy not recovered",
            _uninterpreted: fields,
        })
    }

    pub(super) fn resolve_embedded_styles(&mut self, basis_names: &[String]) -> Result<()> {
        if !self.styles_declared {
            self.styles = basis_names.to_vec();

            self.styles.sort();
        }
        self.hidden_styles = basis_names.iter()
            .filter(|name| !self.styles.contains(name)).cloned().collect();
        self.available_styles = self.styles.clone();
        self.available_styles.extend(self.hidden_styles.iter().cloned());
        Ok(())
    }

    pub(super) fn from_nofs(mut fields: BTreeMap<String, String>) -> Result<Self> {
        if take(&mut fields, ".type")? != "mu" {
            return Err(VoiceError::new(
                "暂不支持该 SV1 声库类型，仅支持 DDS AI 声库",
            ));
        }
        if take(&mut fields, "f0model-configs")? != "_ _ _ _ _ _ _ _" {
            return Err(VoiceError::new("暂不支持该 SV1 声库的自定义 F0 模型配置"));
        }
        let name = take(&mut fields, ".name")?;
        let version = take(&mut fields, ".version")?;
        let vendor = take(&mut fields, ".vendor")?;
        let default_language = take(&mut fields, ".language")?;
        let supported_languages = match fields.remove(".multi") {
            Some(value) => value.split_whitespace().map(str::to_owned).collect(),
            None => vec![default_language.clone()],
        };
        let styles: Vec<String> = fields
            .remove(".timbre_styles")
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        let f0_enhancement = fields
            .remove(".feature_f0_enhancement")
            .map(|value| {
                if value.is_empty() {
                    Ok(1.0)
                } else {
                    value
                        .parse::<f32>()
                        .map_err(|_| VoiceError::new("SV1 F0 enhancement is invalid"))
                }
            })
            .transpose()?;
        if name.is_empty()
            || vendor.is_empty()
            || version.parse::<super::VoiceRevision>().is_err()
            || !supported_languages.contains(&default_language)
            || f0_enhancement.is_some_and(|value| !value.is_finite() || value < 0.0)
        {
            return Err(VoiceError::new("SV1 voice configuration is invalid"));
        }
        Ok(Self {
            family: VoiceFamily::Sv1,
            model_version: 1,
            f0_enhancement,
            name,
            version,
            vendor,
            default_language,
            supported_languages,
            available_styles: styles.clone(),
            styles,
            hidden_styles: Vec::new(),
            styles_declared: true,
            speakers: Vec::new(),
            additional_fields: fields.len(),
            parser_scope: "SV1 AI NOFS public metadata; DDS model family",
            _uninterpreted: fields,
        })
    }
}

#[cfg(test)]
mod mode_catalog_tests {
    use super::*;

    fn configuration(version: u8, styles: Option<&str>) -> Result<VoiceConfiguration> {
        let mut text = format!("model-version {version}\nname Test\nversion 1\nvendor Test\ndefault_language english\nsupported_languages english\n");
        if let Some(styles) = styles { text.push_str(&format!("style_config {styles}\n")); }
        VoiceConfiguration::parse(text.as_bytes())
    }

    fn basis() -> Vec<String> {
        ["Soulful", "Powerful", "Breathy", "Soft", "Resonant"].map(String::from).to_vec()
    }

    #[test]
    fn declared_catalogue_keeps_its_order_and_exposes_embedded_names() {
        let mut config = configuration(2, Some("Breathy,Resonant,Soulful,Powerful")).unwrap();
        config.resolve_embedded_styles(&basis()).unwrap();
        assert_eq!(config.styles, ["Breathy", "Resonant", "Soulful", "Powerful"]);
        assert!(!config.styles.iter().any(|name| name == "Soft"));
        assert_eq!(config.hidden_styles, ["Soft"]);
        assert_eq!(config.vocal_mode_names(), ["Breathy", "Resonant", "Soulful", "Powerful", "Soft"]);
    }

    #[test]
    fn empty_declaration_and_absent_declaration_discover_embedded_names() {
        let mut empty = configuration(2, Some("")).unwrap();
        empty.resolve_embedded_styles(&basis()).unwrap();
        assert!(empty.styles.is_empty());
        assert_eq!(empty.hidden_styles, basis());
        assert_eq!(empty.vocal_mode_names(), basis());
        let mut absent = configuration(2, None).unwrap();
        absent.resolve_embedded_styles(&basis()).unwrap();
        assert_eq!(absent.styles, ["Breathy", "Powerful", "Resonant", "Soft", "Soulful"]);
        assert_eq!(absent.vocal_mode_names(), absent.styles);
    }

    #[test]
    fn duplicate_and_extended_declared_syntax_is_not_silently_accepted() {
        assert!(configuration(2, Some("Soft,Soft")).is_err());
        assert!(configuration(2, Some("Soft=")).is_err());
    }

    #[test]
    fn declared_timbre_only_mode_is_not_rejected_by_f0_fallback() {
        let mut config = configuration(2, Some("TimbreOnly,Breathy")).unwrap();
        config.resolve_embedded_styles(&basis()).unwrap();
        assert_eq!(config.styles, ["TimbreOnly", "Breathy"]);
    }

    #[test]
    fn legacy_catalogue_preserves_declarations_and_discovers_embedded_names() {
        let mut config = configuration(1, Some("Legacy")).unwrap();
        config.resolve_embedded_styles(&basis()).unwrap();
        assert_eq!(config.styles, ["Legacy"]);
        assert_eq!(config.hidden_styles, basis());
        assert!(configuration(1, Some("")).is_err());
    }
}

fn take(fields: &mut BTreeMap<String, String>, key: &str) -> Result<String> {
    fields
        .remove(key)
        .ok_or_else(|| VoiceError::new(format!("Missing voice configuration field: {key}")))
}
