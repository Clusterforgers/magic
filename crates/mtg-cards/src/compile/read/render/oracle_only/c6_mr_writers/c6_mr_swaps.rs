//! Words that the renderers write one way and a card of the sixth sample of M to R another: "each creature and
//! planeswalker you don't control", "there are N or more ... counters on ~" in an activation condition, and "deals
//! damage equal to ... to ..." for "deals X damage to ..., where X is ...".

/// A swap: the text the rules must hold, the renderers' words and the card's words.
type C6MrSwap = (&'static str, &'static str, &'static str);

const C6_MR_SWAPS: [C6MrSwap; 1] = [(
    r#"{"Each":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]},{"Not":{"ControlledBy":"You"}}]}"#,
    "each creature or planeswalker you don't control",
    "each creature and planeswalker you don't control",
)];

/// The condition of an activation on the counters of the source.
const C6_MR_COUNTED: &str = r#""condition":{"Compare":{"left":{"Counters":{"on":"This","#;

/// Adds the wordings with each swap that applies to the rules.
pub(super) fn add(json: &str, texts: &mut Vec<String>) {
    let mut found = Vec::new();
    for (needle, from, to) in C6_MR_SWAPS {
        if json.contains(needle) {
            found.extend(super::c6_mr_swapped(texts, from, to));
        }
    }
    texts.extend(found);
    if json.contains(C6_MR_COUNTED) {
        let there: Vec<String> = texts
            .iter()
            .filter_map(|text| c6_mr_there_are(text))
            .collect();
        texts.extend(there);
    }
    if json.contains(r#"{"DealDamage":"#) {
        let equal: Vec<String> = texts
            .iter()
            .filter_map(|text| c6_mr_equal_to(text))
            .collect();
        texts.extend(equal);
    }
}

/// "Activate only if there are N or more ... counters on ~." for "Activate only if ~ has N or more ... counters on it."
fn c6_mr_there_are(text: &str) -> Option<String> {
    let (head, rest) = text.split_once("Activate only if ~ has ")?;
    let (counters, tail) = rest.split_once(" counters on it.")?;
    Some(format!(
        "{head}Activate only if there are {counters} counters on ~.{tail}"
    ))
}

/// "deals damage equal to [amount] to [what]." for "deals X damage to [what], where X is [amount]."
fn c6_mr_equal_to(text: &str) -> Option<String> {
    let (head, rest) = text.split_once("deals X damage to ")?;
    let (what, rest) = rest.split_once(", where X is ")?;
    let (amount, tail) = match rest.split_once(". ") {
        Some((amount, tail)) => (amount, format!(" {tail}")),
        None => (rest.strip_suffix('.')?, String::new()),
    };
    Some(format!(
        "{head}deals damage equal to {amount} to {what}.{tail}"
    ))
}
