//! r40-0 readers of whole static abilities: attack requirements that follow another attacker ("If a creature you control
//! attacks, this creature also attacks if able.", "If a creature with a magnet counter on it attacks, all creatures with
//! magnet counters on them attack if able."). Each reading is kept only when its writer gives the text back.

use super::selectors::{Form, phrase};
use super::tokens::{Reader, same_text, tokenize};
use crate::ability::Ability;
use crate::modify::{CombatQuirk, GameRule};
use crate::refs::{Ref, Selector, ZoneKind};

/// Reads a whole static ability of the r40-0 wordings, kept only when it is written back as `text`.
pub(super) fn r400_static_read(text: &str) -> Option<Ability> {
    let found = r400_follow(text)?;
    crate::compile::written_variants(&found)
        .iter()
        .any(|written| same_text(written, text))
        .then_some(found)
}

/// Reads an alternative cost of the r40-0 wordings, kept only when it is written back as `text`.
pub(super) fn r400_alt_cost_read(text: &str) -> Option<Ability> {
    let found = r400_reveal_hand_read(text)?;
    crate::compile::written_variants(&found)
        .iter()
        .any(|written| same_text(written, text))
        .then_some(found)
}

/// The cards in the caster's hand.
pub(in crate::compile::read) fn r400_hand() -> Selector {
    Selector(vec![
        crate::refs::Pred::In(ZoneKind::Hand),
        crate::refs::Pred::OwnedBy(Ref::You),
    ])
}

/// The cast for revealing the hand in place of the mana cost, open while `condition` holds: every card of the hand is
/// revealed, so there is nothing to choose.
pub(in crate::compile::read) fn r400_reveal_hand(
    condition: Option<crate::value::Condition>,
) -> Ability {
    Ability::FreeCast {
        condition,
        cost: Some(crate::cost::Cost(vec![
            crate::cost::CostItem::RevealFromHand {
                filter: Selector::default(),
                count: crate::value::Amount::Count(r400_hand()),
            },
        ])),
    }
}

/// "If <condition>, you may reveal your hand rather than pay this spell's mana cost."
fn r400_reveal_hand_read(text: &str) -> Option<Ability> {
    let tokens = tokenize(text);
    let mut r = Reader::new(&tokens, false);
    r.need("if")?;
    let condition = super::as_long_as::body(&mut r, None)?;
    r.punct(',').then_some(())?;
    r.need("you may reveal your hand rather than pay")?;
    r.source().then_some(())?;
    r.punct('\'').then_some(())?;
    r.need("s mana cost")?;
    (r.sentence_end() && r.end()).then_some(())?;
    Some(r400_reveal_hand(Some(condition)))
}

/// Reads a whole triggered ability of the r40-0 wordings, kept only when it is written back as `text`.
pub(super) fn r400_triggered_read(text: &str) -> Option<Ability> {
    let found = r400_blocked_by_all_read(text)?;
    crate::compile::written_variants(&found)
        .iter()
        .any(|written| same_text(written, text))
        .then_some(found)
}

/// The trigger "Whenever a <subject> attacks, all <blockers> block that creature this turn if able.": the creatures
/// `all` picks as it resolves must block the attacker this turn.
pub(in crate::compile::read) fn r400_blocked_by_all(
    subject: &Selector,
    all: &Selector,
) -> Option<Ability> {
    serde_json::from_value(serde_json::json!({"Triggered": {
        "when": {"Attacks": {"what": subject}},
        "effects": [{"Rule": {
            "rule": {"MustBlockAttacker": {"who": {"Each": all}, "attacker": "Triggering"}},
            "duration": "UntilEndOfTurn"
        }}]
    }}))
    .ok()
}

/// Reads "all <creatures>", the creatures with counters of a kind ("all creatures with magnet counters on them").
fn r400_all(r: &mut Reader<'_>) -> Option<Selector> {
    r.need("all")?;
    let mut all = phrase(r, Form::All)?;
    if r.word("with") {
        let counter = super::counters::kind(r)?;
        r.need("counters on them")?;
        all.0.push(crate::refs::Pred::HasCounter(counter));
    }
    Some(all)
}

/// "Whenever a <creature> attacks, all <creatures> block that creature this turn if able."
fn r400_blocked_by_all_read(text: &str) -> Option<Ability> {
    let tokens = tokenize(text);
    let mut r = Reader::new(&tokens, false);
    r.need("whenever")?;
    (r.word("a") || r.word("an")).then_some(())?;
    let subject = phrase(&mut r, Form::Subject)?;
    r.need("attacks")?;
    r.punct(',').then_some(())?;
    let all = r400_all(&mut r)?;
    r.need("block that creature this turn if able")?;
    (r.sentence_end() && r.end()).then_some(())?;
    r400_blocked_by_all(&subject, &all)
}

/// "If a <creature> attacks, ~ also attacks if able." or "If a <creature> attacks, all <creatures> attack if able."
fn r400_follow(text: &str) -> Option<Ability> {
    let tokens = tokenize(text);
    let mut r = Reader::new(&tokens, false);
    r.need("if")?;
    (r.word("a") || r.word("an")).then_some(())?;
    let among = phrase(&mut r, Form::Subject)?;
    r.need("attacks")?;
    r.punct(',').then_some(())?;
    let who = if r.source() || r.words("this creature") {
        r.need("also attacks if able")?;
        Ref::This
    } else {
        let all = r400_all(&mut r)?;
        r.need("attack if able")?;
        Ref::Each(all)
    };
    (r.sentence_end() && r.end()).then_some(())?;
    Some(Ability::Rule {
        rule: GameRule::Combat {
            who,
            quirk: CombatQuirk::AttackIfAnyAttacks(among),
        },
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: the attack requirements that follow another attacker read as the rule their writer gives back, and a near
    //! miss (another verb, all creatures bound where other creatures are followed) reads nothing.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::itself("If a creature you control attacks, ~ also attacks if able.", true)]
    #[case::magnets(
        "If a creature with a magnet counter on it attacks, all creatures with magnet counters on them attack if able.",
        true
    )]
    #[case::blocks("If a creature you control blocks, ~ also blocks if able.", false)]
    #[case::other_creatures_bound(
        "If a creature with a magnet counter on it attacks, all creatures you control attack if able.",
        false
    )]
    fn r400_follow_reads(#[case] text: &str, #[case] reads: bool) {
        assert_eq!(r400_static_read(text).is_some(), reads, "{text}");
    }

    #[rstest]
    #[case::no_lands(
        "If you have no land cards in hand, you may reveal your hand rather than pay ~'s mana cost.",
        true
    )]
    #[case::no_cards(
        "If you have no cards in hand, you may reveal your hand rather than pay ~'s mana cost.",
        true
    )]
    #[case::a_card(
        "If you have no land cards in hand, you may reveal a card from your hand rather than pay ~'s mana cost.",
        false
    )]
    fn r400_reveal_hand_reads(#[case] text: &str, #[case] reads: bool) {
        assert_eq!(r400_alt_cost_read(text).is_some(), reads, "{text}");
    }

    #[rstest]
    #[case::magnets(
        "Whenever a creature with a magnet counter on it attacks, all creatures with magnet counters on them block that creature this turn if able.",
        true
    )]
    #[case::each_combat(
        "Whenever a creature with a magnet counter on it attacks, all creatures with magnet counters on them block that creature each combat if able.",
        false
    )]
    #[case::blocks(
        "Whenever a creature with a magnet counter on it blocks, all creatures with magnet counters on them block that creature this turn if able.",
        false
    )]
    fn r400_blocked_by_all_reads(#[case] text: &str, #[case] reads: bool) {
        assert_eq!(r400_triggered_read(text).is_some(), reads, "{text}");
    }
}
