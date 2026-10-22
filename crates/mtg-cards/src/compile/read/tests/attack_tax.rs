//! Goal: "creatures can't attack you unless their controller pays {N} for each ..." reads as `AttackTax` for the player, for the player and the planeswalkers they control or for the planeswalkers alone, in each wording of "for each", with an amount counted by "{X} ... where X is the number of ..." and with a group of creatures, as a static ability and after "until your next turn,"; each writes back as the same text, and a cost that is not mana, a free attack, another subject or a duration of another kind stays with the table.
use super::*;

fn stat(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Static, false)
}

#[rstest]
#[case::propaganda(
    "Creatures can't attack you unless their controller pays {2} for each creature they control that's attacking you.",
    r#"{"Rule":{"rule":{"AttackTax":{"attacked":"Player","mana":"{2}"}}}}"#
)]
#[case::the_player_of_those_creatures(
    "Creatures can't attack you unless their controller pays {1} for each of those creatures.",
    r#"{"Rule":{"rule":{"AttackTax":{"attacked":"Player","mana":"{1}"}}}}"#
)]
#[case::ghostly_prison_on_a_player_and_walkers(
    "Creatures can't attack you or planeswalkers you control unless their controller pays {1} for each of those creatures.",
    r#"{"Rule":{"rule":{"AttackTax":{"attacked":"Both","mana":"{1}"}}}}"#
)]
#[case::walkers_alone(
    "Creatures can't attack planeswalkers you control unless their controller pays {1} for each creature they control that's attacking a planeswalker you control.",
    r#"{"Rule":{"rule":{"AttackTax":{"attacked":"Planeswalkers","mana":"{1}"}}}}"#
)]
#[case::walkers_alone_of_those(
    "Creatures can't attack planeswalkers you control unless their controller pays {1} for each of those creatures.",
    r#"{"Rule":{"rule":{"AttackTax":{"attacked":"Planeswalkers","mana":"{1}"}}}}"#
)]
#[case::phyrexian(
    "Creatures can't attack you or planeswalkers you control unless their controller pays {W/P} for each of those creatures.",
    r#"{"Rule":{"rule":{"AttackTax":{"attacked":"Both","mana":"{W/P}"}}}}"#
)]
#[case::two_symbols(
    "Creatures can't attack you unless their controller pays {1}{W} for each creature they control that's attacking you.",
    r#"{"Rule":{"rule":{"AttackTax":{"attacked":"Player","mana":"{1}{W}"}}}}"#
)]
#[case::an_amount_counted(
    "Creatures can't attack you or planeswalkers you control unless their controller pays {X} for each of those creatures, where X is the number of enchantments you control.",
    r#"{"Rule":{"rule":{"AttackTax":{"attacked":"Both","mana":"{1}","per":{"Count":[{"Type":"Enchantment"},{"ControlledBy":"You"}]}}}}}"#
)]
#[case::a_group_of_creatures(
    "Nonblack creatures can't attack you unless their controller pays {2} for each creature they control that's attacking you.",
    r#"{"Rule":{"rule":{"AttackTax":{"attacked":"Player","attackers":[{"Type":"Creature"},{"Not":{"Color":"Black"}}],"mana":"{2}"}}}}"#
)]
fn an_attack_tax_reads_as_a_static_ability(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::until_your_next_turn(
    "Until your next turn, creatures can't attack you or planeswalkers you control unless their controller pays {2} for each of those creatures.",
    r#"{"Spell":{"effects":[{"Rule":{"rule":{"AttackTax":{"attacked":"Both","mana":"{2}"}},"duration":"UntilYourNextTurn"}}]}}"#
)]
fn an_attack_tax_reads_as_an_effect_for_a_while(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::life("Creatures can't attack you unless their controller pays 2 life for each of those creatures.")]
#[case::no_cost("Creatures can't attack you unless their controller pays for each of those creatures.")]
#[case::an_x_without_a_count("Creatures can't attack you unless their controller pays {X} for each of those creatures.")]
#[case::another_subject("Creatures can't attack target player unless their controller pays {2} for each of those creatures.")]
#[case::a_flat_cost("Creatures can't attack you unless their controller pays {2}.")]
#[case::both_without_the_long_wording(
    "Creatures can't attack you or planeswalkers you control unless their controller pays {1} for each creature they control that's attacking you."
)]
#[case::the_opponents("Creatures can't attack your opponents unless their controller pays {2} for each of those creatures.")]
#[case::a_tap("Creatures can't attack you unless their controller taps an untapped creature they control.")]
fn a_tax_the_table_keeps_stays_unread(#[case] text: &str) {
    assert_eq!(stat(text), None, "{text}");
}
