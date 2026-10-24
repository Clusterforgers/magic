//! Goal: the wordings that alone held back two precon cards each in the third round (a player who controls the creature that entered, a free cast with a fixed mana value limit, a creature that gains shroud after an attach, a return from the graveyard at the end step, a dead creature returned later, damage from one creature to another) read into the DSL, write back as the same text, and leave their near-misses to the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::blood_seeker(
    "Whenever a creature an opponent controls enters, you may have that player lose 1 life."
)]
#[case::attach(
    "When ~ enters, attach it to target creature you control. That creature gains shroud until end of turn."
)]
#[case::ghoul(
    "At the beginning of your end step, if you gained 3 or more life this turn, return this card from your graveyard to the battlefield tapped."
)]
#[case::ooze(
    "Whenever a creature dealt damage by this creature this turn dies, put a +1/+1 counter on this creature."
)]
#[case::rienne(
    "Whenever another multicolored creature you control dies, return it to its owner's hand at the beginning of the next end step."
)]
fn probe_triggers(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::expertise(
    "You may cast a spell with mana value 4 or less from your hand without paying its mana cost."
)]
#[case::bite(
    "Target creature you control deals damage equal to its power to target creature an opponent controls."
)]
fn probe_spells(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case("When ~ enters, target creature you control gains shroud until end of turn.")]
#[case("When ~ enters, target creature you control gains haste until end of turn.")]
#[case(
    "When ~ enters, attach it to target creature you control. That creature gains haste until end of turn."
)]
#[case(
    "When ~ enters, attach it to target creature you control. It gains shroud until end of turn."
)]
fn probe_more(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}
