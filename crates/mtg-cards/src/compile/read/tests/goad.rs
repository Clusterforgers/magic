//! Goal: goading reads into the DSL in the shapes of the cards (an effect on a target or a group, a goaded creature of an Aura or an Equipment), writes back as the card's own text, and the near-misses stay with the table.
use super::*;

#[rstest]
#[case::a_target(
    "Goad target creature.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Goad":{"what":{"Target":0}}}]}}"#
)]
#[case::a_target_of_an_opponent(
    "Goad target creature an opponent controls.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Goad":{"what":{"Target":0}}}]}}"#
)]
fn the_wordings_read_into_the_dsl_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::a_spell_for_a_group("Goad all creatures you don't control.", ClauseKind::Spell)]
#[case::a_spell_for_the_group_of_opponents(
    "Goad all creatures your opponents control.",
    ClauseKind::Spell
)]
#[case::in_an_activation("{3}, {T}: Goad target creature.", ClauseKind::Activated)]
#[case::in_a_sacrifice_activation(
    "{1}{R}, Sacrifice an artifact or creature: Goad target creature.",
    ClauseKind::Activated
)]
#[case::in_an_attack_trigger(
    "Whenever ~ attacks, goad target creature an opponent controls.",
    ClauseKind::Triggered
)]
#[case::optional("When ~ enters, you may goad target creature.", ClauseKind::Triggered)]
#[case::it_after_a_tap("Tap target creature. Goad it.", ClauseKind::Spell)]
#[case::the_aura_pump("Enchanted creature gets +2/+2 and is goaded.", ClauseKind::Static)]
#[case::the_aura_pump_of_one("Enchanted creature gets +1/+1 and is goaded.", ClauseKind::Static)]
#[case::the_aura_keyword(
    "Enchanted creature has indestructible and is goaded.",
    ClauseKind::Static
)]
#[case::the_aura_pump_and_keyword(
    "Enchanted creature gets +1/+1, has deathtouch, and is goaded.",
    ClauseKind::Static
)]
#[case::the_equipment("Equipped creature gets +2/+0 and is goaded.", ClauseKind::Static)]
#[case::scry_when_it_attacks(
    "Whenever enchanted creature attacks, you scry 2.",
    ClauseKind::Triggered
)]
#[case::treasure_when_it_attacks(
    "Whenever enchanted creature attacks, you create a Treasure token.",
    ClauseKind::Triggered
)]
fn goad_reads_and_writes_back(#[case] text: &str, #[case] kind: ClauseKind) {
    golden(text, kind, false);
}

#[rstest]
#[case::the_aura_pump(
    "Enchanted creature gets +2/+2 and is goaded.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":2,"toughness":2}},"Goaded"]}}"#
)]
#[case::the_aura_pump_and_keyword(
    "Enchanted creature gets +1/+1, has deathtouch, and is goaded.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"Deathtouch"},"Goaded"]}}"#
)]
#[case::the_aura_keyword(
    "Enchanted creature has indestructible and is goaded.",
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":"Indestructible"},"Goaded"]}}"#
)]
fn a_goaded_creature_of_a_static_reads_the_goaded_modification(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::each_creature_of_a_player("Goad each creature that player controls.", ClauseKind::Spell)]
#[case::it_without_a_creature("Draw a card. Goad it.", ClauseKind::Spell)]
#[case::a_sentence_after("Goad target creature. That player draws a card.", ClauseKind::Spell)]
#[case::the_player_instead("Goad target player.", ClauseKind::Spell)]
#[case::a_spell("Goad target spell.", ClauseKind::Spell)]
#[case::a_card_in_a_graveyard("Goad target creature card in a graveyard.", ClauseKind::Spell)]
#[case::must_be_blocked(
    "Enchanted creature gets +3/+3, must be blocked if able, and is goaded.",
    ClauseKind::Static
)]
#[case::only_goaded("Enchanted creature is goaded.", ClauseKind::Static)]
#[case::goaded_before_the_pump("Enchanted creature is goaded and gets +2/+2.", ClauseKind::Static)]
#[case::no_and("Enchanted creature gets +2/+2, is goaded.", ClauseKind::Static)]
#[case::the_card_itself("This creature gets +2/+2 and is goaded.", ClauseKind::Static)]
#[case::goaded_creatures_cant_block(
    "Goaded creatures your opponents control can't block.",
    ClauseKind::Static
)]
fn near_misses_stay_with_the_table(#[case] text: &str, #[case] kind: ClauseKind) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}
