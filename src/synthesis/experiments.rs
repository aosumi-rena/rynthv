mod controls;
mod dds;
pub use controls::*;
pub use dds::*;

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU16, Ordering};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExperimentalSettings {
    pub enabled: bool,
    pub extended_styles: bool,
    pub negative_styles: bool,
    pub extended_expression: bool,
    pub random_styles: bool,
    pub component_curves: bool,
    pub voice_blending: bool,
    pub component_effects: bool,
    pub growl: bool,
    pub dds_controls: bool,
    pub random_voices: bool,
}

static UI_FEATURES: AtomicU16 = AtomicU16::new(0);

impl ExperimentalSettings {
    pub fn style_range(self) -> (f64, f64) {
        let max = if self.enabled && self.extended_styles { 300.0 } else { 150.0 };
        (if self.enabled && self.negative_styles { -max } else { 0.0 }, max)
    }
    pub fn expression_factor(self) -> f32 {
        if self.enabled && self.extended_expression { 2.0 } else { 1.0 }
    }
    pub fn active(self, feature: bool) -> bool { self.enabled && feature }
    pub fn publish_ui(self) {
        let values = [self.enabled,self.extended_styles,self.negative_styles,self.extended_expression,
            self.random_styles,self.component_curves,self.voice_blending,self.component_effects,self.growl,self.dds_controls,self.random_voices];
        UI_FEATURES.store(values.into_iter().enumerate().fold(0,|bits,(i,v)|bits|((v as u16)<<i)),Ordering::Relaxed);
    }
    pub fn ui() -> Self {
        let bits=UI_FEATURES.load(Ordering::Relaxed);
        let flag=|i:u32|(bits & (1_u16<<i))!=0_u16;
        Self { enabled:flag(0),extended_styles:flag(1),negative_styles:flag(2),extended_expression:flag(3),
            random_styles:flag(4),component_curves:flag(5),voice_blending:flag(6),component_effects:flag(7),growl:flag(8),dds_controls:flag(9),random_voices:flag(10) }
    }
}

#[derive(Clone, Default)]
pub struct ExperimentalContext {
    pub settings: ExperimentalSettings,
    pub sources: Vec<BlendSource>,
}
impl std::fmt::Debug for ExperimentalContext {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {
        f.debug_struct("ExperimentalContext").field("settings",&self.settings)
            .field("sources",&self.sources.iter().map(|s|(&s.voice,&s.source_identity)).collect::<Vec<_>>()).finish()
    }
}
impl PartialEq for ExperimentalContext {
    fn eq(&self,other:&Self)->bool {
        self.settings==other.settings && self.sources.len()==other.sources.len()
            && self.sources.iter().zip(&other.sources).all(|(a,b)|a.voice==b.voice
                && a.source_identity==b.source_identity && a.identity_override==b.identity_override)
    }
}
#[derive(Clone)]
pub struct BlendSource {
    pub voice: super::settings::VoiceSelection,
    pub source_identity: String,
    pub name: String,
    pub default_language: String,
    pub supported_languages: Vec<String>,
    pub prepared: std::sync::Arc<super::voice::PreparedVoice>,
    pub identity_override: Option<u64>,
}
