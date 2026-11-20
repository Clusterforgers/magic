//! The activated abilities a permanent has now, which an object that has "all activated abilities of" it gains (CR 113.6,
//! 613.1f): the printed ones, the ones effects gave it and the mana abilities of its land types (CR 305.6).

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::{AbilityDef, CardDefinition};
use mtg_core::ids::ObjectId;

use crate::abilities::{AbilityRef, AbilitySource, Grant};
use crate::effects::Borrowed;
use crate::state::GameState;

/// The activated abilities the object `donor` (whose card is `card`) has, given as its abilities `held` and its plain mana
/// abilities `mana`: the activated ones of `held` in order, then the mana abilities. A printed ability keeps the address
/// of the printed rule (`Grant::Borrowed`); any other is named by its place in this list (`Grant::R111Lent`). `with_mana`
/// false leaves the mana abilities out (CR 605.1a) without changing the places of the others.
pub(crate) fn lent(
    donor: ObjectId,
    card: &CardDefinition,
    held: &[(AbilityRef, Arc<Ability>)],
    mana: &[AbilityDef],
    with_mana: bool,
) -> Vec<Borrowed> {
    let mut found = Vec::new();
    let activated = held
        .iter()
        .filter(|(_, body)| matches!(**body, Ability::Activated { .. }));
    let mut nth = 0;
    for (address, body) in activated {
        if with_mana || !body.is_mana_ability() {
            let printed = address.object == donor && address.source == AbilitySource::Printed;
            found.push(Borrowed {
                donor,
                index: if printed { address.ability } else { nth },
                body: Arc::clone(body),
                r111_current: !printed,
            });
        }
        nth += 1;
    }
    let own_mana = crate::ability_reader::r111_printed_mana(card);
    let rules = crate::ability_reader::printed_rules(card).count();
    let mut used = vec![false; own_mana.len()];
    for def in mana {
        let printed = own_mana
            .iter()
            .enumerate()
            .position(|(at, own)| own == def && !used[at]);
        if let Some(at) = printed {
            used[at] = true;
        }
        if with_mana {
            let AbilityDef::Mana(ability) = def;
            found.push(Borrowed {
                donor,
                index: printed.map_or(nth, |at| rules + at),
                body: crate::ability_reader::tap_for_mana(ability.produces),
                r111_current: printed.is_none(),
            });
        }
        nth += 1;
    }
    found
}

/// The activated abilities `donor` has now, as the finished layers give them ([`lent`]).
pub(crate) fn lent_now(state: &GameState, donor: ObjectId, with_mana: bool) -> Vec<Borrowed> {
    let Some(object) = state.object(donor) else {
        return Vec::new();
    };
    let held = crate::ability_reader::abilities_of(state, donor);
    let mana = crate::characteristics::characteristics(state, donor)
        .map(|chars| chars.abilities)
        .unwrap_or_default();
    lent(donor, object.card(), &held, &mana, with_mana)
}

/// The ability `Grant::R111Lent { donor, nth }` names: the one the holder `holder` has under that address now, or else the
/// donor's activated ability at that place.
pub(crate) fn r111_read(state: &GameState, holder: ObjectId, grant: Grant) -> Option<Arc<Ability>> {
    let Grant::R111Lent { donor, nth } = grant else {
        return None;
    };
    let address = AbilityRef::granted(holder, grant);
    if let Some(held) = state.layers().get(holder)
        && let Some(found) = held.abilities.iter().find(|held| held.address == address)
    {
        return Some(Arc::clone(&found.body));
    }
    lent_now(state, donor, true)
        .into_iter()
        .find(|lent| lent.r111_current && lent.index == nth)
        .map(|lent| lent.body)
}
