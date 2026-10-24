//! A draw that follows counters put on the source without a subject: "put a +1/+1 counter on this creature and draw a
//! card." The renderers write a sentence of its own: "put a +1/+1 counter on ~. Then draw a card."

use crate::ability::Ability;
use crate::effect::Effect;

/// Adds the wordings of counters and a draw joined by "and".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let counters_then_draw = matches!(
        ability.effects(),
        [Effect::PutCounters { .. }, Effect::Draw { .. }]
    );
    if !counters_then_draw {
        return;
    }
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(". Then draw "))
        .map(|text| text.replacen(". Then draw ", " and draw ", 1))
        .collect();
    texts.extend(said);
}
