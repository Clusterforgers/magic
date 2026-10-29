//! A spell you cast from your hand as the event of a trigger: "Whenever you cast an instant or sorcery spell with mana
//! value 5 or greater from your hand, ...". Read as the spell's own kind plus `CastFrom(Hand)`, written back the same way.

use super::tokens::Reader;
use super::triggers::player_verb;
use super::triggers::subject_text;
use crate::refs::{Pred, Ref, Selector, ZoneKind};

/// "from your hand" after the spell of a cast trigger of you: the phrase of the spell reads it as a card in the hand (or
/// leaves it), and the spell was cast from the hand.
pub(super) fn from_hand(spell: &mut Vec<Pred>, r: &mut Reader<'_>) {
    let in_hand = [Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)];
    if in_hand.iter().all(|pred| spell.contains(pred)) {
        spell.retain(|pred| !in_hand.contains(pred));
        spell.push(Pred::CastFrom(ZoneKind::Hand));
    } else if r.words("from your hand") {
        spell.push(Pred::CastFrom(ZoneKind::Hand));
    }
}

/// "Whenever you cast an instant spell", "Whenever an opponent casts a spell", "Whenever you cast a spell from your
/// hand": the spell cast and who casts it.
pub(super) fn cast_text(what: &Selector, by: &Ref) -> Option<String> {
    if let Some(spell) = super::w14b_or_spell::w14b_text(what) {
        return Some(format!("Whenever {} {spell}", player_verb(by, "cast")?));
    }
    let hand = Pred::CastFrom(ZoneKind::Hand);
    let from_hand = *by == Ref::You && what.0.contains(&hand);
    let mut preds = vec![Pred::Spell];
    preds.extend(
        what.0
            .iter()
            .filter(|pred| !from_hand || **pred != hand)
            .cloned(),
    );
    let spell = subject_text(&Selector(preds))?;
    let tail = if from_hand { " from your hand" } else { "" };
    Some(format!(
        "Whenever {} {spell}{tail}",
        player_verb(by, "cast")?
    ))
}
