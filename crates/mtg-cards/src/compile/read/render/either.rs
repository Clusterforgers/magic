//! A choice of one of two modes of one target that differ in a phrase of the sentence: "target creature gets +2/-2 or
//! -2/+2 until end of turn."

use super::alternatives;
use crate::effect::Mode;
use crate::refs::Bounds;

/// The sentences of a mode without a capital or a period, when it is a single sentence.
fn sentences(mode: &Mode) -> Vec<String> {
    alternatives(&mode.targets, &mode.effects)
        .into_iter()
        .filter_map(|mut sentences| (sentences.len() == 1).then(|| sentences.remove(0)))
        .collect()
}

/// The two sentences as one: the words they share before and after, and the phrases that differ joined by "or".
fn joined(first: &str, second: &str) -> Option<String> {
    let (a, b): (Vec<&str>, Vec<&str>) = (first.split(' ').collect(), second.split(' ').collect());
    let before = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let after = a[before..]
        .iter()
        .rev()
        .zip(b[before..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (middle_a, middle_b) = (&a[before..a.len() - after], &b[before..b.len() - after]);
    if before == 0 || middle_a.len() != 1 || middle_b.len() != 1 {
        return None;
    }
    let mut words: Vec<String> = a[..before].iter().map(|word| (*word).to_owned()).collect();
    words.push(format!("{} or {}", middle_a[0], middle_b[0]));
    words.extend(a[a.len() - after..].iter().map(|word| (*word).to_owned()));
    Some(words.join(" "))
}

/// The wordings of a choice of one of two modes with the same target: the phrase they differ in is the choice.
pub(super) fn texts(choose: &Bounds, modes: &[Mode]) -> Vec<String> {
    let [first, second] = modes else {
        return Vec::new();
    };
    if !choose.is_one() || first.targets.len() != 1 || first.targets != second.targets {
        return Vec::new();
    }
    let seconds = sentences(second);
    sentences(first)
        .iter()
        .flat_map(|a| seconds.iter().filter_map(|b| joined(a, b)))
        .collect()
}
