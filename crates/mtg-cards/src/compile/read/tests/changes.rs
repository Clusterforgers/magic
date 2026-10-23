//! Goal: what a target or a group gets, gains, loses or has for a while (a pump, lost abilities and creature types, a base size, a new look, a group of a targeted player, several targets, "until your next turn", the duration first, "non-Elf", "intimidate") reads into one `Continuous` effect, writes back as the text it came from, and the near-misses stay with the table.
use super::*;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::{Pred, Ref};
use crate::value::Amount;
use mtg_core::card::Subtype;
use mtg_core::color::Color;

#[rstest]
#[case::non_subtype("Non-Elf creatures get -2/-2 until end of turn.", ClauseKind::Spell)]
#[case::each_non_subtype_counted(
    "Each non-Vampire creature gets -X/-X until end of turn, where X is the number of Vampires you control.",
    ClauseKind::Spell
)]
#[case::duration_first_and_keyword_first(
    "Until end of turn, creatures you control gain trample and get +X/+X, where X is the greatest power among creatures you control.",
    ClauseKind::Spell
)]
#[case::the_group_of_a_player(
    "Creatures target player controls get +0/+1 and gain all creature types until end of turn.",
    ClauseKind::Spell
)]
#[case::each_of_a_players_group(
    "Each creature target player controls gets +1/+1 until end of turn.",
    ClauseKind::Spell
)]
#[case::lost_keywords(
    "{1}: Permanents your opponents control lose hexproof and indestructible until end of turn.",
    ClauseKind::Activated
)]
#[case::until_your_next_turn(
    "Humans you control gain indestructible until your next turn.",
    ClauseKind::Spell
)]
#[case::a_base_size_of_x(
    "Creatures you control have base power and toughness X/X until end of turn.",
    ClauseKind::Spell
)]
#[case::a_players_group_for_a_turn(
    "Until your next turn, creatures target player controls have base power and toughness 1/1.",
    ClauseKind::Spell
)]
#[case::opponents_group(
    "Creatures your opponents control have base power and toughness 0/1 until end of turn.",
    ClauseKind::Spell
)]
#[case::lost_abilities_and_a_size(
    "Until end of turn, creatures target player controls lose all abilities and have base power and toughness 0/2.",
    ClauseKind::Spell
)]
#[case::lost_creature_types(
    "Target creature gets +3/-3 and loses all creature types until end of turn.",
    ClauseKind::Spell
)]
#[case::intimidate(
    "{1}{B}{G}: Target creature gets +1/+1 and gains intimidate until end of turn.",
    ClauseKind::Activated
)]
#[case::up_to_four_targets(
    "Up to four target permanents you control gain indestructible until end of turn.",
    ClauseKind::Spell
)]
#[case::up_to_two_targets(
    "Up to two target creatures get +1/+1 until end of turn.",
    ClauseKind::Spell
)]
#[case::protection_from_each_color(
    "Target creature you control gains protection from each color until end of turn.",
    ClauseKind::Spell
)]
#[case::becomes_a_look_for_each(
    "Until end of turn, each creature target player controls loses all abilities and becomes a blue Frog with base power and toughness 1/1.",
    ClauseKind::Spell
)]
#[case::becomes_a_look(
    "Until end of turn, target creature loses all abilities and becomes a blue Frog with base power and toughness 1/1.",
    ClauseKind::Spell
)]
#[case::a_look_without_a_size(
    "Target creature becomes a blue Frog until end of turn.",
    ClauseKind::Spell
)]
#[case::artifact_too(
    "{T}: Target nonland permanent becomes an artifact in addition to its other types until end of turn.",
    ClauseKind::Activated
)]
#[case::a_target_land(
    "{T}: Target land you control becomes a 3/3 Elemental creature with haste until end of turn. It's still a land.",
    ClauseKind::Activated
)]
#[case::a_size_of_x_in_addition(
    "{T}: Until end of turn, target land you control becomes an X/X Citizen creature with haste in addition to its other types, where X is twice the number of Gates you control.",
    ClauseKind::Activated
)]
#[case::the_duration_first("Until end of turn, target creature gets +1/+1.", ClauseKind::Spell)]
fn a_change_for_a_while_reads_and_writes_back(#[case] text: &str, #[case] kind: ClauseKind) {
    golden(text, kind, true);
}

fn effect_of(text: &str) -> Effect {
    let read = clause(text, ClauseKind::Spell, true).unwrap_or_else(|| panic!("not read: {text}"));
    read.effects()
        .first()
        .cloned()
        .unwrap_or_else(|| panic!("no effect: {text}"))
}

#[test]
fn a_look_keeps_the_creature_types_before_the_colors_and_the_size_last() {
    let Effect::Continuous { mods, duration, .. } = effect_of(
        "Until end of turn, target creature loses all abilities and becomes a blue Frog with base power and toughness 1/1.",
    ) else {
        panic!("not a continuous effect");
    };
    assert_eq!(duration, Duration::UntilEndOfTurn);
    assert_eq!(
        mods,
        vec![
            Modification::LoseAbilities,
            Modification::SetCreatureTypes(vec![Subtype("Frog".into())]),
            Modification::SetColors(vec![Color::Blue]),
            Modification::SetPT {
                power: Amount::Fixed(1),
                toughness: Amount::Fixed(1)
            },
        ]
    );
}

#[test]
fn the_group_of_a_player_names_the_player_the_spell_targets() {
    let read = clause(
        "Until your next turn, creatures target player controls have base power and toughness 1/1.",
        ClauseKind::Spell,
        true,
    )
    .unwrap();
    assert_eq!(read.targets().len(), 1);
    let Some(Effect::Continuous {
        affected: Ref::Each(group),
        duration,
        ..
    }) = read.effects().first()
    else {
        panic!("not a group");
    };
    assert_eq!(*duration, Duration::UntilYourNextTurn);
    assert!(group.0.contains(&Pred::ControlledBy(Ref::Target(0))));
}

#[test]
fn an_other_creature_type_is_not_lost_by_a_non_subtype_filter() {
    let Effect::Continuous {
        affected: Ref::Each(group),
        ..
    } = effect_of("Non-Elf creatures get -2/-2 until end of turn.")
    else {
        panic!("not a group");
    };
    assert!(
        group
            .0
            .contains(&Pred::Not(Box::new(Pred::Subtype(Subtype("Elf".into())))))
    );
}

#[rstest]
#[case::no_duration("Target creature has base power and toughness 1/1.")]
#[case::no_duration_for_a_look(
    "Target creature becomes a blue Frog with base power and toughness 1/1."
)]
#[case::the_wrong_verb_number(
    "Creatures you control get +1/+1 and loses flying until end of turn."
)]
#[case::until_end_of_combat("Creatures target player controls get +1/+1 until end of combat.")]
#[case::a_look_with_nothing(
    "Target creature becomes a with base power and toughness 1/1 until end of turn."
)]
#[case::a_size_that_counts(
    "Creatures you control have base power and toughness equal to their mana value until end of turn."
)]
#[case::two_players(
    "Creatures target player controls and creatures target opponent controls get +1/+1 until end of turn."
)]
fn near_misses_of_a_change_for_a_while_stay_with_the_table(#[case] text: &str) {
    assert!(spell(text).is_none(), "{text}");
}
