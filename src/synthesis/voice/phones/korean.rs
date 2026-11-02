//! Korean phoneme spellings across model generations.
pub(super) fn local(xsampa: &str) -> Option<&'static str> {
    Some(match xsampa {
        "6_o" => "6", "e_o" => "e_o", "i-" => "i", "M" => "M",
        "o_o" => "o", "u" => "u", "V" => "V", "w" => "w",
        "M\\" => "M_", "j" => "j", "4" => "4", "b" => "b",
        "p_t" => "p_t", "ph" => "p", "d" => "d", "t_t" => "t_t",
        "th" => "t", "s" => "s", "s_t" => "s_t", "dz\\" => "dz\\",
        // Older tables combine these contextual variants.
        "s\\_t" => "s_t", "C" => "h",
        "ts\\_t" => "ts\\h", "ts\\h" => "ts\\_h", "g" => "g",
        "k_t" => "k_t", "kh" => "k", "h" => "h", "l" => "l",
        "m" => "m", "n" => "n", ":N" => "N",
        _ => return None,
    })
}
