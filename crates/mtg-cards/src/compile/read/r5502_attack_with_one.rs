//! "Each opponent must attack with at least one creature each combat if able." (Seeker of Slaanesh) and "Each opponent
//! must attack you or a planeswalker you control with at least one creature each combat if able." (Trove of
//! Temptation), read into `GameRule::MustAttackWithOne` and written back.

use super::tokens::Reader;
use crate::modify::GameRule;
use crate::refs::{Pred, Ref, Selector};
use mtg_core::card::CardType;

/// "you or a planeswalker you control" as the defenders a creature attacks.
fn you_or_your_planeswalker() -> Ref {
    Ref::Union(vec![
        Ref::You,
        Ref::Each(Selector(vec![
            Pred::Type(CardType::Planeswalker),
            Pred::ControlledBy(Ref::You),
        ])),
    ])
}

/// "each opponent must attack [you or a planeswalker you control] with at least one creature each combat if able".
pub fn r5502_read(r: &mut Reader<'_>) -> Option<GameRule> {
    r.need("each opponent must attack")?;
    let at = r
        .words("you or a planeswalker you control")
        .then(|| Box::new(you_or_your_planeswalker()));
    r.need("with at least one creature each combat if able")?;
    Some(GameRule::MustAttackWithOne {
        who: Ref::Opponents,
        at,
    })
}

/// What `r5502_read` reads, written back without a period; nothing for another rule.
pub fn r5502_texts(rule: &GameRule) -> Vec<String> {
    let GameRule::MustAttackWithOne {
        who: Ref::Opponents,
        at,
    } = rule
    else {
        return Vec::new();
    };
    let at = match at {
        None => "",
        Some(at) if **at == you_or_your_planeswalker() => " you or a planeswalker you control",
        Some(_) => return Vec::new(),
    };
    vec![format!(
        "Each opponent must attack{at} with at least one creature each combat if able"
    )]
}
