//! Two abilities in one sentence of the r47-01 pack: "You may cast this card from your graveyard, but not from anywhere
//! else." is the card's permission to cast itself from its owner's graveyard and its rule that it is cast only while it
//! is there (CR 601.3), in that order.

use super::tokens::Reader;
use crate::ability::Ability;
use crate::modify::{CastPermission, GameRule};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::Condition;

/// The two abilities: the permission from the graveyard and "cast only from there".
fn r4701_graveyard_only_pair() -> [Ability; 2] {
    [
        Ability::Rule {
            rule: GameRule::MayCast(CastPermission {
                what: Selector(vec![Pred::This]),
                from: Some(ZoneKind::Graveyard),
                ..CastPermission::default()
            }),
            condition: None,
            from: ZoneKind::Graveyard,
        },
        Ability::CastOnly {
            condition: Condition::Matches {
                subject: Ref::This,
                selector: Selector(vec![Pred::In(ZoneKind::Graveyard)]),
            },
        },
    ]
}

/// "You may cast this card from your graveyard, but not from anywhere else."
pub(super) fn r4701_graveyard_only(r: &mut Reader<'_>) -> Option<Vec<Ability>> {
    r.need("you may cast")?;
    (r.words("this card") || r.source()).then_some(())?;
    r.need("from your graveyard")?;
    r.punct(',');
    r.need("but not from anywhere else")?;
    r.sentence_end().then_some(())?;
    Some(r4701_graveyard_only_pair().to_vec())
}

/// The wordings of the two abilities `r4701_graveyard_only` reads; empty for any other two.
pub(super) fn r4701_pair_texts(first: &Ability, second: &Ability) -> Vec<String> {
    let [permission, only] = r4701_graveyard_only_pair();
    if *first != permission || *second != only {
        return Vec::new();
    }
    ["this card", "~"]
        .iter()
        .map(|card| format!("You may cast {card} from your graveyard, but not from anywhere else."))
        .collect()
}
