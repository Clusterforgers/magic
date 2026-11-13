//! r55-05 convert wording: an ability that turns a permanent over (`Effect::Transform`) is also worded with "convert",
//! the verb of More Than Meets the Eye cards (CR 701.28), for every wording that says "transform ~" or "transform it";
//! a last "Convert it." also joins the sentence before it ("Put a +1/+1 counter on ~ and convert it.").

use crate::ability::Ability;

/// The verbs replaced, each with its convert wording.
const VERBS: [(&str, &str); 4] = [
    ("Transform ~", "Convert ~"),
    ("transform ~", "convert ~"),
    ("Transform it", "Convert it"),
    ("transform it", "convert it"),
];

/// Whether the ability turns a permanent over.
fn transforms(ability: &Ability) -> bool {
    serde_json::to_string(ability).is_ok_and(|json| json.contains("{\"Transform\":"))
}

/// Adds the convert wording of every wording of a transforming ability.
pub(super) fn r5505_convert_wordings(ability: &Ability, texts: &mut Vec<String>) {
    if !transforms(ability) {
        return;
    }
    let converted: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let mut out = text.clone();
            for (from, to) in VERBS {
                out = out.replace(from, to);
            }
            (out != *text).then_some(out)
        })
        .flat_map(|text| {
            // "Put a +1/+1 counter on ~. Convert it." is printed as one sentence: "... on ~ and convert it."
            let joined = text.replacen(". Convert it.", " and convert it.", 1);
            [Some(joined).filter(|joined| *joined != text), Some(text)]
        })
        .flatten()
        .collect();
    for text in converted {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}
