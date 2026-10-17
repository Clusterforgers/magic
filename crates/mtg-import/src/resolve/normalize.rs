//! Canonical comparison form for card names.

use unicode_normalization::UnicodeNormalization;

/// Whether `c` is a combining diacritical mark.
fn is_combining_mark(c: char) -> bool {
    ('\u{300}'..='\u{36F}').contains(&c)
}

fn fold_char(c: char, out: &mut String) {
    match c {
        '\u{200B}'..='\u{200D}' | '\u{2060}' | '\u{FEFF}' | '\u{AD}' => {}
        '\u{2019}' | '\u{2018}' | '\u{2BC}' | '\u{B4}' | '`' => out.push('\''),
        '\u{2010}'..='\u{2015}' | '\u{2212}' => out.push('-'),
        '\u{201C}' | '\u{201D}' => out.push('"'),
        '\u{E6}' | '\u{C6}' => out.push_str("ae"),
        '\u{153}' | '\u{152}' => out.push_str("oe"),
        '\u{DF}' => out.push_str("ss"),
        '\u{F8}' | '\u{D8}' => out.push('o'),
        '\u{111}' | '\u{110}' => out.push('d'),
        '\u{142}' | '\u{141}' => out.push('l'),
        c if c.is_whitespace() => out.push(' '),
        c => out.extend(c.to_lowercase()),
    }
}

fn collapse_spaces(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Rewrites any run of slashes between non-empty halves as ` // `, so `Fire/Ice` equals `Fire // Ice`.
fn normalize_slashes(text: &str) -> String {
    let mut halves = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' {
            while chars.next_if_eq(&'/').is_some() {}
            halves.push(collapse_spaces(&current));
            current.clear();
        } else {
            current.push(c);
        }
    }
    halves.push(collapse_spaces(&current));
    if halves.iter().any(String::is_empty) {
        // A dangling slash is probably a truncated name; leave it visible instead of repairing it.
        collapse_spaces(text)
    } else {
        halves.join(" // ")
    }
}

/// Folds case, accents, ligatures, typographic punctuation, whitespace and `//` separators so equal spellings compare equal.
pub fn normalize_name(name: &str) -> String {
    let mut folded = String::with_capacity(name.len());
    for c in name.nfd().filter(|&c| !is_combining_mark(c)) {
        fold_char(c, &mut folded);
    }
    normalize_slashes(&folded)
}

#[cfg(test)]
mod tests {
    //! Goal: name normalization equates spellings that denote the same card and never equates different cards.
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    #[rstest]
    #[case::identity("lightning bolt", "lightning bolt")]
    #[case::case("Lightning BOLT", "lightning bolt")]
    #[case::trim_and_collapse("  Lightning \t  Bolt\n", "lightning bolt")]
    #[case::nbsp("Lightning\u{a0}Bolt", "lightning bolt")]
    #[case::accents("Lim-Dûl's Vault", "lim-dul's vault")]
    #[case::accents_decomposed("Lim-Du\u{302}l's Vault", "lim-dul's vault")]
    #[case::jotun("Jötun Grunt", "jotun grunt")]
    #[case::seance("Séance", "seance")]
    #[case::ae_ligature("Æther Vial", "aether vial")]
    #[case::ae_spelled_out("Aether Vial", "aether vial")]
    #[case::oe_ligature("Cœur", "coeur")]
    #[case::curly_apostrophe("Urza\u{2019}s Tower", "urza's tower")]
    #[case::backtick_apostrophe("Urza`s Tower", "urza's tower")]
    #[case::en_dash("Will\u{2013}o\u{2013}Wisp", "will-o-wisp")]
    #[case::em_dash("Will\u{2014}o\u{2014}Wisp", "will-o-wisp")]
    #[case::zero_width("Light\u{200b}ning Bolt", "lightning bolt")]
    #[case::bom_inside("\u{feff}Lightning Bolt", "lightning bolt")]
    #[case::slashes_double("Fire // Ice", "fire // ice")]
    #[case::slashes_single("Fire/Ice", "fire // ice")]
    #[case::slashes_spaced_single("Fire / Ice", "fire // ice")]
    #[case::slashes_no_space("Fire//Ice", "fire // ice")]
    #[case::slashes_triple("Fire /// Ice", "fire // ice")]
    #[case::many_halves("Who/What/When/Where/Why", "who // what // when // where // why")]
    #[case::dangling_slash_kept("Fire //", "fire //")]
    #[case::leading_slash_kept("// Ice", "// ice")]
    #[case::broken_slashes_kept("Fire / / Ice", "fire / / ice")]
    #[case::empty("", "")]
    #[case::only_spaces("   ", "")]
    #[case::digits_untouched("1996 World Champion", "1996 world champion")]
    #[case::comma_untouched("Atraxa, Praetors' Voice", "atraxa, praetors' voice")]
    #[case::turkish_dotted_i("\u{130}stanbul", "istanbul")]
    #[case::sharp_s("Gro\u{df}", "gross")]
    #[case::cjk_untouched("\u{9f8d}", "\u{9f8d}")]
    fn folds_equivalent_spellings(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(normalize_name(input), expected);
    }

    #[rstest]
    #[case("Lightning Bolt", "Lightning Bolts")]
    #[case("Fire // Ice", "Ice // Fire")]
    #[case("Fire", "Fire // Ice")]
    #[case("Island", "Isle")]
    #[case("Sol Ring", "Solring")]
    fn keeps_different_names_different(#[case] a: &str, #[case] b: &str) {
        assert_ne!(normalize_name(a), normalize_name(b));
    }

    proptest! {
        #[test]
        fn is_idempotent(name in "\\PC{0,60}") {
            let once = normalize_name(&name);
            prop_assert_eq!(normalize_name(&once), once);
        }

        #[test]
        fn is_idempotent_on_slashy_names(name in "[A-Za-z /\u{e9}]{0,30}") {
            let once = normalize_name(&name);
            prop_assert_eq!(normalize_name(&once), once);
        }

        #[test]
        fn ignores_case_and_surrounding_space(name in "[A-Za-z][A-Za-z ,'-]{0,30}") {
            let shouted = format!("  {}\t", name.to_uppercase());
            prop_assert_eq!(normalize_name(&shouted), normalize_name(&name));
        }

        #[test]
        fn output_has_no_double_spaces_or_edge_space(name in "\\PC{0,60}") {
            let out = normalize_name(&name);
            prop_assert!(!out.contains("  "));
            prop_assert_eq!(out.trim(), out.as_str());
        }
    }
}
