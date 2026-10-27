//! A free cast of a card that is a target or was exiled: "You may cast target instant or sorcery card from an opponent's
//! graveyard without paying its mana cost." and "Shuffle your library. Then exile the top card of your library. Until end
//! of turn, you may play that card without paying its mana cost."

use super::twin_variants;
use crate::ability::Ability;

/// The free cast of the target, and the return of the target to the hand the renderers write.
const CAST_TARGET: &str = r#"{"CastFromZone":{"what":{"Target":0},"free":true}}"#;
const RETURN_TARGET: &str = r#"{"Move":{"what":{"Target":0},"to":"Hand"}}"#;

/// The free play of the card that was exiled, which the plain twin leaves out.
const PLAY_EXILED: &str = r#",{"CastFromZone":{"what":{"Bound":"card"},"free":true,"play":true}}"#;

/// Adds the wordings of a spell that casts a target card for free or plays the card it exiled for free.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if !matches!(ability, Ability::Spell { .. }) {
        return;
    }
    let cast: Vec<String> = twin_variants(ability, CAST_TARGET, RETURN_TARGET)
        .iter()
        .filter_map(|text| {
            let phrase = text.strip_prefix("Return ")?;
            let phrase = phrase
                .strip_suffix(" to its owner's hand.")
                .or_else(|| phrase.strip_suffix(" to your hand."))?;
            Some(format!(
                "You may cast {phrase} without paying its mana cost."
            ))
        })
        .collect();
    let play: Vec<String> = twin_variants(ability, PLAY_EXILED, "")
        .iter()
        .filter(|text| text.ends_with(". Exile the top card of your library."))
        .map(|text| {
            format!(
                "{} Until end of turn, you may play that card without paying its mana cost.",
                text.replacen(". Exile the top", ". Then exile the top", 1)
            )
        })
        .collect();
    texts.extend(cast);
    texts.extend(play);
}
