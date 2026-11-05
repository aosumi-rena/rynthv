//! Execute modern conditioning from score-derived scalars and named phones.
use rynthv::{
    model::{Language, NotePerformance},
    synthesis::{
        stages::f0::{
            condition::PitchSession,
            features::phonetic_context,
            frontend::{FrontendNote, NoteFrontend},
        },
        voice::{PhoneticMap, PreparedTree, TensorStorage},
    },
};
use serde_json::json;

pub(super) fn run(
    tree: &PreparedTree,
    frontend: usize,
    features: usize,
    notes: &[super::phonetic::Note],
    phones: &[super::phonetic::Phone],
    map: &PhoneticMap,
    phone_embeddings: &[f32],
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    if notes.is_empty() || notes.len() > 128 {
        return Err("Modern context checkpoint requires 1..128 note rows".into());
    }
    let root = tree.node(tree.root_index())?;
    let is_pitch = root.children.len() == 5;
    let mut notes = notes
        .iter()
        .map(|n| FrontendNote {
            phones: n.phones.clone(),
            pitch: n.pitch,
            pitch_offset: 0.0,
            duration_seconds: n.duration_seconds,
            frame_seconds: 0.005,
            kind: n.kind,
            flag: n.flag,
            tag: None,
            parts: 1,
            part_index: 0,
            appended_durations: vec![],
            performance: NotePerformance::default(),
        })
        .collect::<Vec<_>>();
    if is_pitch {
        let transpose = PitchSession::default().transposition(notes.iter().map(|n| n.pitch))?;
        for n in &mut notes {
            n.pitch_offset += transpose;
        }
    }
    let scalar = NoteFrontend::prepare(tree, frontend)?;
    let encoded = scalar.encode(&notes)?;
    let mut normalized = vec![0.0; encoded.values.len()];
    tree.range_normalizer(scalar.normalizer_node)?.normalize(
        &encoded.values,
        &mut normalized,
        true,
    )?;
    let feature = tree.node(features)?;
    if feature.children.len() != 10 {
        return Err("Modern feature ownership count differs".into());
    }
    let config = tree.payload(features)?;
    if config.len() != 16 {
        return Err("Modern feature configuration differs".into());
    }
    let style_width = u16::from_le_bytes([config[0], config[1]]) as usize;
    let mut projected = tree.execute_node(feature.children[2], &normalized)?;
    let style = if style_width > 0 {
        let TensorStorage::FloatVector { values } = tree.tensor(root.children[2])? else {
            return Err("Expected single-speaker F0 fixed style".into());
        };
        tree.execute_node(feature.children[3], &values.repeat(notes.len()))?
    } else {
        vec![]
    };
    let width = 512;
    if projected.len() != notes.len() * width
        || (!style.is_empty() && style.len() != projected.len())
    {
        return Err("Modern note projection shape differs".into());
    }
    let add = |output: &mut [f32], input: &[f32]| {
        for (a, b) in output.iter_mut().zip(input) {
            *a += b;
        }
    };
    let mut initial = projected.clone();
    if !style.is_empty() {
        add(&mut initial, &style);
    }
    let mut boundaries = Vec::new();
    boundaries.push(json!({"owner":"notes","label":"stack_input","frames":notes.len(),"channels":width,"values":initial}));
    let mut context = phonetic_context::encode_stack(
        tree,
        feature.children[5],
        &initial,
        width,
        |label, values| {
            boundaries.push(json!({"owner":"notes","label":label,"frames":notes.len(),"channels":width,"values":values}));
            Ok(())
        },
    )?;
    add(&mut context, &projected);
    let language_ids = phones
        .iter()
        .map(|p| {
            let language =
                Language::parse(&p.language).ok_or("Unknown diagnostic phonetic language")?;
            map.languages
                .iter()
                .position(|name| name == language.id())
                .ok_or("Phonetic language missing from original map")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let languages = tree.execute_node(
        feature.children[1],
        &language_ids.iter().map(|i| *i as f32).collect::<Vec<_>>(),
    )?;
    let phone_to_note = phones.iter().map(|p| p.note).collect::<Vec<_>>();
    if phone_to_note.iter().any(|i| *i >= notes.len())
        || phone_embeddings.len() != phones.len() * 32
        || languages.len() != phones.len() * 32
    {
        return Err("Modern phone context shape differs".into());
    }
    let mut combined = Vec::with_capacity(phones.len() * (width + 64));
    for (i, &note) in phone_to_note.iter().enumerate() {
        combined.extend_from_slice(&phone_embeddings[i * 32..(i + 1) * 32]);
        combined.extend_from_slice(&languages[i * 32..(i + 1) * 32]);
        combined.extend_from_slice(&context[note * width..(note + 1) * width]);
    }
    projected = tree.execute_node(feature.children[4], &combined)?;
    if projected.len() != phones.len() * width {
        return Err("Modern phone projection shape differs".into());
    }
    if !style.is_empty() {
        for (row, &note) in projected.chunks_exact_mut(width).zip(&phone_to_note) {
            add(row, &style[note * width..(note + 1) * width]);
        }
    }
    boundaries.push(json!({"owner":"phones","label":"stack_input","frames":phones.len(),"channels":width,"values":projected}));
    let phones = phonetic_context::encode_stack(
        tree,
        feature.children[6],
        &projected,
        width,
        |label, values| {
            boundaries.push(json!({"owner":"phones","label":label,"frames":phone_to_note.len(),"channels":width,"values":values}));
            Ok(())
        },
    )?;
    Ok(
        json!({"note_raw":encoded.values,"note_normalized":normalized,"language_ids":language_ids,
        "phone_to_note":phone_to_note,"boundaries":boundaries,"note_context":context,"phone_context":phones}),
    )
}
