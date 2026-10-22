//! Goal: the static wordings (entering tapped, with counters, after a choice or a payment, unless something is controlled; a spell that can't be countered; a creature that can't block or must attack; a bonus or a keyword for a creature, an attached creature or a group) read into the shapes of the hand-written files, write back as the same text, and leave every near-miss to the table.
use super::*;

fn stat(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Static, false)
}

const ENTERS: &str = r#""when":{"Enters":{"what":["This"]}}"#;

fn replacing(by: &str) -> String {
    format!(r#"{{"Replacement":{{{ENTERS},"by":{by}}}}}"#)
}

fn replacing_if(by: &str, condition: &str) -> String {
    format!(r#"{{"Replacement":{{{ENTERS},"by":{by},"condition":{condition}}}}}"#)
}

#[rstest]
#[case::a_land("This land enters tapped.", "\"Tapped\"")]
#[case::an_artifact("This artifact enters tapped.", "\"Tapped\"")]
#[case::a_creature("This creature enters tapped.", "\"Tapped\"")]
#[case::the_card_name("~ enters tapped.", "\"Tapped\"")]
#[case::one_counter(
    "This creature enters with a +1/+1 counter on it.",
    r#"{"WithCounters":{"counter":"+1/+1","count":1}}"#
)]
#[case::three_counters(
    "This creature enters with three +1/+1 counters on it.",
    r#"{"WithCounters":{"counter":"+1/+1","count":3}}"#
)]
#[case::minus_counters(
    "This creature enters with two -1/-1 counters on it.",
    r#"{"WithCounters":{"counter":"-1/-1","count":2}}"#
)]
#[case::charge_counters(
    "This artifact enters with three charge counters on it.",
    r#"{"WithCounters":{"counter":"charge","count":3}}"#
)]
#[case::a_named_kind(
    "This creature enters with an oil counter on it.",
    r#"{"WithCounters":{"counter":{"Other":"oil"},"count":1}}"#
)]
#[case::a_color("As this artifact enters, choose a color.", r#"{"Choose":"Color"}"#)]
#[case::a_creature_type(
    "As this enchantment enters, choose a creature type.",
    r#"{"Choose":"CreatureType"}"#
)]
#[case::pay_life(
    "As this land enters, you may pay 3 life. If you don't, it enters tapped.",
    r#"{"PayLifeOrTapped":3}"#
)]
#[case::a_player("As this creature enters, choose a player.", r#"{"Choose":"Player"}"#)]
#[case::an_opponent(
    "As this enchantment enters, choose an opponent.",
    r#"{"Choose":"Opponent"}"#
)]
fn a_permanent_enters_as_the_hand_written_files_say(#[case] text: &str, #[case] by: &str) {
    reads_as(text, ClauseKind::Static, false, &replacing(by));
}

#[test]
fn x_counters_need_an_x_in_the_mana_cost() {
    let text = "This creature enters with X +1/+1 counters on it.";
    reads_as(
        text,
        ClauseKind::Static,
        true,
        &replacing(r#"{"WithCounters":{"counter":"+1/+1","count":"X"}}"#),
    );
    assert_eq!(stat(text), None);
}

#[rstest]
#[case::basic_lands(
    "This land enters tapped unless you control two or more basic lands.",
    r#"{"Not":{"Compare":{"left":{"Count":[{"Type":"Land"},{"Supertype":"Basic"},{"ControlledBy":"You"}]},"op":"Ge","right":2}}}"#
)]
#[case::other_lands_or_more(
    "This land enters tapped unless you control two or more other lands.",
    r#"{"Not":{"Compare":{"left":{"Count":[{"Type":"Land"},"Other",{"ControlledBy":"You"}]},"op":"Ge","right":2}}}"#
)]
#[case::other_lands_or_fewer(
    "This land enters tapped unless you control two or fewer other lands.",
    r#"{"Not":{"Compare":{"left":{"Count":[{"Type":"Land"},"Other",{"ControlledBy":"You"}]},"op":"Le","right":2}}}"#
)]
#[case::a_planeswalker(
    "This land enters tapped unless you control a planeswalker.",
    r#"{"Not":{"Compare":{"left":{"Count":[{"Type":"Planeswalker"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}}"#
)]
#[case::a_basic_land(
    "This land enters tapped unless you control a basic land.",
    r#"{"Not":{"Compare":{"left":{"Count":[{"Type":"Land"},{"Supertype":"Basic"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}}"#
)]
#[case::a_legendary_creature(
    "This land enters tapped unless you control a legendary creature.",
    r#"{"Not":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"Supertype":"Legendary"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}}"#
)]
#[case::a_land_type(
    "This land enters tapped unless you control a Forest.",
    r#"{"Not":{"Compare":{"left":{"Count":[{"Subtype":"Forest"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}}"#
)]
#[case::one_of_two_land_types(
    "This land enters tapped unless you control a Forest or a Plains.",
    r#"{"Not":{"Compare":{"left":{"Count":[{"AnyOf":[{"Subtype":"Forest"},{"Subtype":"Plains"}]},{"ControlledBy":"You"}]},"op":"Ge","right":1}}}"#
)]
#[case::a_vehicle_or_mount(
    "This land enters tapped unless you control a Mount or Vehicle.",
    r#"{"Not":{"Compare":{"left":{"Count":[{"AnyOf":[{"Subtype":"Mount"},{"Subtype":"Vehicle"}]},{"ControlledBy":"You"}]},"op":"Ge","right":1}}}"#
)]
#[case::opponents(
    "This land enters tapped unless your opponents control eight or more lands.",
    r#"{"Not":{"Compare":{"left":{"Count":[{"Type":"Land"},{"ControlledBy":"Opponents"}]},"op":"Ge","right":8}}}"#
)]
#[case::two_opponents(
    "This land enters tapped unless you have two or more opponents.",
    r#"{"Not":{"Compare":{"left":{"Players":"Opponents"},"op":"Ge","right":2}}}"#
)]
#[case::three_opponents(
    "This land enters tapped unless you have three or more opponents.",
    r#"{"Not":{"Compare":{"left":{"Players":"Opponents"},"op":"Ge","right":3}}}"#
)]
#[case::a_low_life_total(
    "This land enters tapped unless a player has 13 or less life.",
    r#"{"Not":{"Compare":{"left":{"LowestLife":"EachPlayer"},"op":"Le","right":13}}}"#
)]
#[case::an_opponent_with_a_low_life_total(
    "This land enters tapped unless an opponent has 10 or less life.",
    r#"{"Not":{"Compare":{"left":{"LowestLife":"Opponents"},"op":"Le","right":10}}}"#
)]
#[case::every_opponent_with_a_low_life_total(
    "This land enters tapped unless each opponent has 10 or less life.",
    r#"{"Not":{"Compare":{"left":{"HighestLife":"Opponents"},"op":"Le","right":10}}}"#
)]
#[case::an_opponent_with_a_high_life_total(
    "This land enters tapped unless an opponent has 30 or more life.",
    r#"{"Not":{"Compare":{"left":{"HighestLife":"Opponents"},"op":"Ge","right":30}}}"#
)]
#[case::every_opponent_with_a_high_life_total(
    "This land enters tapped unless each opponent has 30 or more life.",
    r#"{"Not":{"Compare":{"left":{"LowestLife":"Opponents"},"op":"Ge","right":30}}}"#
)]
#[case::every_player_with_a_low_life_total(
    "This land enters tapped unless each player has 13 or less life.",
    r#"{"Not":{"Compare":{"left":{"HighestLife":"EachPlayer"},"op":"Le","right":13}}}"#
)]
#[case::opponents_or_life(
    "This land enters tapped unless you have two or more opponents or a player has 13 or less life.",
    r#"{"Not":{"Any":[{"Compare":{"left":{"Players":"Opponents"},"op":"Ge","right":2}},{"Compare":{"left":{"LowestLife":"EachPlayer"},"op":"Le","right":13}}]}}"#
)]
#[case::lands_or_opponents(
    "This land enters tapped unless you control two or more other lands or you have two or more opponents.",
    r#"{"Not":{"Any":[{"Compare":{"left":{"Count":[{"Type":"Land"},"Other",{"ControlledBy":"You"}]},"op":"Ge","right":2}},{"Compare":{"left":{"Players":"Opponents"},"op":"Ge","right":2}}]}}"#
)]
fn entering_tapped_unless_reads_a_count_of_what_is_controlled(
    #[case] text: &str,
    #[case] condition: &str,
) {
    reads_as(
        text,
        ClauseKind::Static,
        false,
        &replacing_if("\"Tapped\"", condition),
    );
}

#[test]
fn sodden_verdure_reads_in_the_shape_of_its_file() {
    let hand_written = include_str!("../../../../cards/sodden_verdure.json");
    let card: serde_json::Value = serde_json::from_str(hand_written).unwrap();
    let file = card["faces"][0]["abilities"][0].clone();
    let read = golden(
        "This land enters tapped unless you control two or more basic lands.",
        ClauseKind::Static,
        false,
    );
    assert_eq!(serde_json::to_value(read).unwrap(), file);
}

#[rstest]
#[case::cant_be_countered(
    "This spell can't be countered.",
    r#"{"Rule":{"rule":{"CantBeCountered":"This"},"from":"Stack"}}"#
)]
#[case::attacks(
    "This creature attacks each combat if able.",
    r#"{"Rule":{"rule":{"MustAttack":"This"}}}"#
)]
#[case::blocks(
    "This creature blocks each combat if able.",
    r#"{"Rule":{"rule":{"MustBlock":"This"}}}"#
)]
#[case::all_attack(
    "All creatures attack each combat if able.",
    r#"{"Rule":{"rule":{"MustAttack":{"Each":[{"Type":"Creature"}]}}}}"#
)]
#[case::your_creatures_attack(
    "Creatures you control attack each combat if able.",
    r#"{"Rule":{"rule":{"MustAttack":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}}"#
)]
#[case::reveal_the_top(
    "Play with the top card of your library revealed.",
    r#"{"Rule":{"rule":{"RevealTop":"You"}}}"#
)]
#[case::no_maximum_hand_size(
    "You have no maximum hand size.",
    r#"{"Rule":{"rule":{"NoMaxHandSize":"You"}}}"#
)]
#[case::every_player_has_no_maximum_hand_size(
    "Players have no maximum hand size.",
    r#"{"Rule":{"rule":{"NoMaxHandSize":"EachPlayer"}}}"#
)]
#[case::look_at_the_top(
    "You may look at the top card of your library any time.",
    r#"{"Rule":{"rule":{"LookAtTopAnyTime":"You"}}}"#
)]
#[case::an_additional_land(
    "You may play an additional land on each of your turns.",
    r#"{"Rule":{"rule":{"AdditionalLandPlays":{}}}}"#
)]
#[case::an_additional_land_for_each_player(
    "Each player may play an additional land on each of their turns.",
    r#"{"Rule":{"rule":{"AdditionalLandPlays":{"who":"EachPlayer"}}}}"#
)]
#[case::a_commander(
    "This creature can be your commander.",
    r#"{"Rule":{"rule":"CanBeCommander"}}"#
)]
#[case::unblockable(
    "This creature can't be blocked.",
    r#"{"Static":{"affected":"This","mods":["CantBeBlocked"]}}"#
)]
#[case::unblockable_by_the_small(
    "This creature can't be blocked by creatures with power 2 or less.",
    r#"{"Static":{"affected":"This","mods":[{"CantBeBlockedBy":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":2}}]}]}}"#
)]
#[case::unblockable_by_fliers(
    "This creature can't be blocked by creatures with flying.",
    r#"{"Static":{"affected":"This","mods":[{"CantBeBlockedBy":[{"Type":"Creature"},{"HasKeyword":"Flying"}]}]}}"#
)]
#[case::a_blocker_that_cannot_be_blocked(
    "This creature can't block and can't be blocked.",
    r#"{"Static":{"affected":"This","mods":["CantBlock","CantBeBlocked"]}}"#
)]
#[case::an_aura_makes_unblockable(
    "Enchanted creature can't be blocked.",
    r#"{"Static":{"affected":"Attached","mods":["CantBeBlocked"]}}"#
)]
#[case::a_group_is_unblockable(
    "Creatures you control can't be blocked.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":["CantBeBlocked"]}}"#
)]
#[case::cant_block(
    "This creature can't block.",
    r#"{"Static":{"affected":"This","mods":["CantBlock"]}}"#
)]
#[case::cant_attack(
    "This creature can't attack.",
    r#"{"Static":{"affected":"This","mods":["CantAttack"]}}"#
)]
#[case::an_aura_forbids(
    "Enchanted creature can't attack or block.",
    r#"{"Static":{"affected":"Attached","mods":["CantAttack","CantBlock"]}}"#
)]
fn rules_and_restrictions_read_as_the_hand_written_files_say(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::an_equipment(
    "Equipped creature gets +2/+0.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":2,"toughness":0}}]}}"#
)]
#[case::an_aura_with_a_keyword(
    "Enchanted creature gets +5/+5 and has trample.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":5,"toughness":5}},{"AddKeyword":"Trample"}]}}"#
)]
#[case::two_keywords(
    "Equipped creature has hexproof and haste.",
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":"Hexproof"},{"AddKeyword":"Haste"}]}}"#
)]
#[case::a_penalty(
    "Enchanted creature gets -1/-1.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":-1,"toughness":-1}}]}}"#
)]
#[case::your_creatures(
    "Creatures you control get +1/+1.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#
)]
#[case::the_others(
    "Other creatures you control get +1/+1.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#
)]
#[case::a_keyword_for_a_group(
    "Creatures you control have haste.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Haste"}]}}"#
)]
#[case::the_opponents(
    "Creatures your opponents control get -2/-2.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"mods":[{"ModifyPT":{"power":-2,"toughness":-2}}]}}"#
)]
#[case::every_creature(
    "All creatures get +1/+1.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#
)]
#[case::the_chosen_type(
    "Creatures you control of the chosen type get +1/+1.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},"ChosenSubtype"]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#
)]
#[case::the_chosen_color(
    "Creatures you control of the chosen color get +1/+1.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},"ChosenColor"]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#
)]
fn a_bonus_or_a_keyword_reads_as_the_hand_written_files_say(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::two_sentences("This land enters tapped. As it enters, choose a color.")]
#[case::counters_for_each(
    "This creature enters with a +1/+1 counter on it for each creature you control."
)]
#[case::counters_if("This creature enters with a +1/+1 counter on it if you attacked this turn.")]
#[case::counters_the_rules_define("This creature enters with a shield counter on it.")]
#[case::counters_of_two_kinds(
    "This creature enters with a +1/+1 counter and a charge counter on it."
)]
#[case::an_x_without_an_x_in_the_cost("This creature enters with X +1/+1 counters on it.")]
#[case::an_amount_of_digits("This creature enters with 2 +1/+1 counters on it.")]
#[case::unless_a_turn("This creature enters tapped unless it's your turn.")]
#[case::unless_one_or_more("This land enters tapped unless you control one or more basic lands.")]
#[case::unless_an_opponent("This land enters tapped unless your opponents control a land.")]
#[case::unless_two_colors(
    "This land enters tapped unless you control a white, blue, or black permanent."
)]
#[case::unless_you_revealed(
    "This land enters tapped unless you revealed a Soldier card this way or you control a Soldier."
)]
#[case::pay_no_life("As this land enters, you may pay 0 life. If you don't, it enters tapped.")]
#[case::choose_two("As this creature enters, choose a color and a creature type.")]
#[case::choose_a_player_in_secret("As this creature enters, secretly choose an opponent.")]
#[case::unless_one_opponent("This land enters tapped unless you have one opponent.")]
#[case::unless_a_player_has_exactly_this_life(
    "This land enters tapped unless a player has 13 life."
)]
#[case::unless_a_player_has_more_than_this_life(
    "This land enters tapped unless a player has more than 13 life."
)]
#[case::unless_nobody_has_life("This land enters tapped unless a player has life.")]
#[case::unblockable_except_by(
    "This creature can't be blocked except by creatures with flying or reach."
)]
#[case::unblockable_by_more_than_one("This creature can't be blocked by more than one creature.")]
#[case::unblockable_this_turn("Creatures you control can't be blocked this turn.")]
#[case::an_additional_land_this_turn("You may play an additional land this turn.")]
#[case::hand_size_for_a_turn("You have no maximum hand size this turn.")]
#[case::look_at_the_top_of_a_library_that_is_not_yours(
    "You may look at the top card of target player's library any time."
)]
#[case::commanders_of_others("Your commander can be your commander.")]
#[case::cant_be_countered_by_a_group("Spells you cast can't be countered.")]
#[case::cant_block_unless("This creature can't block creatures with flying.")]
#[case::attacks_if_able("This creature attacks if able.")]
#[case::a_bonus_for_itself("This creature gets +1/+1.")]
#[case::a_bonus_for_a_land("Enchanted land gets +1/+1.")]
#[case::a_bonus_with_x("Equipped creature gets +X/+X.")]
#[case::a_keyword_and_an_ability("Equipped creature has flying and can't be blocked.")]
#[case::a_keyword_adjective("Flying creatures you control get +1/+1.")]
#[case::a_gain("Creatures you control gain haste.")]
#[case::a_goad("Enchanted creature gets +1/+1 and is goaded.")]
#[case::each_creature_attacks("Each creature you control attacks each combat if able.")]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(stat(text), None, "{text}");
}

#[rstest]
#[case("This land enters tapped unless you control a Swamp or a Mountain.")]
#[case("This land enters tapped unless you control an Island or a Swamp.")]
#[case("This land enters tapped unless you control a Plains or an Island.")]
#[case("This land enters tapped unless you control an Equipment.")]
#[case("This land enters tapped unless you control a legendary green creature.")]
#[case("This land enters tapped unless you control three or more basic lands.")]
#[case("This land enters tapped unless you control four or fewer other lands.")]
#[case("This artifact enters with a charge counter on it.")]
#[case("This enchantment enters with five time counters on it.")]
#[case("As this enchantment enters, you may pay 1 life. If you don't, it enters tapped.")]
#[case("Equipped creature gets +3/+3.")]
#[case("Enchanted creature gets +1/+2 and has flying and first strike.")]
#[case("Creatures you control have flying, vigilance, and haste.")]
#[case(
    "Enchanted creature gets +4/+4 and has flying, first strike, vigilance, trample, and lifelink."
)]
#[case("Equipped creature has double strike.")]
#[case("Creatures you control have lifelink.")]
#[case("Other creatures you control have trample.")]
#[case("Attacking creatures you control get +1/+0.")]
#[case("Creatures you control get +2/+2.")]
#[case("This creature can't attack or block.")]
fn more_static_wordings_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Static, false);
}

#[rstest]
#[case::an_increase_for_a_kind(
    "Noncreature spells cost {1} more to cast.",
    r#"{"CostModifier":{"applies_to":["Spell",{"Not":{"Type":"Creature"}}],"change":"Increase","mana":"{1}"}}"#
)]
#[case::a_reduction_for_what_you_cast(
    "Artifact spells you cast cost {1} less to cast.",
    r#"{"CostModifier":{"applies_to":["Spell",{"Type":"Artifact"},{"ControlledBy":"You"}],"change":"Reduce","mana":"{1}"}}"#
)]
#[case::every_spell(
    "Spells cost {1} less to cast.",
    r#"{"CostModifier":{"applies_to":["Spell"],"change":"Reduce","mana":"{1}"}}"#
)]
#[case::two_generic(
    "Creature spells you cast cost {2} less to cast.",
    r#"{"CostModifier":{"applies_to":["Spell",{"Type":"Creature"},{"ControlledBy":"You"}],"change":"Reduce","mana":"{2}"}}"#
)]
#[case::what_opponents_cast(
    "Spells your opponents cast cost {1} more to cast.",
    r#"{"CostModifier":{"applies_to":["Spell",{"ControlledBy":"Opponents"}],"change":"Increase","mana":"{1}"}}"#
)]
#[case::a_colour_of_spell(
    "Red spells you cast cost {R} less to cast.",
    r#"{"CostModifier":{"applies_to":["Spell",{"Color":"Red"},{"ControlledBy":"You"}],"change":"Reduce","mana":"{R}"}}"#
)]
#[case::a_coloured_symbol(
    "Nonland spells cost {U} more to cast.",
    r#"{"CostModifier":{"applies_to":["Spell",{"Not":{"Type":"Land"}}],"change":"Increase","mana":"{U}"}}"#
)]
#[case::this_spell_for_each_target(
    "This spell costs {1} more to cast for each target beyond the first.",
    r#"{"CostModifier":{"applies_to":["This"],"change":"Increase","mana":"{1}","per":{"Sum":[{"TargetCount":0},-1]}}}"#
)]
#[case::this_spell_for_less(
    "This spell costs {2} less to cast.",
    r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","mana":"{2}"}}"#
)]
fn a_cost_modifier_reads_as_the_hand_written_files_say(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::where_x("Spells cost {X} less to cast, where X is the number of artifacts you control.")]
#[case::as_long_as("Spells cost {1} less to cast as long as it's your turn.")]
#[case::no_amount("Spells cost less to cast.")]
#[case::an_x_in_the_mana("Spells cost {X} less to cast.")]
#[case::a_cost_not_of_mana("Spells cost {1} more to cast and you lose 1 life.")]
#[case::another_verb("Spells cost {1} more to play.")]
#[case::not_spells("Creatures you control cost {1} less to cast.")]
#[case::activated_abilities("Activated abilities cost {1} less to activate.")]
#[case::beyond_the_first_on_a_kind(
    "Noncreature spells cost {1} more to cast for each target beyond the first."
)]
fn cost_wordings_the_templates_do_not_know_stay_with_the_table(#[case] text: &str) {
    assert_eq!(stat(text), None, "{text}");
}
