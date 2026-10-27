//! Cards drawn and life lost by the colors of mana spent to cast the spell: "Converge — You draw X cards and lose X life,
//! where X is the number of colors of mana spent to cast this spell."

use super::twin_variants;
use crate::ability::Ability;

/// The draw and the loss by the colors spent, and the draw of two cards and loss of 2 life the renderers write.
const BY_COLORS: &str = r#"{"Draw":{"count":"ColorsSpent"}},{"LoseLife":{"amount":"ColorsSpent"}}"#;
const BY_TWO: &str = r#"{"Draw":{"count":2}},{"LoseLife":{"amount":2}}"#;

/// Adds the wordings of a spell that draws and loses life by the colors of mana spent to cast it.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let said: Vec<String> = twin_variants(ability, BY_COLORS, BY_TWO)
        .iter()
        .filter(|text| text.contains(" two cards") && text.contains(" 2 life"))
        .filter_map(|text| text.strip_suffix('.'))
        .map(|text| {
            format!(
                "{}, where X is the number of colors of mana spent to cast this spell.",
                text.replacen(" two cards", " X cards", 1)
                    .replacen(" 2 life", " X life", 1)
            )
        })
        .collect();
    texts.extend(said);
}
