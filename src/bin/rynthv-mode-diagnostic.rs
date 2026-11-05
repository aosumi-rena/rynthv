//! Bounded original-model F0 mode operand comparison, without rendering or fitting.
use rynthv::synthesis::{
    stages::f0::style::StyleBasis,
    voice::{VoiceFamily, VoiceInventory},
};
use serde_json::json;
use std::{fs, io::Write, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 4 {
        return Err("Usage: rynthv-mode-diagnostic VOICE MODE PERCENT NEW_REPORT".into());
    }
    let output = PathBuf::from(&args[3]);
    if output.exists() { return Err("Report already exists".into()); }
    let mode = args[1].to_str().ok_or("Mode must be UTF-8")?;
    let percent: f64 = args[2].to_str().ok_or("Percent must be UTF-8")?.parse()?;
    if !percent.is_finite() || !(0.0..=150.0).contains(&percent) {
        return Err("Percent must be finite in0..150".into());
    }
    let inventory = VoiceInventory::load(&PathBuf::from(&args[0]))?;
    let voice = inventory.prepare()?;
    if voice.family != VoiceFamily::Sv2 || !voice.speakers.is_empty() {
        return Err("Requires one AI2 speaker".into());
    }
    let tree = voice.model("f0")?;
    let node = tree.node(tree.root_index())?.children[1];
    let basis = StyleBasis::prepare(tree, node)?;
    let index = basis.names.iter().position(|name| name == mode)
        .ok_or("Mode absent in F0 basis")?;
    let mut weights = vec![0.0; basis.names.len()];
    weights[index] = (percent as f32) / 100.0_f32;
    let neutral = basis.neutral_frames(1)?;
    let row = basis.frame(&weights)?;
    let report = json!({"scope":"One original-weight F0 conditioning operand; not a model export",
        "mode":mode,"percent":percent,"weights":weights,"model_order":basis.names,
        "neutral":neutral,"neutral_words":neutral.iter().map(|v|v.to_bits()).collect::<Vec<_>>(),
        "row":row,"row_words":row.iter().map(|v|v.to_bits()).collect::<Vec<_>>()});
    let mut file = fs::OpenOptions::new().create_new(true).write(true).open(output)?;
    file.write_all(&serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}
