pub fn features(names: &[String]) -> Result<[f32; 16], String> {
    let mut result = [0.0; 16];
    for name in names {
        let (channel, value) = match name.as_str() {
            "Slur" => (0, 1.0),
            "Accent" => (1, 1.0),
            "Staccato" => (2, 1.0),
            "Tenuto" => (3, 1.0),
            "Trill Minor" => (4, 1.0),
            "Trill Major" => (4, 2.0),
            "Tremolo" => (5, 1.0),
            "Fall" => (6, 1.0),
            "Breath" => (7, 1.0),
            "Harmonics" => (11, 1.0),
            _ => return Err(format!("Unsupported instrument articulation: {name}")),
        };
        result[channel] = value;
    }
    Ok(result)
}
