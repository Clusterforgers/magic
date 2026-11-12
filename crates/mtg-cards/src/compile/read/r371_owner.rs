//! Combat measured from the creature's owner: "can't attack its owner (or planeswalkers its owner controls)"
//! (`CantAttackOwner`), "can't be blocked unless it's attacking its owner or a permanent its owner controls"
//! (`CantBeBlockedUnlessAttackingOwner`) and "~ attacks each combat if able and can't attack its owner ..." (the rule
//! and the restriction of one sentence). Read and written back.

use super::effects::templates;
use super::statics::statics;
use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::{Attacked, GameRule, Modification, Prohibited};
use crate::refs::{Ref, ZoneKind};

/// The words after "can't" of `CantBeBlockedUnlessAttackingOwner`.
pub(super) const UNLESS_AT_OWNER: &str =
    "be blocked unless it's attacking its owner or a permanent its owner controls";

/// What follows "can't attack" when the owner is protected: "its owner or planeswalkers its owner controls",
/// "planeswalkers its owner controls" or "its owner".
pub(super) fn r371_whom(r: &mut Reader<'_>) -> Option<Modification> {
    let attacked = if r.words("its owner or planeswalkers its owner controls") {
        Attacked::Both
    } else if r.words("planeswalkers its owner controls") {
        Attacked::Planeswalkers
    } else if r.words("its owner") {
        Attacked::Player
    } else {
        return None;
    };
    Some(Modification::CantAttackOwner(attacked))
}

/// The restriction `r371_whom` reads, written back with "can't attack".
pub(super) const fn r371_whom_text(attacked: Attacked) -> &'static str {
    match attacked {
        Attacked::Both => "can't attack its owner or planeswalkers its owner controls",
        Attacked::Planeswalkers => "can't attack planeswalkers its owner controls",
        Attacked::Player => "can't attack its owner",
    }
}

/// "~ attacks each combat if able and can't attack its owner or planeswalkers its owner controls.": the rule that the
/// card attacks and the restriction on it.
pub(super) fn r371_attacks_and_cant(r: &mut Reader<'_>) -> Option<Vec<Ability>> {
    r.source().then_some(())?;
    r.need("attacks each combat if able and can't attack")?;
    let restriction = r371_whom(r)?;
    r.sentence_end().then_some(())?;
    Some(vec![
        Ability::Rule {
            rule: GameRule::MustAttack(Ref::This),
            condition: None,
            from: ZoneKind::Battlefield,
        },
        statics(Ref::This, vec![restriction], None),
    ])
}

/// The sentence `r371_attacks_and_cant` reads, written back; empty for any other two abilities.
pub(super) fn r371_pair_texts(first: &Ability, second: &Ability) -> Vec<String> {
    let (
        Ability::Rule {
            rule: GameRule::MustAttack(Ref::This),
            condition: None,
            from: ZoneKind::Battlefield,
        },
        Ability::Static {
            affected: Ref::This,
            mods,
            condition: None,
            defining: false,
            from: ZoneKind::Battlefield,
        },
    ) = (first, second)
    else {
        return Vec::new();
    };
    let [Modification::CantAttackOwner(attacked)] = mods.as_slice() else {
        return Vec::new();
    };
    vec![format!(
        "~ attacks each combat if able and {}.",
        r371_whom_text(*attacked)
    )]
}

/// "~ attacks each combat if able, can't be sacrificed, and can't attack its owner.": the rule that the card attacks,
/// the rule that it can't be sacrificed and the restriction on whom it attacks.
pub(super) fn r371_attacks_unsacrificed(r: &mut Reader<'_>) -> Option<Vec<Ability>> {
    r.source().then_some(())?;
    r.need("attacks each combat if able")?;
    r.punct(',').then_some(())?;
    r.need("can't be sacrificed")?;
    r.punct(',').then_some(())?;
    r.need("and can't attack")?;
    let restriction = r371_whom(r)?;
    r.sentence_end().then_some(())?;
    Some(vec![
        rule(GameRule::MustAttack(Ref::This)),
        rule(GameRule::Cant {
            who: Ref::This,
            action: Prohibited::Sacrifice,
        }),
        statics(Ref::This, vec![restriction], None),
    ])
}

fn rule(rule: GameRule) -> Ability {
    Ability::Rule {
        rule,
        condition: None,
        from: ZoneKind::Battlefield,
    }
}

/// The sentence `r371_attacks_unsacrificed` reads, written back; empty for any other three abilities.
pub fn r371_three_texts(first: &Ability, second: &Ability, third: &Ability) -> Vec<String> {
    let unsacrificed = rule(GameRule::Cant {
        who: Ref::This,
        action: Prohibited::Sacrifice,
    });
    if *second != unsacrificed {
        return Vec::new();
    }
    r371_pair_texts(first, third)
        .into_iter()
        .filter_map(|text| {
            text.strip_prefix("~ attacks each combat if able and ")
                .map(|rest| {
                    format!("~ attacks each combat if able, can't be sacrificed, and {rest}")
                })
        })
        .collect()
}

/// "untap ~ and draw a card": a sentence and the controller's draw joined by "and", read as the two sentences in order.
pub(super) fn r371_and_draw(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> Option<()> {
    let found = templates().find_map(|template| {
        r.attempt(|r| {
            let mut first = template(r)?;
            r.need("and")?;
            (r.peek_word() == Some("draw")).then_some(())?;
            let second = templates().find_map(|template| {
                r.attempt(|r| {
                    let found = template(r)?;
                    r.sentence_end().then_some(found)
                })
            })?;
            (second.len() == 1 && matches!(second.first(), Some(Effect::Draw { .. })))
                .then_some(())?;
            first.extend(second);
            Some(first)
        })
    })?;
    effects.extend(found);
    Some(())
}

/// The wordings of `texts` whose last sentence, the controller's draw, follows the one before with "and".
pub(super) fn r371_and_draw_wordings(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter_map(|text| {
            let (head, draw) = text.rsplit_once(". Draw ")?;
            (!head.contains(". ") && !draw.contains(". "))
                .then(|| format!("{head} and draw {draw}"))
        })
        .collect()
}
