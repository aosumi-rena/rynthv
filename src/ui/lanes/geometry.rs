use crate::{model::Parameter, ui::state::Lane};

pub(super) fn range(parameter: &Parameter, zoom: f32) -> (f64, f64) {
    let (low, high) = match parameter {
        Parameter::Pitch => (-300.0, 300.0),
        Parameter::Loudness => (-12.0, 12.0),
        Parameter::ToneShift => (-400.0, 400.0),
        Parameter::Vibrato => (0.0, 2.0),
        Parameter::Rap => (0.0, 1.0),
        Parameter::VocalMode(_) => crate::synthesis::experiments::ExperimentalSettings::ui().style_range(),
        Parameter::Experimental {..} => parameter.range(),
        _ => (-1.0, 1.0),
    };
    let factor=if matches!(parameter,Parameter::Loudness|Parameter::Tension|Parameter::Breathiness|Parameter::Voicing|Parameter::Gender|Parameter::ToneShift|Parameter::Mouth) {
        f64::from(crate::synthesis::experiments::ExperimentalSettings::ui().expression_factor())
    }else{1.0};
    let (low,high)=(low*factor,high*factor);
    let center = (low + high) * 0.5;
    let half = (high - low) * 0.5 * 100.0 / f64::from(zoom);
    (center - half, center + half)
}
pub(super) fn lane_range(lane: &Lane) -> (f64, f64) {
    range(&lane.foreground, lane.zoom)
}
pub(super) fn labels(parameter: &Parameter, low: f64, high: f64) -> (String, String) {
    match parameter {
        Parameter::Pitch | Parameter::ToneShift => {
            (format!("{low:+.0} ′"), format!("{high:+.0} ′"))
        }
        Parameter::Loudness => (format!("{low:+.0} dB"), format!("{high:+.0} dB")),
        Parameter::Vibrato => (format!("{low:.1} x"), format!("{high:.1} x")),
        Parameter::Tension => (crate::i18n::t("放松").into(), crate::i18n::t("紧张").into()),
        Parameter::Breathiness => (crate::i18n::t("清晰").into(), crate::i18n::t("气声").into()),
        Parameter::Gender => (crate::i18n::t("女性").into(), crate::i18n::t("男性").into()),
        Parameter::Voicing => (crate::i18n::t("无声").into(), crate::i18n::t("有声").into()),
        Parameter::Mouth => (crate::i18n::t("闭合").into(), crate::i18n::t("张开").into()),
        Parameter::VocalMode(_) => (format!("{low:.0}%"), format!("{high:.0}%")),
        Parameter::Experimental {key,..} if key=="growl_frequency" => (format!("{low:.0} Hz"),format!("{high:.0} Hz")),
        Parameter::Experimental {key,..} if key=="growl_amplitude" || key=="ddsDefaultStyle" || key.starts_with("ddsPhoneMix_") || key.starts_with("voiceBlend_") => (format!("{:.0}%",low*100.0),format!("{:.0}%",high*100.0)),
        Parameter::Experimental {..} => (format!("{low:.0}%"),format!("{high:.0}%")),
        Parameter::Rap => (String::new(), String::new()),
    }
}
