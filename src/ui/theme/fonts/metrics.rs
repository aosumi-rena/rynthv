use eframe::egui::FontData;

pub(super) fn height_scale(font: &FontData) -> Option<f32> {
    let bytes = font.font.as_ref();
    let face = if bytes.get(..4)? == b"ttcf" {
        let count = read_u32(bytes, 8)?;
        if font.index >= count {
            return None;
        }
        let offset = 12usize.checked_add(usize::try_from(font.index).ok()?.checked_mul(4)?)?;
        usize::try_from(read_u32(bytes, offset)?).ok()?
    } else {
        if font.index != 0 {
            return None;
        }
        0
    };
    let signature = bytes.get(face..face.checked_add(4)?)?;
    if signature != b"\0\x01\0\0" && signature != b"OTTO" && signature != b"true" {
        return None;
    }
    let count = usize::from(read_u16(bytes, face.checked_add(4)?)?);
    let directory = face.checked_add(12)?;
    bytes.get(directory..directory.checked_add(count.checked_mul(16)?)?)?;
    let table = |tag: &[u8; 4]| -> Option<&[u8]> {
        for index in 0..count {
            let entry = directory.checked_add(index.checked_mul(16)?)?;
            if bytes.get(entry..entry.checked_add(4)?)? == tag {
                let start = usize::try_from(read_u32(bytes, entry.checked_add(8)?)?).ok()?;
                let length = usize::try_from(read_u32(bytes, entry.checked_add(12)?)?).ok()?;
                return bytes.get(start..start.checked_add(length)?);
            }
        }
        None
    };
    let units = f32::from(read_u16(table(b"head")?, 18)?);
    if !(16.0..=16384.0).contains(&units) {
        return None;
    }
    let os2 = table(b"OS/2");
    let typo = os2.and_then(|data| Some((read_i16(data, 68)?, read_i16(data, 70)?)));
    let use_typo = os2.and_then(|data| read_u16(data, 62)).is_some_and(|flags| flags & 0x80 != 0);
    let hhea = table(b"hhea").and_then(|data| Some((read_i16(data, 4)?, read_i16(data, 6)?)));
    let (ascent, descent) = if use_typo {
        typo?
    } else if let Some(pair) = hhea.filter(|&(ascent, descent)| ascent != 0 || descent != 0) {
        pair
    } else if let Some(pair) = typo.filter(|&(ascent, descent)| ascent != 0 || descent != 0) {
        pair
    } else {
        let data = os2?;
        let ascent = f32::from(read_u16(data, 74)?);
        let descent = f32::from(read_u16(data, 76)?);
        let span = ascent + descent;
        return (span > 0.0).then_some(units / span);
    };
    let span = f32::from(ascent).abs() + f32::from(descent).abs();
    (span > 0.0).then_some(units / span)
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_be_bytes(bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?))
}

fn read_i16(bytes: &[u8], offset: usize) -> Option<i16> {
    Some(i16::from_be_bytes(bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?))
}
