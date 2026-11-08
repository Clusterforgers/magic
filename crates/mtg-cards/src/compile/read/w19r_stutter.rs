//! A word the writers say twice in a row ("also also", "Then then", "you you") is said once: no oracle text repeats these.

/// The words a writer may repeat by slip; other doubled words ("had had") can be real text.
const SLIPS: [&str; 8] = ["also", "then", "you", "and", "or", "the", "a", "to"];

/// The text with each doubled slip word said once, or none when no word is doubled.
pub(super) fn once(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<&str> = Vec::with_capacity(words.len());
    for word in &words {
        let doubled = out.last().is_some_and(|last| {
            last.eq_ignore_ascii_case(word) && SLIPS.contains(&word.to_ascii_lowercase().as_str())
        });
        if !doubled {
            out.push(word);
        }
    }
    (out.len() != words.len()).then(|| out.join(" "))
}
