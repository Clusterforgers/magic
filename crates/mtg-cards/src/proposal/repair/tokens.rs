//! A creature token with only one of power and toughness takes the other from the printed `N/N` of the face when the
//! text states it once for that size.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use super::{Repair, RepairClass};

/// Every `N/N` of the text that is a size: not preceded by a sign (`+1/+1`) and not followed by one (`1/-1`).
fn sizes(text: &str) -> Vec<(i64, i64)> {
    let chars: Vec<char> = text.chars().collect();
    let number = |from: usize| {
        let digits: String = chars[from..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        (digits.parse::<i64>().ok(), from + digits.len())
    };
    let mut found = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let signed = at > 0 && matches!(chars[at - 1], '+' | '-' | '\u{2212}');
        let inside = at > 0 && chars[at - 1].is_ascii_digit();
        if !chars[at].is_ascii_digit() || signed || inside {
            at += 1;
            continue;
        }
        let (power, next) = number(at);
        at = next;
        if chars.get(next) == Some(&'/') && chars.get(next + 1).is_some_and(char::is_ascii_digit) {
            let (toughness, end) = number(next + 1);
            if let (Some(power), Some(toughness)) = (power, toughness) {
                found.push((power, toughness));
            }
            at = end;
        }
    }
    found
}

/// The only value `wanted` can take among the sizes whose other part is `known`.
fn only(text: &str, known: i64, power_known: bool) -> Option<i64> {
    let values: BTreeSet<i64> = sizes(text)
        .into_iter()
        .filter_map(|(power, toughness)| {
            if power_known {
                (power == known).then_some(toughness)
            } else {
                (toughness == known).then_some(power)
            }
        })
        .collect();
    match values.len() {
        1 => values.into_iter().next(),
        _ => None,
    }
}

/// Fills the missing half of the size of each creature token of the ability.
fn fill(value: &mut Value, text: &str, at: &str, repairs: &mut Vec<Repair>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::Object(token)) = map.get_mut("Custom") {
                size(token, text, at, repairs);
            }
            for inner in map.values_mut() {
                fill(inner, text, at, repairs);
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| fill(item, text, at, repairs)),
        _ => {}
    }
}

/// Gives a creature token that has one of power and toughness the other, from the sizes the text prints.
fn size(token: &mut Map<String, Value>, text: &str, at: &str, repairs: &mut Vec<Repair>) {
    let creature = token
        .get("types")
        .and_then(Value::as_array)
        .is_some_and(|types| types.iter().any(|kind| kind.as_str() == Some("Creature")));
    let get = |token: &Map<String, Value>, key: &str| token.get(key).and_then(Value::as_i64);
    let (power, toughness) = (get(token, "power"), get(token, "toughness"));
    let (key, found) = match (power, toughness) {
        (None, Some(toughness)) => ("power", only(text, toughness, false)),
        (Some(power), None) => ("toughness", only(text, power, true)),
        _ => return,
    };
    let Some(found) = found.filter(|_| creature) else {
        return;
    };
    token.insert(key.to_owned(), found.into());
    let name = token.get("name").and_then(Value::as_str).unwrap_or("");
    repairs.push(Repair {
        class: RepairClass::TokenSize,
        at: at.to_owned(),
        note: format!(
            "the token {name:?} has no {key}: {found}, the one the text prints for its size"
        ),
    });
}

/// Repairs the creature tokens of one ability.
pub(super) fn repair(ability: &mut Value, text: &str, at: &str, repairs: &mut Vec<Repair>) {
    fill(ability, text, at, repairs);
}

#[cfg(test)]
mod tests {
    //! Goal: only a printed size (`N/N`) names the power of a token; modifiers, counters and negative numbers do not.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::plain("Create a 1/1 white Human creature token.", &[(1, 1)])]
    #[case::two_digits("create a 10/10 Eldrazi", &[(10, 10)])]
    #[case::zero("Create a 0/0 colorless Construct artifact creature token with 'This token gets +1/+1'.", &[(0, 0)])]
    #[case::a_bonus("Target creature gets +2/+2 until end of turn.", &[])]
    #[case::a_malus("Target creature gets -1/-1 and +1/-1.", &[])]
    #[case::the_unicode_minus("Target creature gets \u{2212}2/\u{2212}2.", &[])]
    #[case::two_sizes("a 2/2 Knight and a 3/2 Cleric", &[(2, 2), (3, 2)])]
    #[case::a_fraction_of_words("half/ten 1/", &[])]
    fn the_sizes_of_a_text(#[case] text: &str, #[case] expected: &[(i64, i64)]) {
        assert_eq!(sizes(text), expected);
    }

    #[rstest]
    #[case::one_power("a 2/2 Knight", 2, false, Some(2))]
    #[case::two_powers("a 2/2 Knight and a 3/2 Cleric", 2, false, None)]
    #[case::the_same_size_twice("a 1/1 Rat and a 1/1 Bat", 1, false, Some(1))]
    #[case::no_size_for_it("a 1/1 Rat", 5, false, None)]
    #[case::the_toughness_of_a_power("a 4/4 Beast", 4, true, Some(4))]
    fn the_missing_half_is_the_only_one_the_text_prints(
        #[case] text: &str,
        #[case] known: i64,
        #[case] power_known: bool,
        #[case] expected: Option<i64>,
    ) {
        assert_eq!(only(text, known, power_known), expected);
    }
}
