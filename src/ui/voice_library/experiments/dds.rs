use crate::{app::App,synthesis::{experiments::{DdsControls,ComponentOutput,PhonemeMix},settings::VoiceSelection,voice::VoiceFamily}};
use eframe::egui::*;

pub(super) fn show(app:&App,ui:&mut Ui,c:&mut DdsControls) {
    let family=app.voice_inventory().and_then(|v|v.configuration.as_ref()).map(|c|c.family);
    if family!=Some(VoiceFamily::Sv2) {ui.checkbox(&mut c.cross_model_vectors,crate::i18n::t("Allow cross-model speaker vectors"));}
    ui.checkbox(&mut c.default_as_style,crate::i18n::t("Treat default as a vocal mode"));
    if c.default_as_style {
        ui.label(crate::i18n::t("Default vocal mode weight"));
        ui.add(Slider::new(&mut c.default_weight,-3.0..=3.0));
    }
    let entries:Vec<_>=app.synthesis.library.catalog().into_iter().flat_map(|a|&a.entries)
        .filter(|e|e.family==family&&e.availability.is_loadable()).collect();
    ui.label(crate::i18n::t("Vocoder model"));
    let selected=c.vocoder.as_ref().and_then(|key|entries.iter().find(|e|e.database_id==key.database_id && e.revision==key.revision))
        .map(|e|format!("{} · {}",e.display_name,super::super::selector::source_hint(e)));
    ComboBox::from_id_salt("dds-vocoder").selected_text(selected.unwrap_or_else(||if c.vocoder.is_some() {crate::i18n::t("Not installed")}else{crate::i18n::t("Voice default")}.into())).show_ui(ui,|ui| {
        ui.selectable_value(&mut c.vocoder,None,crate::i18n::t("Voice default"));
        for entry in &entries {ui.selectable_value(&mut c.vocoder,Some(VoiceSelection{database_id:entry.database_id.clone(),revision:entry.revision}),format!("{} · {}",entry.display_name,super::super::selector::source_hint(entry))).on_hover_text(entry.directory.display().to_string());}
    });
    ui.label(crate::i18n::t("Audio output"));
    let label=|v|crate::i18n::t(match v {ComponentOutput::Complete=>"Complete",ComponentOutput::Harmonics=>"Harmonics",ComponentOutput::Aspiration=>"Aspiration"});
    ComboBox::from_id_salt("dds-output").selected_text(label(c.output)).show_ui(ui,|ui| {
        for value in [ComponentOutput::Complete,ComponentOutput::Harmonics,ComponentOutput::Aspiration] {ui.selectable_value(&mut c.output,value,label(value));}
    });
    ui.separator();ui.label(crate::i18n::t("Phoneme mixing"));
    let mut remove=None;
    for (index,slot) in c.phonemes.iter_mut().enumerate() {
        ui.push_id(index,|ui| {
            ui.horizontal(|ui| {ui.label(format!("{} {}",crate::i18n::t("Mix phoneme"),index+1));if ui.small_button("×").clicked() {remove=Some(index);}});
            ui.label(crate::i18n::t("Source phoneme (empty: all)"));ui.text_edit_singleline(&mut slot.source);
            ui.label(crate::i18n::t("Target phoneme"));ui.text_edit_singleline(&mut slot.token);
            ui.label(crate::i18n::t("Mix language"));
            let languages=app.voice_inventory().and_then(|v|v.configuration.as_ref()).map(|c|c.supported_languages.as_slice()).unwrap_or_default();
            ComboBox::from_id_salt("language").selected_text(if slot.language.is_empty() {crate::i18n::t("Follow phoneme")}else{&slot.language}).show_ui(ui,|ui| {
                ui.selectable_value(&mut slot.language,String::new(),crate::i18n::t("Follow phoneme"));
                for name in languages {ui.selectable_value(&mut slot.language,name.clone(),name);}
            });
            ui.add(Slider::new(&mut slot.weight,0.0..=1.0));
        });
    }
    if let Some(index)=remove {c.phonemes.remove(index);}
    if c.phonemes.len()<8 && ui.button(crate::i18n::t("Add phoneme mix")).clicked() {c.phonemes.push(PhonemeMix{id:uuid::Uuid::new_v4().to_string(),source:String::new(),token:String::new(),language:String::new(),weight:0.0});}
}
