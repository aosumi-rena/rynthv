use super::super::{PhoneSet, ResolvedPhone, Result, VoiceError};

const PREFIX: &str = "phoneme:";

fn spelling(token: &str) -> Option<(&str, &str)> {
    let (length, value) = token.strip_prefix(PREFIX)?.split_once(':')?;
    let length = length.parse::<usize>().ok()?;
    (length > 0 && length < value.len() && value.is_char_boundary(length))
        .then(|| value.split_at(length))
}

pub fn symbol(token: &str) -> Option<&str> {
    spelling(token).map(|(symbol, _)| symbol)
}

pub fn token(symbol: &str, xsampa: &str) -> String {
    format!("{PREFIX}{}:{symbol}{xsampa}", symbol.len())
}

pub(in crate::synthesis::voice) fn resolve<'a>(
    index: usize, table: &'a PhoneSet, unified: &'a PhoneSet, token: &str,
) -> Result<ResolvedPhone<'a>> {
    let (symbol, xsampa) = spelling(token)
        .ok_or_else(|| VoiceError::new("Invalid resource phoneme identity"))?;
    let local = if table.name.ends_with("-xsampa-phones") {
        table.lookup(xsampa)
    } else {
        table.lookup(symbol)
    };
    let local = match local {
        Some(phone) => phone,
        None => {
            if table.name.ends_with("-phones") {
                return Err(VoiceError::new(format!(
                    "Model table {} has no resource phoneme {symbol} ({xsampa})", table.name)));
            }
            let mut matches = table.symbols.iter().filter_map(|symbol| table.lookup(symbol))
                .filter(|phone| phone.mapped_symbol == Some(xsampa));
            let phone = matches.next().ok_or_else(|| VoiceError::new(
                format!("Model table {} has no resource phoneme {symbol} ({xsampa})", table.name)))?;
            if matches.next().is_some() {
                return Err(VoiceError::new(format!(
                    "Model table {} has ambiguous resource phoneme {symbol} ({xsampa})", table.name)));
            }
            phone
        }
    };
    let mapped = local.mapped_symbol.unwrap_or(local.symbol);
    let output = unified.lookup(mapped).ok_or_else(|| VoiceError::new(
        format!("Model table {} references absent phoneme {mapped}", table.name)))?;
    Ok(ResolvedPhone {
        language_index: index,
        selected_phone_set: &table.name,
        local_stored_index: Some(local.stored_index),
        source_class: Some(local.class),
        unified_phone_index: output.stored_index,
        unified_symbol: output.symbol,
        used_silence_fallback: false,
    })
}
