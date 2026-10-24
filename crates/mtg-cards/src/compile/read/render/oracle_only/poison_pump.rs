//! A creature that grows with the poison of the opponents: "This creature gets +1/+1 for each poison counter your
//! opponents have."

use crate::ability::Ability;
use crate::modify::Modification;
use crate::refs::Ref;
use crate::value::{Amount, CounterKind};

/// Adds the wording of a static bonus to the card that counts the poison counters of all opponents.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Static {
        affected: Ref::This,
        mods,
        condition: None,
        ..
    } = ability
    else {
        return;
    };
    let poison = Amount::Counters {
        on: Ref::Opponents,
        counter: CounterKind::Poison,
    };
    let [Modification::ModifyPT { power, toughness }] = mods.as_slice() else {
        return;
    };
    if *power == poison && *toughness == poison {
        texts.push("~ gets +1/+1 for each poison counter your opponents have.".to_owned());
    }
}
