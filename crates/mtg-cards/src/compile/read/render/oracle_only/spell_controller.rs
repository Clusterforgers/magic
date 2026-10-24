//! The player and the spell of a cast event: "Whenever an opponent casts their first noncreature spell each turn, that
//! player loses life equal to that spell's mana value." The renderers call the cast object "that permanent".

use crate::ability::Ability;
use crate::trigger::TriggerPattern;

/// What the renderers write for the spell of the event and the oracle's words for it.
const SPELL_WORDS: [(&str, &str); 2] = [
    ("that permanent's controller", "that player"),
    ("that permanent's", "that spell's"),
];

/// Adds the wordings that name the caster and the spell of the event of a trigger on a spell being cast.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let events_a_spell = |event: &TriggerPattern| match event {
        TriggerPattern::SpellCast { .. } => true,
        TriggerPattern::Nth { event, .. } => matches!(**event, TriggerPattern::SpellCast { .. }),
        _ => false,
    };
    let Ability::Triggered { when, .. } = ability else {
        return;
    };
    if !events_a_spell(when) {
        return;
    }
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.contains("that permanent's"))
        .map(|text| {
            SPELL_WORDS
                .iter()
                .fold(text.clone(), |so_far, (from, into)| {
                    so_far.replace(from, into)
                })
        })
        .collect();
    texts.extend(found);
}
