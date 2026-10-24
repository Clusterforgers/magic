//! A bonus for each object of a kind: "This creature gets +1/+1 for each creature card in your opponents' graveyards."
//! The renderers word the count after "where X is".

use crate::ability::Ability;
use crate::modify::Modification;
use crate::refs::Ref;
use crate::value::Amount;

const OPENING: &str = " gets +X/+X, where X is the number of ";

/// The group of a count in the singular, as "for each" words it: "creature cards in a graveyard" is "creature card in
/// a graveyard".
fn singular(group: &str) -> Option<String> {
    if group.contains(" cards in ") {
        return Some(group.replacen(" cards in ", " card in ", 1));
    }
    let (noun, rest) = group.split_once(' ')?;
    Some(format!("{} {rest}", noun.strip_suffix('s')?))
}

/// Adds the wordings of a static bonus to the source that counts the objects of a kind for both numbers.
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
    let [
        Modification::ModifyPT {
            power: Amount::Count(power),
            toughness: Amount::Count(toughness),
        },
    ] = mods.as_slice()
    else {
        return;
    };
    if power != toughness {
        return;
    }
    let said: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let (subject, group) = text.split_once(OPENING)?;
            let each = singular(group.strip_suffix('.')?)?;
            Some(format!("{subject} gets +1/+1 for each {each}."))
        })
        .collect();
    texts.extend(said);
}
