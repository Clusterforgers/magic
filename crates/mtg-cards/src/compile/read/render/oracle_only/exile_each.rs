//! The top card of each player's library exiled and any number of the spells cast for free: "Whenever ~ attacks, exile
//! the top card of each player's library, then you may cast any number of spells from among those cards without paying
//! their mana costs."

use super::super::pattern_text;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;

/// Adds the wording of a trigger that exiles one card of every library and casts what it likes of them for free.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when,
        limit: None,
        targets,
        effects,
        ..
    } = ability
    else {
        return;
    };
    let [
        Effect::ExileTop {
            who: Ref::EachPlayer,
            count,
            bind,
        },
        Effect::CastFromZone {
            who: Ref::You,
            what: Ref::Bound(cast),
            free: true,
            play: false,
            way: None,
            otherwise,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    if !targets.is_empty() || !count.is_one() || bind != cast || !otherwise.is_empty() {
        return;
    }
    if let Some(event) = pattern_text(when) {
        texts.push(format!(
            "{event}, exile the top card of each player's library, then you may cast any number of spells from among those cards without paying their mana costs."
        ));
    }
}
