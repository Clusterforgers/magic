//! A card's permission to cast itself as though it had flash under a condition: "You may cast this spell as though it had
//! flash if you control a Faerie.", "As long as you control a green or blue permanent, you may cast ~ as though it had
//! flash.", "If there are two or more instant and/or sorcery cards in your graveyard, you may cast ~ as though it had
//! flash." It is a `MayCast` rule of the card that works from its owner's hand.

use super::tokens::Reader;
use crate::ability::Ability;
use crate::modify::{CastPermission, GameRule};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::Condition;

/// The permission: the card itself, cast by its owner from a hand as though it had flash.
fn ep27_permission() -> CastPermission {
    CastPermission {
        rather: None,
        who: Ref::You,
        what: Selector(vec![Pred::This]),
        from: None,
        free: false,
        flash: true,
        only: None,
        play: false,
        once: false,
        any_turn: false,
        additional: None,
        spend_as: crate::modify::SpendAs::Normal,
        when: None,
        any_cast: false,
        riders: Vec::new(),
        limit: None,
    }
}

/// "you may cast ~ as though it had flash".
fn ep27_body(r: &mut Reader<'_>) -> Option<()> {
    r.need("you may cast")?;
    r.source().then_some(())?;
    r.need("as though it had flash")
}

/// The rule under `condition`.
fn ep27_rule(condition: Condition) -> Ability {
    Ability::Rule {
        rule: GameRule::MayCast(ep27_permission()),
        condition: Some(condition),
        from: ZoneKind::Hand,
    }
}

/// The condition leads ("As long as C, ..." or "If C, ...") or follows ("... if C" or "... as long as C").
pub(super) fn ep27_flash(r: &mut Reader<'_>) -> Option<Ability> {
    if r.peek_word() == Some("as") {
        let condition = super::as_long_as::read(r, None)?;
        r.punct(',').then_some(())?;
        ep27_body(r)?;
        return Some(ep27_rule(condition));
    }
    if r.word("if") {
        let condition = super::conditional::if_condition(r)?;
        r.punct(',').then_some(())?;
        ep27_body(r)?;
        return Some(ep27_rule(condition));
    }
    ep27_body(r)?;
    let condition = if r.peek_word() == Some("as") {
        super::as_long_as::read(r, None)?
    } else {
        r.need("if")?;
        super::conditional::if_condition(r)?
    };
    Some(ep27_rule(condition))
}

/// The wordings of a rule `ep27_flash` reads; empty for another rule.
pub(super) fn ep27_flash_texts(
    rule: &GameRule,
    condition: Option<&Condition>,
    from: ZoneKind,
) -> Vec<String> {
    let (GameRule::MayCast(permission), Some(condition), ZoneKind::Hand) = (rule, condition, from)
    else {
        return Vec::new();
    };
    if *permission != ep27_permission() {
        return Vec::new();
    }
    let body = "you may cast ~ as though it had flash";
    let long = super::as_long_as::texts(condition);
    let when: Vec<String> = super::conditional::condition_texts(condition)
        .into_iter()
        .chain(super::facts::texts(condition))
        .collect();
    let lead = |text: &str| {
        let mut chars = text.chars();
        chars
            .next()
            .map(|first| first.to_uppercase().chain(chars).collect::<String>())
            .unwrap_or_default()
    };
    let mut out = Vec::new();
    for held in &long {
        out.push(format!("{} as long as {held}", lead(body)));
        out.push(format!("As long as {held}, {body}"));
    }
    for held in &when {
        out.push(format!("{} if {held}", lead(body)));
        out.push(format!("If {held}, {body}"));
    }
    out
}
