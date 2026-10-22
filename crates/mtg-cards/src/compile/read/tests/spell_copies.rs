//! Goal: the wordings that copy a spell or an ability ("Copy target instant or sorcery spell", "except that the copy is red", "Copy target triggered ability you control", "You may choose new targets for the copy") and make a permanent a copy until end of turn read into `CopySpell`, `CopyAbility` and `BecomeCopy`, are written back as the card's text, and every near-miss stays with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

fn activated(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Activated, false)
}

fn statics(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Static, false)
}

const SPELLS: &str = r#""targets":[{"what":{"Object":["Spell",{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}}]"#;

#[rstest]
#[case::twincast(
    "Copy target instant or sorcery spell. You may choose new targets for the copy.",
    r#"{"Spell":{TARGETS,"effects":[{"CopySpell":{"what":{"Target":0},"new_targets":true}}]}}"#
)]
#[case::without_new_targets(
    "Copy target instant or sorcery spell.",
    r#"{"Spell":{TARGETS,"effects":[{"CopySpell":{"what":{"Target":0}}}]}}"#
)]
#[case::fork(
    "Copy target instant or sorcery spell, except that the copy is red. You may choose new targets for the copy.",
    r#"{"Spell":{TARGETS,"effects":[{"CopySpell":{"what":{"Target":0},"new_targets":true,"except":[{"SetColors":["Red"]}]}}]}}"#
)]
#[case::two_colors(
    "Copy target instant or sorcery spell, except that the copy is blue and black.",
    r#"{"Spell":{TARGETS,"effects":[{"CopySpell":{"what":{"Target":0},"except":[{"SetColors":["Blue","Black"]}]}}]}}"#
)]
#[case::colorless(
    "Copy target instant or sorcery spell, except that the copy is colorless. You may choose new targets for the copy.",
    r#"{"Spell":{TARGETS,"effects":[{"CopySpell":{"what":{"Target":0},"new_targets":true,"except":[{"SetColors":[]}]}}]}}"#
)]
fn a_spell_copies_an_instant_or_sorcery_spell(#[case] text: &str, #[case] json: &str) {
    reads_as(
        text,
        ClauseKind::Spell,
        false,
        &json.replace("TARGETS", SPELLS),
    );
}

#[rstest]
#[case::any_spell(
    "Copy target spell.",
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"CopySpell":{"what":{"Target":0}}}]}}"#
)]
#[case::your_spell(
    "Copy target instant or sorcery spell you control. You may choose new targets for the copy.",
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"ControlledBy":"You"}]}}],"effects":[{"CopySpell":{"what":{"Target":0},"new_targets":true}}]}}"#
)]
#[case::a_creature_spell(
    "Copy target creature spell. You may choose new targets for the copy.",
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"Type":"Creature"}]}}],"effects":[{"CopySpell":{"what":{"Target":0},"new_targets":true}}]}}"#
)]
#[case::a_noncreature_spell(
    "Copy target noncreature spell.",
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"Not":{"Type":"Creature"}}]}}],"effects":[{"CopySpell":{"what":{"Target":0}}}]}}"#
)]
#[case::a_permanent_spell(
    "Copy target permanent spell you control.",
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"Not":{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}},{"ControlledBy":"You"}]}}],"effects":[{"CopySpell":{"what":{"Target":0}}}]}}"#
)]
#[case::any_permanent_spell(
    "Copy target permanent spell.",
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"Not":{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}}]}}],"effects":[{"CopySpell":{"what":{"Target":0}}}]}}"#
)]
fn other_spells_are_copied_with_the_same_words(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::triggered(
    "Copy target triggered ability you control. You may choose new targets for the copy.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Ability":"Triggered"},{"ControlledBy":"You"}]}}],"effects":[{"CopyAbility":{"what":{"Target":0},"new_targets":true}}]}}"#
)]
#[case::activated(
    "Copy target activated ability you control.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Ability":"Activated"},{"ControlledBy":"You"}]}}],"effects":[{"CopyAbility":{"what":{"Target":0}}}]}}"#
)]
#[case::either_kind(
    "Copy target activated or triggered ability you control. You may choose new targets for the copy.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"AnyOf":[{"Ability":"Activated"},{"Ability":"Triggered"}]},{"ControlledBy":"You"}]}}],"effects":[{"CopyAbility":{"what":{"Target":0},"new_targets":true}}]}}"#
)]
#[case::anyones(
    "Copy target triggered ability.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Ability":"Triggered"}]}}],"effects":[{"CopyAbility":{"what":{"Target":0}}}]}}"#
)]
fn an_ability_is_copied_by_its_kind(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::strionic_resonator(
    "{2}, {T}: Copy target triggered ability you control. You may choose new targets for the copy.",
    r#"{"Activated":{"cost":[{"Mana":"{2}"},"Tap"],"targets":[{"what":{"Object":[{"Ability":"Triggered"},{"ControlledBy":"You"}]}}],"effects":[{"CopyAbility":{"what":{"Target":0},"new_targets":true}}]}}"#
)]
#[case::lithoform_engine_spells(
    "{3}, {T}: Copy target instant or sorcery spell you control. You may choose new targets for the copy.",
    r#"{"Activated":{"cost":[{"Mana":"{3}"},"Tap"],"targets":[{"what":{"Object":["Spell",{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"ControlledBy":"You"}]}}],"effects":[{"CopySpell":{"what":{"Target":0},"new_targets":true}}]}}"#
)]
#[case::lithoform_engine_permanents(
    "{4}, {T}: Copy target permanent spell you control.",
    r#"{"Activated":{"cost":[{"Mana":"{4}"},"Tap"],"targets":[{"what":{"Object":["Spell",{"Not":{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}},{"ControlledBy":"You"}]}}],"effects":[{"CopySpell":{"what":{"Target":0}}}]}}"#
)]
fn an_activated_ability_copies(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Activated, false, json);
}

#[rstest]
#[case::dualcaster_mage(
    "When this creature enters, copy target instant or sorcery spell. You may choose new targets for the copy.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},TARGETS,"effects":[{"CopySpell":{"what":{"Target":0},"new_targets":true}}]}}"#
)]
#[case::an_ability_on_an_attack(
    "Whenever ~ attacks, copy target triggered ability you control.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[{"Ability":"Triggered"},{"ControlledBy":"You"}]}}],"effects":[{"CopyAbility":{"what":{"Target":0}}}]}}"#
)]
fn a_trigger_copies(#[case] text: &str, #[case] json: &str) {
    reads_as(
        text,
        ClauseKind::Triggered,
        false,
        &json.replace("TARGETS", SPELLS),
    );
}

#[rstest]
#[case::dual_casting(
    "Enchanted creature has \"{R}, {T}: Copy target instant or sorcery spell you control. You may choose new targets for the copy.\"",
    r#"{"Static":{"affected":"Attached","mods":[{"GrantAbility":{"Activated":{"cost":[{"Mana":"{R}"},"Tap"],"targets":[{"what":{"Object":["Spell",{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"ControlledBy":"You"}]}}],"effects":[{"CopySpell":{"what":{"Target":0},"new_targets":true}}]}}}]}}"#
)]
#[case::any_activated_ability(
    "Enchanted creature has \"{T}: Draw a card.\"",
    r#"{"Static":{"affected":"Attached","mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}]}}}]}}"#
)]
#[case::an_aura_on_a_land(
    "Enchanted land has \"{T}: Create a 1/1 green Squirrel creature token.\"",
    r#"{"Static":{"affected":"Attached","mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"CreateToken":{"token":{"Custom":{"name":"Squirrel","colors":["Green"],"types":["Creature"],"subtypes":["Squirrel"],"power":1,"toughness":1}}}}]}}}]}}"#
)]
#[case::an_aura_on_any_permanent(
    "Enchanted permanent has \"{T}: Draw a card.\"",
    r#"{"Static":{"affected":"Attached","mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}]}}}]}}"#
)]
#[case::an_equipment(
    "Equipped creature has \"{1}, {T}: Draw a card.\"",
    r#"{"Static":{"affected":"Attached","mods":[{"GrantAbility":{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"Draw":{}}]}}}]}}"#
)]
fn a_permanent_is_given_an_activated_ability(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::cytoshape(
    "Choose a nonlegendary creature on the battlefield. Target creature becomes a copy of that creature until end of turn.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Choose":{"from":{"Each":[{"Type":"Creature"},{"Not":{"Supertype":"Legendary"}}]},"as":"copied"}},{"BecomeCopy":{"what":{"Target":0},"of":{"Bound":"copied"},"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::it_for_that_creature(
    "Choose a creature on the battlefield. Target creature becomes a copy of it until end of turn.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Choose":{"from":{"Each":[{"Type":"Creature"}]},"as":"copied"}},{"BecomeCopy":{"what":{"Target":0},"of":{"Bound":"copied"},"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::an_artifact(
    "Choose an artifact on the battlefield. Target artifact becomes a copy of that artifact until end of turn.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Artifact"}]}}],"effects":[{"Choose":{"from":{"Each":[{"Type":"Artifact"}]},"as":"copied"}},{"BecomeCopy":{"what":{"Target":0},"of":{"Bound":"copied"},"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::a_permanent(
    "Choose a permanent on the battlefield. Target permanent you control becomes a copy of that permanent until end of turn.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"ControlledBy":"You"}]}}],"effects":[{"Choose":{"from":{"Each":[]},"as":"copied"}},{"BecomeCopy":{"what":{"Target":0},"of":{"Bound":"copied"},"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::the_card_itself(
    "~ becomes a copy of target creature until end of turn.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"BecomeCopy":{"what":"This","of":{"Target":0},"duration":"UntilEndOfTurn"}}]}}"#
)]
fn a_permanent_becomes_a_copy_until_end_of_turn(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::tendrils(
    "Target player loses 2 life and you gain 2 life.",
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"LoseLife":{"who":{"Target":0},"amount":2}},{"GainLife":{"amount":2}}]}}"#
)]
#[case::each_opponent(
    "Each opponent loses 1 life and you gain 1 life.",
    r#"{"Spell":{"effects":[{"LoseLife":{"who":"Opponents","amount":1}},{"GainLife":{"amount":1}}]}}"#
)]
#[case::a_discard_and_a_draw(
    "Target opponent discards a card and you draw a card.",
    r#"{"Spell":{"targets":[{"what":"Opponent"}],"effects":[{"Discard":{"who":{"Target":0},"count":1}},{"Draw":{"count":1}}]}}"#
)]
#[case::two_phrases_and_you(
    "Target player mills three cards and loses 3 life and you gain 3 life.",
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Mill":{"who":{"Target":0},"count":3}},{"LoseLife":{"who":{"Target":0},"amount":3}},{"GainLife":{"amount":3}}]}}"#
)]
fn another_player_acts_and_you_follow(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::the_copies(
    "Copy target instant or sorcery spell. You may choose new targets for the copies."
)]
#[case::the_wrong_sentence(
    "Copy target instant or sorcery spell. You may choose new targets for it."
)]
#[case::twice("Copy target instant or sorcery spell twice.")]
#[case::it("Copy it.")]
#[case::that_spell("Copy that spell. You may choose new targets for the copy.")]
#[case::a_creature("Copy target creature.")]
#[case::a_card_in_a_graveyard("Copy target instant card in your graveyard.")]
#[case::a_keyword_exception(
    "Copy target instant or sorcery spell, except that the copy has flying."
)]
#[case::a_type_exception(
    "Copy target instant or sorcery spell, except that the copy is a Dragon in addition to its other types."
)]
#[case::a_size_exception("Copy target instant or sorcery spell, except that the copy is 3/3.")]
#[case::an_exception_without_that("Copy target instant or sorcery spell, except the copy is red.")]
#[case::an_exception_without_a_comma(
    "Copy target instant or sorcery spell except that the copy is red."
)]
#[case::a_dangling_exception("Copy target instant or sorcery spell, except that the copy is")]
#[case::new_targets_alone("You may choose new targets for the copy.")]
#[case::any_ability("Copy target ability. You may choose new targets for the copy.")]
#[case::a_loyalty_ability("Copy target loyalty ability you control.")]
#[case::an_opponents_ability("Copy target triggered ability an opponent controls.")]
#[case::an_ability_with_an_exception(
    "Copy target triggered ability you control, except that the copy is red."
)]
#[case::a_second_target("Copy target spell. Copy target triggered ability you control.")]
#[case::a_spell_in_a_zone("Copy target instant or sorcery spell from your graveyard.")]
#[case::several_spells("Copy each instant or sorcery spell.")]
#[case::up_to_one("Copy up to one target instant or sorcery spell.")]
#[case::a_permanent_with_a_color("Copy target red permanent spell you control.")]
fn near_misses_of_copying_a_spell_or_an_ability_stay_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Spell, false), None, "{text}");
}

#[rstest]
#[case::no_zone(
    "Choose a creature. Target creature becomes a copy of that creature until end of turn."
)]
#[case::no_duration(
    "Choose a nonlegendary creature on the battlefield. Target creature becomes a copy of that creature."
)]
#[case::another_duration(
    "Choose a creature on the battlefield. Target creature becomes a copy of that creature until your next turn."
)]
#[case::no_choice("Target creature becomes a copy of that creature until end of turn.")]
#[case::another_noun(
    "Choose a creature on the battlefield. Target creature becomes a copy of that artifact until end of turn."
)]
#[case::two_types(
    "Choose a creature or artifact on the battlefield. Target creature becomes a copy of that creature until end of turn."
)]
#[case::an_exception(
    "Choose a creature on the battlefield. Target creature becomes a copy of that creature until end of turn, except it's red."
)]
#[case::a_spell_chosen(
    "Choose a creature on the battlefield. Target spell becomes a copy of that creature until end of turn."
)]
#[case::a_card_in_a_graveyard(
    "Choose a creature on the battlefield. Target creature card in your graveyard becomes a copy of that creature until end of turn."
)]
#[case::the_card_without_a_duration("~ becomes a copy of target creature.")]
#[case::the_card_copying_a_card(
    "~ becomes a copy of target creature card in a graveyard until end of turn."
)]
#[case::the_card_copying_a_spell("~ becomes a copy of target spell until end of turn.")]
#[case::the_card_copying_itself("~ becomes a copy of ~ until end of turn.")]
#[case::another_creature(
    "Target creature becomes a copy of another target creature until end of turn."
)]
fn near_misses_of_becoming_a_copy_stay_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Spell, false), None, "{text}");
}

#[rstest]
#[case::a_triggered_ability("Enchanted creature has \"When this creature dies, draw a card.\"")]
#[case::an_ability_and_a_keyword("Enchanted creature has \"{T}: Draw a card.\" and flying")]
#[case::an_unfinished_quote("Enchanted creature has \"{T}: Draw a card.")]
#[case::an_unread_ability("Enchanted creature has \"{T}: Do something odd.\"")]
#[case::a_mana_ability("Enchanted creature has \"{T}: Add {G}.\"")]
#[case::a_keyword_for_a_land("Enchanted land has flying")]
#[case::a_bonus_for_a_land("Enchanted land gets +1/+1 and has \"{T}: Draw a card.\"")]
#[case::another_noun("Enchanted Forest has \"{T}: Draw a card.\"")]
fn near_misses_of_giving_an_ability_stay_unread(#[case] text: &str) {
    assert_eq!(statics(text), None, "{text}");
}

#[rstest]
#[case::two_other_players("Target player loses 2 life and target opponent gains 2 life.")]
#[case::a_dangling_and("Target player loses 2 life and you")]
#[case::you_with_no_verb("Target player loses 2 life and you a card.")]
fn near_misses_of_a_second_player_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[test]
fn copies_of_other_clause_kinds_are_not_confused() {
    assert_eq!(trigger("When ~ enters, copy it."), None);
    assert_eq!(activated("{T}: Copy target creature."), None);
}
