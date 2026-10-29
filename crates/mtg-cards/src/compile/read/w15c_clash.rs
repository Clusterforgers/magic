//! Clash wordings: "clash with defending player" (the opponent named, not chosen), and a free cast from the hand that
//! names its mana value limit after the hand (Marvo, Deep Operative).

use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::Ref;

/// The words of a clash with the defending player.
pub(super) const W15C_DEFENDING: &str = "clash with defending player";

/// A clash of the controller with the defending player that does nothing more.
pub(super) fn w15c_defending_clash() -> Effect {
    Effect::Clash {
        who: Ref::You,
        win: Vec::new(),
        lose: Vec::new(),
        until: false,
        with: Some(Ref::Defending),
    }
}

/// The wordings that name the mana value limit of a free cast from the hand after the hand: "you may cast a spell from
/// your hand with mana value 8 or less without paying its mana cost", also as a sentence opened by "Then" after another.
pub(super) fn w15c_hand_first(texts: &[String]) -> Vec<String> {
    const LEAD: &str = "cast a spell with mana value ";
    const HAND: &str = " or less from your hand without paying";
    texts
        .iter()
        .filter_map(|text| {
            let (before, rest) = text.split_once(LEAD)?;
            let (number, after) = rest.split_once(HAND)?;
            Some(format!(
                "{before}cast a spell from your hand with mana value {number} or less without paying{after}"
            ))
        })
        .flat_map(|text| {
            let then = text
                .contains(". You may cast a spell from your hand")
                .then(|| text.replace(". You may cast", ". Then you may cast"));
            std::iter::once(text).chain(then)
        })
        .collect()
}

/// "Clash with defending player.": a clash whose opponent is the defending player.
pub(super) fn w15c_clash(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need(W15C_DEFENDING)?;
    Some(vec![w15c_defending_clash()])
}
