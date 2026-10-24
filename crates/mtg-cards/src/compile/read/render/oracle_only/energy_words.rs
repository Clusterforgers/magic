//! Energy in words: "When this creature enters, you get six {E}." The renderers write a run of symbols for a fixed amount.

use crate::ability::Ability;
use crate::compile::read::amounts::count_word;

/// One energy symbol.
const SYMBOL: &str = "{E}";

/// Adds the wordings that name a run of two or more energy symbols by its number.
pub(super) fn add(_ability: &Ability, texts: &mut Vec<String>) {
    let found: Vec<String> = texts.iter().flat_map(|text| spelled(text)).collect();
    texts.extend(found);
}

/// The text with each run of energy symbols (two or more) said as a number, one run at a time.
fn spelled(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(start) = text[from..].find(SYMBOL).map(|at| at + from) {
        let mut end = start;
        let mut count = 0;
        while text[end..].starts_with(SYMBOL) {
            end += SYMBOL.len();
            count += 1;
        }
        if let Some(word) = (count >= 2).then(|| count_word(count)).flatten() {
            found.push(format!("{}{word} {SYMBOL}{}", &text[..start], &text[end..]));
        }
        from = end;
    }
    found
}
