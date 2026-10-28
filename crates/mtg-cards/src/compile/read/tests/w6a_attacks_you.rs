//! Goal: "can't attack you", "can't attack you or planeswalkers you control" and "can't attack planeswalkers you control" read as `CantAttackYou` for a group of creatures, for the enchanted or equipped creature on its own and as the last part of a list of bonuses, each writes back as the same text, and other players, a block beside it or a duration stay with the table.
use super::*;

fn stat(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Static, false)
}

#[rstest]
#[case::blazing_archon(
    "Creatures can't attack you.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"CantAttackYou":"Player"}]}}"#
)]
#[case::sandwurm_convergence(
    "Creatures with flying can't attack you or planeswalkers you control.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"HasKeyword":"Flying"}]},"mods":[{"CantAttackYou":"Both"}]}}"#
)]
#[case::the_walkers_alone(
    "Creatures can't attack planeswalkers you control.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"CantAttackYou":"Planeswalkers"}]}}"#
)]
#[case::spectral_grasp(
    "Enchanted creature can't attack you or planeswalkers you control.",
    r#"{"Static":{"affected":"Attached","mods":[{"CantAttackYou":"Both"}]}}"#
)]
#[case::vow_of_lightning(
    "Enchanted creature gets +2/+2, has first strike, and can't attack you or planeswalkers you control.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":2,"toughness":2}},{"AddKeyword":"FirstStrike"},{"CantAttackYou":"Both"}]}}"#
)]
#[case::vow_of_wildness(
    "Enchanted creature gets +3/+3, has trample, and can't attack you or planeswalkers you control.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":3,"toughness":3}},{"AddKeyword":"Trample"},{"CantAttackYou":"Both"}]}}"#
)]
#[case::reverence(
    "Creatures with power 2 or less can't attack you.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":2}}]},"mods":[{"CantAttackYou":"Player"}]}}"#
)]
fn a_restriction_on_attacking_you_reads(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::another_player("Creatures can't attack target player.")]
#[case::the_opponents("Creatures can't attack your opponents.")]
#[case::a_block_beside_it("Creatures with flying can't attack you or block creatures you control.")]
#[case::a_permanent("Enchanted creature can't attack you or permanents you control.")]
fn another_restriction_stays_unread(#[case] text: &str) {
    assert_eq!(stat(text), None, "{text}");
}
