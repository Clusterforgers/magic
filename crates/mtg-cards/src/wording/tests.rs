//! Goal: each equivalence matches the wordings it names, with the fewest rules, and never a near miss that changes a
//! number, a target, a controller, a duration, a "may" or a condition; a card is a candidate only when every clause
//! that differs matches.
use rstest::rstest;

use super::*;
use crate::card::Span;
use crate::compile::ClauseKind;

#[rstest]
#[case::self_name("When this creature enters, draw a card.", "When ~ enters, draw a card.", &[Rule::SelfName])]
#[case::self_name_possessive(
    "Put a +1/+1 counter on each creature that shares a type with this creature's.",
    "Put a +1/+1 counter on each creature that shares a type with ~'s.",
    &[Rule::SelfName]
)]
#[case::self_name_spell("This spell costs {1} less to cast.", "~ costs {1} less to cast.", &[Rule::SelfName])]
#[case::contraction(
    "As long as it's a Human, it has first strike.",
    "As long as it is a Human, it has first strike.",
    &[Rule::Contraction]
)]
#[case::contraction_past(
    "If you've gained life this turn, draw a card.",
    "If you gained life this turn, draw a card.",
    &[Rule::Contraction]
)]
#[case::pronoun(
    "Shuffle ~ into its owner's library.",
    "Shuffle ~ into her owner's library.",
    &[Rule::Pronoun]
)]
#[case::back_reference(
    "Whenever one or more creatures you control attack, they gain indestructible until end of turn.",
    "Whenever one or more creatures you control attack, those creatures gain indestructible until end of turn.",
    &[Rule::BackReference]
)]
#[case::back_reference_singular(
    "Tap target creature. It doesn't untap during its controller's next untap step.",
    "Tap target creature. That creature doesn't untap during its controller's next untap step.",
    &[Rule::BackReference]
)]
#[case::elision(
    "When ~ blocks, it becomes a Bird Giant, and it loses defender.",
    "When ~ blocks, it becomes a Bird Giant and loses defender.",
    &[Rule::Elision, Rule::Punctuation]
)]
#[case::and_or(
    "Draw a card for each artifact and/or enchantment you control.",
    "Draw a card for each artifact or enchantment you control.",
    &[Rule::AndOr]
)]
#[case::duration_order(
    "Target creature gets +3/+3 until end of turn.",
    "Until end of turn, target creature gets +3/+3.",
    &[Rule::DurationOrder]
)]
#[case::dies(
    "When ~ dies, you gain 2 life.",
    "When ~ is put into a graveyard from the battlefield, you gain 2 life.",
    &[Rule::Dies]
)]
#[case::discard_hand(
    "Discard your hand.",
    "Discard X cards, where X is the number of cards in your hand.",
    &[Rule::DiscardHand]
)]
#[case::counted_as_x(
    "~ enters with a number of +1/+1 counters on it equal to the number of lands you control.",
    "~ enters with X +1/+1 counters on it, where X is the number of lands you control.",
    &[Rule::CountedAsX]
)]
#[case::counted_cards(
    "Draw cards equal to the number of creatures you control.",
    "Draw X cards, where X is the number of creatures you control.",
    &[Rule::CountedAsX]
)]
#[case::mana_count("{T}: Add three {R}.", "{T}: Add {R}{R}{R}.", &[Rule::ManaCount])]
#[case::any_order(
    "Put the rest on the bottom of your library in any order.",
    "Put the rest on the bottom of your library.",
    &[Rule::AnyOrder]
)]
#[case::punctuation(
    "Draw a card. Then you gain 1 life.",
    "Draw a card, then then you also gain 1 life",
    &[Rule::Punctuation]
)]
#[case::article("Create a 1/1 white Soldier creature token.", "Create an 1/1 white Soldier creature token.", &[Rule::Punctuation])]
fn equivalent_wordings_match(#[case] printed: &str, #[case] written: &str, #[case] rules: &[Rule]) {
    assert_eq!(same_words(printed, written).as_deref(), Some(rules));
}

#[rstest]
#[case::number(
    "Target creature gets +2/+2 until end of turn.",
    "Until end of turn, target creature gets +3/+3."
)]
#[case::counted_number(
    "~ enters with a number of +1/+1 counters on it equal to the number of lands you control.",
    "~ enters with X +1/+1 counters on it, where X is twice the number of lands you control."
)]
#[case::mana_number("{T}: Add three {R}.", "{T}: Add {R}{R}.")]
#[case::target("Destroy target creature.", "Destroy each creature.")]
#[case::target_count("Destroy up to two target creatures.", "Destroy target creature.")]
#[case::controller(
    "Creatures you control get +1/+1.",
    "Creatures your opponents control get +1/+1."
)]
#[case::controller_hand(
    "Discard your hand.",
    "Discard X cards, where X is the number of cards in target opponent's hand."
)]
#[case::duration(
    "Until your next turn, target creature gets +2/+2.",
    "Target creature gets +2/+2 until end of turn."
)]
#[case::duration_dropped(
    "Target creature gains flying until end of turn.",
    "Target creature gains flying."
)]
#[case::may("You may draw a card.", "Draw a card.")]
#[case::condition("If you control a Forest, draw a card.", "Draw a card.")]
#[case::condition_moved(
    "Each opponent discards a card. If you control a Vampire, you gain 2 life.",
    "Each opponent discards a card and you gain 2 life if you control a Vampire."
)]
#[case::self_vs_other(
    "When this creature dies, draw a card.",
    "When another creature dies, draw a card."
)]
#[case::self_vs_it("Tap ~.", "Tap it.")]
#[case::singular_plural("Untap those creatures.", "Untap that creature.")]
#[case::and_vs_or(
    "Draw a card for each artifact and creature you control.",
    "Draw a card for each artifact or creature you control."
)]
#[case::dies_vs_leaves(
    "When ~ dies, draw a card.",
    "When ~ leaves the battlefield, draw a card."
)]
#[case::any_order_elsewhere("Put them into your hand in any order.", "Put them into your hand.")]
#[case::missing_clause(
    "Enchanted creature gets +1/+1, has reach, and can block creatures with shadow.",
    "Enchanted creature gets +1/+1 and has reach."
)]
#[case::doubled_meaningful("Draw two cards.", "Draw two two cards.")]
fn near_misses_do_not_match(#[case] printed: &str, #[case] written: &str) {
    assert_eq!(same_words(printed, written), None);
}

fn clause(text: &str, written: &str) -> ClauseReport {
    ClauseReport {
        face: 0,
        span: Span { start: 0, end: 1 },
        text: text.to_owned(),
        kind: ClauseKind::Spell,
        state: ClauseState::Differs {
            ability: 0,
            written: written.to_owned(),
        },
    }
}

#[test]
fn a_card_is_a_candidate_only_when_every_differing_clause_matches() {
    let hand = clause(
        "Discard your hand.",
        "Discard X cards, where X is the number of cards in your hand.",
    );
    let comma = clause(
        "Draw a card, then discard a card.",
        "Draw a card then discard a card",
    );
    let found = bulk_candidate(&[hand.clone(), comma]).unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].rules, [Rule::DiscardHand]);
    assert_eq!(found[1].rules, [Rule::Punctuation]);

    let may = clause("You may draw a card.", "Draw a card.");
    assert_eq!(bulk_candidate(&[hand.clone(), may]), None);
    assert_eq!(bulk_candidate(&[]), None);
    let mut open = hand;
    open.state = ClauseState::Open;
    assert_eq!(bulk_candidate(&[open]), None);
}

#[test]
fn every_rule_has_a_distinct_name_and_words() {
    let names: std::collections::BTreeSet<&str> =
        Rule::ALL.iter().map(|rule| rule.name()).collect();
    assert_eq!(names.len(), Rule::ALL.len());
    for rule in Rule::ALL {
        assert!(
            !rule.title().is_empty() && rule.summary().ends_with('.'),
            "{rule:?}"
        );
    }
}
