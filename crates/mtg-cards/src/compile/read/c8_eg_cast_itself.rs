//! "You may cast this card from your graveyard as long as you control a Zombie.", "... if you gained life this turn.":
//! a card's permission to cast itself from its owner's graveyard, a rule that works from there.

use super::tokens::Reader;
use crate::ability::Ability;
use crate::modify::{CastPermission, GameRule};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::Condition;

/// "You may cast this card from your graveyard[ as long as C| if C]".
pub fn c8_eg_cast_itself(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("you may cast")?;
    r.source().then_some(())?;
    r.need("from your graveyard")?;
    let condition = if r.peek_word() == Some("as") {
        Some(super::as_long_as::read(r, None)?)
    } else if r.word("if") {
        Some(super::conditional::if_condition(r)?)
    } else {
        None
    };
    Some(Ability::Rule {
        rule: GameRule::MayCast(c8_eg_permission()),
        condition,
        from: ZoneKind::Graveyard,
    })
}

/// The permission the reader gives: the card itself, cast by its owner from the graveyard.
fn c8_eg_permission() -> CastPermission {
    CastPermission {
        who: Ref::You,
        what: Selector(vec![Pred::This]),
        from: Some(ZoneKind::Graveyard),
        free: false,
        flash: false,
        only: None,
        play: false,
        once: false,
    }
}

/// The wordings of a rule `c8_eg_cast_itself` reads, none for another rule.
pub fn c8_eg_texts(rule: &GameRule, condition: Option<&Condition>) -> Vec<String> {
    if *rule != GameRule::MayCast(c8_eg_permission()) {
        return Vec::new();
    }
    let base = "You may cast ~ from your graveyard";
    let Some(condition) = condition else {
        return vec![base.to_owned()];
    };
    let long = super::as_long_as::texts(condition)
        .into_iter()
        .map(|text| format!("{base} as long as {text}"));
    let when = super::conditional::condition_texts(condition)
        .into_iter()
        .chain(super::facts::texts(condition))
        .map(|text| format!("{base} if {text}"));
    long.chain(when).collect()
}
