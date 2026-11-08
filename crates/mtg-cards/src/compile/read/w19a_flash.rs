//! A card that has flash under a condition: "~ has flash as long as you control a Desert.", "This spell has flash as long
//! as you control a Merfolk." It is a static ability of the card while it is in a hand.

use super::Reader;
use super::as_long_as;
use crate::ability::Ability;
use crate::keyword::Keyword;
use crate::modify::Modification;
use crate::refs::{Pred, Ref, Selector, ZoneKind};

/// The card itself, while it is in a hand.
fn w19a_this_in_hand() -> Ref {
    Ref::Each(Selector(vec![Pred::In(ZoneKind::Hand), Pred::This]))
}

/// "~ has flash as long as <condition>".
pub(super) fn w19a_flash(r: &mut Reader<'_>) -> Option<Ability> {
    r.source().then_some(())?;
    r.need("has flash")?;
    let condition = as_long_as::read(r, Some(&Ref::This))?;
    Some(Ability::Static {
        affected: w19a_this_in_hand(),
        mods: vec![Modification::AddKeyword(Keyword::Flash)],
        condition: Some(condition),
        defining: false,
        from: ZoneKind::Hand,
    })
}

/// The wordings of the conditional flash above; empty for any other ability.
pub(super) fn w19a_flash_texts(ability: &Ability) -> Vec<String> {
    let Ability::Static {
        affected,
        mods,
        condition: Some(condition),
        defining: false,
        from: ZoneKind::Hand,
    } = ability
    else {
        return Vec::new();
    };
    if *affected != w19a_this_in_hand()
        || !matches!(mods.as_slice(), [Modification::AddKeyword(Keyword::Flash)])
    {
        return Vec::new();
    }
    as_long_as::texts(condition)
        .into_iter()
        .flat_map(|held| {
            [
                format!("~ has flash as long as {held}"),
                format!("this spell has flash as long as {held}"),
            ]
        })
        .collect()
}
