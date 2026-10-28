//! Whole abilities of the gap constructs whose head or body the renderers do not write: the ability is matched whole
//! as data and worded as the card prints it ("Whenever one or more cards are put into exile from your library and/or
//! your graveyard, put a +1/+1 counter on ~.").

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording.
const C6_GAP_WHOLES: [(&str, &str); 1] = [(
    r#"{"Triggered":{"when":{"Batch":{"AnyOf":[{"ZoneChange":{"what":[{"OwnedBy":"You"}],"from":"Library","to":"Exile"}},{"ZoneChange":{"what":[{"OwnedBy":"You"}],"from":"Graveyard","to":"Exile"}}]}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#,
    "Whenever one or more cards are put into exile from your library and/or your graveyard, put a +1/+1 counter on ~.",
)];

/// The wordings with their abilities as the data writes them, read once.
static C6_GAP_SHAPES: LazyLock<Vec<(serde_json::Value, &str)>> = LazyLock::new(|| {
    C6_GAP_WHOLES
        .iter()
        .filter_map(|(shape, text)| Some((c6_gap_shape(shape)?, *text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(data) = serde_json::to_value(ability) else {
        return;
    };
    for (shape, text) in C6_GAP_SHAPES.iter() {
        if *shape == data {
            texts.push((*text).to_owned());
        }
    }
}

/// The JSON of an ability as the data writes it, defaults filled in the way the rules files hold them.
fn c6_gap_shape(shape: &str) -> Option<serde_json::Value> {
    let ability: Ability = serde_json::from_str(shape).ok()?;
    serde_json::to_value(ability).ok()
}
