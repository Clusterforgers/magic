//! A table of words that the renderers write one way and a card of the fifth sample of M to R writes another:
//! "Creatures can't block this turn", "less than ~'s", "If you do" after a sacrifice of the source, "return that card
//! to the battlefield", "they create", "That player draws", "You add", "investigate that many times", "creatures
//! attacking your opponents and/or planeswalkers they control" and "deals damage to it" for the creature of an event.
//! Each swap applies to the abilities whose rules hold its needle, alone and together with the other swaps that apply.

/// A swap: the text the rules must hold, the renderers' words and the card's words.
type C5MrSwap = (&'static str, &'static str, &'static str);

const C5_MR_SWAPS: [C5MrSwap; 11] = [
    (
        r#""mods":["CantBlock"]"#,
        "All creatures can't block this turn",
        "Creatures can't block this turn",
    ),
    (
        r#""op":"Lt","to":{"Stat":{"of":"This","stat":"Power"}}"#,
        "power less than ~'s power ",
        "power less than ~'s ",
    ),
    (
        r#"{"Sacrifice":{"filter":["This"]}},{"If":{"condition":"Performed","#,
        ". When you do, ",
        ". If you do, ",
    ),
    (
        r#"{"Sacrifice":{"filter":["This"]}},{"If":{"condition":"Performed","#,
        "Whenever ",
        "When ",
    ),
    (
        r#"{"May":{"effects":[{"Move":{"what":"Triggering","to":{"Battlefield":{"under":"You"}}}}]}}"#,
        "may put that card onto the battlefield under your control",
        "may return that card to the battlefield under your control",
    ),
    (
        r#"{"CreateToken":{"who":{"ControllerOf":"Triggering"},"#,
        "that player creates ",
        "they create ",
    ),
    (
        r#"{"Draw":{"who":{"ControllerOf":{"Target":0}}}}"#,
        "That creature's controller draws ",
        "That player draws ",
    ),
    (r#"{"AddMana":{"mana":{"Fixed":"#, ". Add {", ". You add {"),
    (
        r#""token":{"Named":"clue"},"count":"Triggering""#,
        "you create that many Clue tokens",
        "investigate that many times",
    ),
    (
        r#"{"Each":[{"Type":"Creature"},"Attacking"]}"#,
        "attacking creatures get ",
        "creatures attacking your opponents and/or planeswalkers they control get ",
    ),
    (
        r#"{"DealDamage":{"to":"Triggering","#,
        " damage to that player.",
        " damage to it.",
    ),
];

/// Adds the wordings with each swap that applies to the rules, one at a time and all together.
pub(super) fn add(json: &str, texts: &mut Vec<String>) {
    let applying: Vec<&C5MrSwap> = C5_MR_SWAPS
        .iter()
        .filter(|swap| json.contains(swap.0))
        .collect();
    if applying.is_empty() {
        return;
    }
    let mut found = Vec::new();
    for text in texts.iter() {
        let mut all = text.clone();
        for (_, from, to) in &applying {
            if text.contains(from) {
                found.push(text.replacen(from, to, 1));
            }
            all = all.replacen(from, to, 1);
        }
        if all != *text {
            found.push(all);
        }
    }
    texts.extend(found);
}
