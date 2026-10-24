//! A trigger on the card itself becoming tapped, tapped for mana, blocking or becoming blocked calls the card "it" in what it does:
//! "Whenever this creature becomes tapped, it deals 1 damage to any target.", "..., target opponent gains control of it."

use crate::ability::Ability;
use crate::refs::Pred;
use crate::trigger::TriggerPattern;

/// What the card does to or with itself, as the renderers write it with `~` and as a card says it with "it".
const PRONOUNS: [(&str, &str); 4] = [
    (", ~ deals ", ", it deals "),
    (", ~ gets ", ", it gets "),
    (", ~ gains ", ", it gains "),
    (" gains control of ~", " gains control of it"),
];

/// Adds the wordings that call the card "it" after an event of its own.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered { when, .. } = ability else {
        return;
    };
    if !about_itself(when) {
        return;
    }
    let mut found = Vec::new();
    for (named, pronoun) in PRONOUNS {
        found.extend(
            texts
                .iter()
                .filter(|text| text.contains(named))
                .map(|text| text.replacen(named, pronoun, 1)),
        );
    }
    texts.extend(found);
}

/// Whether every event of the pattern is the card itself being tapped, blocking or becoming blocked.
fn about_itself(pattern: &TriggerPattern) -> bool {
    match pattern {
        TriggerPattern::Tapped { what }
        | TriggerPattern::Blocks { what }
        | TriggerPattern::BecomesBlocked { what }
        | TriggerPattern::TappedForMana { what, .. } => what.0 == [Pred::This],
        TriggerPattern::AnyOf(events) => events.iter().all(about_itself),
        _ => false,
    }
}
