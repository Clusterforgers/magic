//! A penalty to power alone with the zero signed as the cards sign it: "Target creature gets -2/-0 until end of turn."
//! The renderers write "-2/+0".

/// Adds each wording with a bonus "-N/+0" written "-N/-0".
pub(super) fn add(texts: &mut Vec<String>) {
    let extra: Vec<String> = texts
        .iter()
        .filter_map(|text| w12a_minus_zero(text))
        .collect();
    texts.extend(extra);
}

/// The wording with each "-N/+0" as "-N/-0"; `None` when it has none.
fn w12a_minus_zero(text: &str) -> Option<String> {
    let mut any = false;
    let words: Vec<String> = text
        .split(' ')
        .map(|word| {
            let negative = word
                .strip_prefix('-')
                .and_then(|rest| rest.split_once("/+0"))
                .map(|(power, _)| power);
            match negative {
                Some(power)
                    if !power.is_empty()
                        && power.chars().all(|c| c.is_ascii_digit() || c == 'X') =>
                {
                    any = true;
                    word.replacen(&format!("-{power}/+0"), &format!("-{power}/-0"), 1)
                }
                _ => word.to_owned(),
            }
        })
        .collect();
    any.then(|| words.join(" "))
}
