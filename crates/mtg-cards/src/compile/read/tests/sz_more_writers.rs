//! Goal: the writers of the sample of S to Z give the oracle wording of a card cast from a zone, damage to the controller of a target, a creature among the creatures you control, damage to two kinds of permanents, cards looked at and sorted, a target other than the source, an untap of lands, tokens after another step, a bounce with an exception, a creature of one size, cards drawn for a characteristic, a count of a spell, a bonus for each card, a graveyard from the battlefield and a sacrifice the opponents choose, and a wrong zone, player, count, type or step never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn avatar(zone: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"if":{{"Matches":{{"subject":"This","selector":[{{"CastFrom":"{zone}"}}]}}}},"effects":[{{"Destroy":{{"what":{{"Each":[{{"Type":"Creature"}},{{"Not":{{"Subtype":"Dinosaur"}}}}]}}}}}}]}}}}"#
    )
}

#[rstest]
#[case::hand(
    "Hand",
    "When this creature enters, if you cast it from your hand, destroy all non-Dinosaur creatures."
)]
#[case::graveyard(
    "Graveyard",
    "When this creature enters, if you cast it from your graveyard, destroy all non-Dinosaur creatures."
)]
fn a_trigger_may_need_the_card_cast_from_a_zone(#[case] zone: &str, #[case] clause: &str) {
    assert!(written(&avatar(zone), clause), "{clause}");
}

#[rstest]
#[case::the_other_zone(
    "Hand",
    "When this creature enters, if you cast it from your graveyard, destroy all non-Dinosaur creatures."
)]
#[case::no_condition(
    "Hand",
    "When this creature enters, destroy all non-Dinosaur creatures."
)]
#[case::another_group(
    "Hand",
    "When this creature enters, if you cast it from your hand, destroy all Dinosaur creatures."
)]
#[case::exile(
    "Hand",
    "When this creature enters, if you cast it from exile, destroy all non-Dinosaur creatures."
)]
fn a_cast_from_a_zone_does_not_read_as_another_zone_or_group(
    #[case] zone: &str,
    #[case] clause: &str,
) {
    assert!(!written(&avatar(zone), clause), "{clause}");
}

fn disintegration(amount: u32, count: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"Destroy":{{"what":{{"Target":0}}}}}},{{"If":{{"condition":{{"Compare":{{"left":{{"Count":[{{"Type":"{count}"}},{{"ControlledBy":"You"}}]}},"op":"Ge","right":1}}}},"then":[{{"DealDamage":{{"to":{{"ControllerOf":{{"Target":0}}}},"amount":{amount}}}}}]}}}}]}}}}"#
    )
}

#[test]
fn damage_may_go_to_the_controller_of_the_target() {
    let clause = "Destroy target creature. If you control an artifact, ~ deals 3 damage to that creature's controller.";
    assert!(written(&disintegration(3, "Artifact"), clause));
}

#[rstest]
#[case::another_number(
    3,
    "Artifact",
    "Destroy target creature. If you control an artifact, ~ deals 4 damage to that creature's controller."
)]
#[case::the_player_of_an_event(
    3,
    "Artifact",
    "Destroy target creature. If you control an artifact, ~ deals 3 damage to that player."
)]
#[case::a_permanent(
    3,
    "Artifact",
    "Destroy target creature. If you control an artifact, ~ deals 3 damage to that permanent's controller."
)]
#[case::another_condition(
    3,
    "Creature",
    "Destroy target creature. If you control an artifact, ~ deals 3 damage to that creature's controller."
)]
fn damage_to_a_controller_does_not_read_as_other_damage(
    #[case] amount: u32,
    #[case] count: &str,
    #[case] clause: &str,
) {
    assert!(!written(&disintegration(amount, count), clause), "{clause}");
}

fn verdant(stat: &str, group: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":[{{"Type":"Creature"}},{group}],"to":"Battlefield"}}}},"effects":[{{"GainLife":{{"amount":{{"Stat":{{"of":"Triggering","stat":"{stat}"}}}}}}}}]}}}}"#
    )
}

const YOURS: &str = r#"{"ControlledBy":"You"}"#;
const THEIRS: &str = r#"{"ControlledBy":"Opponents"}"#;

#[rstest]
#[case::the_card_among_them(
    "Whenever this creature or another creature you control enters, you gain life equal to that creature's toughness."
)]
#[case::the_group(
    "Whenever a creature you control enters, you gain life equal to that creature's toughness."
)]
fn a_creature_you_control_may_include_the_card(#[case] clause: &str) {
    assert!(written(&verdant("Toughness", YOURS), clause), "{clause}");
}

#[rstest]
#[case::power(
    YOURS,
    "Whenever this creature or another creature you control enters, you gain life equal to that creature's power."
)]
#[case::another_creature(
    YOURS,
    "Whenever another creature you control enters, you gain life equal to that creature's toughness."
)]
#[case::an_opponent(
    THEIRS,
    "Whenever this creature or another creature an opponent controls enters, you gain life equal to that creature's toughness."
)]
fn a_creature_that_enters_does_not_read_as_another_group_or_stat(
    #[case] group: &str,
    #[case] clause: &str,
) {
    assert!(!written(&verdant("Toughness", group), clause), "{clause}");
}

fn wrath(second: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"DealDamage":{{"to":{{"Each":[{{"AnyOf":[{{"Type":"Creature"}},{{"Type":"{second}"}}]}}]}},"amount":4}}}}]}}}}"#
    )
}

#[test]
fn damage_to_two_kinds_of_permanents_names_each() {
    assert!(written(
        &wrath("Planeswalker"),
        "~ deals 4 damage to each creature and each planeswalker."
    ));
}

#[rstest]
#[case::another_type("Artifact", "~ deals 4 damage to each creature and each planeswalker.")]
#[case::each_player("Planeswalker", "~ deals 4 damage to each creature and each player.")]
#[case::a_second_each_left_out(
    "Planeswalker",
    "~ deals 4 damage to each creature and planeswalker."
)]
fn damage_to_two_kinds_does_not_read_as_other_kinds(#[case] second: &str, #[case] clause: &str) {
    assert!(!written(&wrath(second), clause), "{clause}");
}

const SEA_GATE: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"LookAtTop":{"count":2,"as":"top"}},{"Choose":{"from":{"Bound":"top"},"as":"kept","rest_as":"rest"}},{"Move":{"what":{"Bound":"kept"},"to":"Hand"}},{"Move":{"what":{"Bound":"rest"},"to":{"Library":{"place":"Bottom"}}}}]}}"#;
const TELLING_TIME: &str = r#"{"Spell":{"effects":[{"LookAtTop":{"count":3,"as":"top"}},{"Choose":{"from":{"Bound":"top"},"as":"kept","rest_as":"others"}},{"Move":{"what":{"Bound":"kept"},"to":"Hand"}},{"Choose":{"from":{"Bound":"others"},"as":"ontop","rest_as":"bottomed"}},{"Move":{"what":{"Bound":"ontop"},"to":{"Library":{"place":"Top"}}}},{"Move":{"what":{"Bound":"bottomed"},"to":{"Library":{"place":"Bottom"}}}}]}}"#;

#[rstest]
#[case::two_cards(
    SEA_GATE,
    "When this creature enters, look at the top two cards of your library. Put one of them into your hand and the other on the bottom of your library."
)]
#[case::three_cards(
    TELLING_TIME,
    "Look at the top three cards of your library. Put one of those cards into your hand, one on top of your library, and one on the bottom of your library."
)]
fn the_top_cards_are_looked_at_one_kept_and_the_rest_put_in_order(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::the_graveyard(
    SEA_GATE,
    "When this creature enters, look at the top two cards of your library. Put one of them into your hand and the other into your graveyard."
)]
#[case::three_for_two(
    SEA_GATE,
    "When this creature enters, look at the top three cards of your library. Put one of them into your hand and the other on the bottom of your library."
)]
#[case::the_top_first(
    TELLING_TIME,
    "Look at the top three cards of your library. Put one of those cards into your hand, one on the bottom of your library, and one on top of your library."
)]
#[case::two_for_three(
    TELLING_TIME,
    "Look at the top two cards of your library. Put one of those cards into your hand, one on top of your library, and one on the bottom of your library."
)]
fn cards_looked_at_do_not_read_as_another_sorting(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

fn sorcerer(toughness: u32, duration: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":["Tap"],"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},"Other"]}}}}],"effects":[{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"SetPT":{{"power":0,"toughness":{toughness}}}}}],"duration":"{duration}"}}}}]}}}}"#
    )
}

#[test]
fn a_target_other_than_the_source_has_its_duration_last() {
    let clause = "{T}: Target creature other than this creature has base power and toughness 0/2 until end of turn.";
    assert!(written(&sorcerer(2, "UntilEndOfTurn"), clause));
}

#[rstest]
#[case::another_toughness(
    2,
    "UntilEndOfTurn",
    "{T}: Target creature other than this creature has base power and toughness 0/3 until end of turn."
)]
#[case::no_duration(
    2,
    "UntilEndOfTurn",
    "{T}: Target creature other than this creature has base power and toughness 0/2."
)]
#[case::any_creature(
    2,
    "UntilEndOfTurn",
    "{T}: Target creature has base power and toughness 0/2 until end of turn."
)]
fn a_target_other_than_the_source_does_not_read_as_another_target(
    #[case] toughness: u32,
    #[case] duration: &str,
    #[case] clause: &str,
) {
    assert!(!written(&sorcerer(toughness, duration), clause), "{clause}");
}

fn snap(count: u32, kind: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"Move":{{"what":{{"Target":0}},"to":"Hand"}}}},{{"Choose":{{"from":{{"Each":[{{"Type":"{kind}"}},{{"ControlledBy":"You"}}]}},"count":{{"min":0,"max":{count}}},"as":"lands"}}}},{{"Untap":{{"what":{{"Bound":"lands"}}}}}}]}}}}"#
    )
}

#[test]
fn an_untap_of_lands_leaves_out_who_controls_them() {
    let clause = "Return target creature to its owner's hand. Untap up to two lands.";
    assert!(written(&snap(2, "Land"), clause));
}

#[rstest]
#[case::three_lands(
    2,
    "Land",
    "Return target creature to its owner's hand. Untap up to three lands."
)]
#[case::creatures(
    2,
    "Creature",
    "Return target creature to its owner's hand. Untap up to two lands."
)]
fn an_untap_of_lands_does_not_read_as_another_untap(
    #[case] count: u32,
    #[case] kind: &str,
    #[case] clause: &str,
) {
    assert!(!written(&snap(count, kind), clause), "{clause}");
}

fn fae(count: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"Compare":{{"stat":"ManaValue","op":"Eq","to":"X"}}}}]}}}}],"effects":[{{"Move":{{"what":{{"Target":0}},"to":"Hand"}}}},{{"CreateToken":{{"token":{{"Custom":{{"name":"Faerie","colors":["Blue"],"types":["Creature"],"subtypes":["Faerie"],"power":1,"toughness":1,"abilities":[{{"Keyword":"Flying"}}]}}}},"count":{count}}}}}]}}}}"#
    )
}

#[rstest]
#[case::you_create(
    "\"X\"",
    "Return target creature with mana value X to its owner's hand. You create X 1/1 blue Faerie creature tokens with flying."
)]
#[case::then_create(
    "\"X\"",
    "Return target creature with mana value X to its owner's hand. Then create X 1/1 blue Faerie creature tokens with flying."
)]
#[case::a_number(
    "2",
    "Return target creature with mana value X to its owner's hand. You create two 1/1 blue Faerie creature tokens with flying."
)]
fn tokens_after_another_step_may_open_with_you_create(#[case] count: &str, #[case] clause: &str) {
    assert!(written(&fae(count), clause), "{clause}");
}

#[test]
fn tokens_after_another_step_do_not_read_as_another_count() {
    let clause = "Return target creature with mana value X to its owner's hand. You create two 1/1 blue Faerie creature tokens with flying.";
    assert!(!written(&fae("\"X\""), clause));
}

fn wave(kinds: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Move":{{"what":{{"Each":[{{"Type":"Creature"}},{{"Not":{{"AnyOf":[{kinds}]}}}}]}},"to":"Hand"}}}}]}}}}"#
    )
}

const FOUR: &str =
    r#"{"Subtype":"Kraken"},{"Subtype":"Leviathan"},{"Subtype":"Octopus"},{"Subtype":"Serpent"}"#;

#[test]
fn a_bounce_of_every_creature_may_leave_out_some_types() {
    let clause = "Return all creatures to their owners' hands except for Krakens, Leviathans, Octopuses, and Serpents.";
    assert!(written(&wave(FOUR), clause));
}

#[rstest]
#[case::two_types(
    r#"{"Subtype":"Kraken"},{"Subtype":"Leviathan"}"#,
    "Return all creatures to their owners' hands except for Krakens, Leviathans, Octopuses, and Serpents."
)]
#[case::a_type_too_many(
    FOUR,
    "Return all creatures to their owners' hands except for Krakens and Leviathans."
)]
#[case::no_exception(FOUR, "Return all creatures to their owners' hands.")]
fn a_bounce_with_an_exception_does_not_read_as_another_exception(
    #[case] kinds: &str,
    #[case] clause: &str,
) {
    assert!(!written(&wave(kinds), clause), "{clause}");
}

fn captain(power: u32, toughness: u32) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}],"to":"Battlefield"}}}},"if":{{"Matches":{{"subject":"Triggering","selector":[{{"Compare":{{"stat":"Power","op":"Eq","to":{power}}}}},{{"Compare":{{"stat":"Toughness","op":"Eq","to":{toughness}}}}}]}}}},"effects":[{{"PutCounters":{{"on":"Triggering","counter":"+1/+1","count":2}}}}]}}}}"#
    )
}

#[test]
fn a_creature_of_one_size_is_a_condition() {
    let clause = "Whenever a creature you control enters, if that creature is 1/1, put two +1/+1 counters on it.";
    assert!(written(&captain(1, 1), clause));
}

#[rstest]
#[case::another_size(
    2,
    2,
    "Whenever a creature you control enters, if that creature is 1/1, put two +1/+1 counters on it."
)]
#[case::no_size(
    1,
    1,
    "Whenever a creature you control enters, put two +1/+1 counters on it."
)]
#[case::another_count(
    1,
    1,
    "Whenever a creature you control enters, if that creature is 1/1, put three +1/+1 counters on it."
)]
fn a_creature_of_one_size_does_not_read_as_another_size(
    #[case] power: u32,
    #[case] toughness: u32,
    #[case] clause: &str,
) {
    assert!(!written(&captain(power, toughness), clause), "{clause}");
}

fn majesty(stat: &str, who: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"ControlledBy":"{who}"}}]}}}}],"effects":[{{"Draw":{{"count":{{"Stat":{{"of":{{"Target":0}},"stat":"{stat}"}}}}}}}}]}}}}"#
    )
}

#[test]
fn cards_may_be_drawn_for_the_power_of_a_target() {
    let clause = "Draw cards equal to the power of target creature you control.";
    assert!(written(&majesty("Power", "You"), clause));
}

#[rstest]
#[case::toughness(
    "Power",
    "You",
    "Draw cards equal to the toughness of target creature you control."
)]
#[case::any_creature("Power", "You", "Draw cards equal to the power of target creature.")]
#[case::an_opponent(
    "Power",
    "Opponents",
    "Draw cards equal to the power of target creature you control."
)]
fn cards_drawn_for_a_target_do_not_read_as_another_count(
    #[case] stat: &str,
    #[case] who: &str,
    #[case] clause: &str,
) {
    assert!(!written(&majesty(stat, who), clause), "{clause}");
}

fn swindle(stat: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":["Spell"]}}}}],"effects":[{{"Counter":{{"what":{{"Target":0}}}}}},{{"CreateToken":{{"token":{{"Named":"treasure"}},"count":{{"Stat":{{"of":{{"Target":0}},"stat":"{stat}"}}}}}}}}]}}}}"#
    )
}

#[test]
fn a_count_taken_from_a_countered_spell_names_the_spell() {
    let clause =
        "Counter target spell. Create X Treasure tokens, where X is that spell's mana value.";
    assert!(written(&swindle("ManaValue"), clause));
}

#[rstest]
#[case::power(
    "ManaValue",
    "Counter target spell. Create X Treasure tokens, where X is that spell's power."
)]
#[case::that_permanent(
    "ManaValue",
    "Counter target spell. Create X Treasure tokens, where X is that permanent's mana value."
)]
fn a_count_taken_from_a_spell_does_not_read_as_another_characteristic(
    #[case] stat: &str,
    #[case] clause: &str,
) {
    assert!(!written(&swindle(stat), clause), "{clause}");
}

fn wight(whose: &str) -> String {
    let count = format!(
        r#"{{"Count":[{{"Type":"Creature"}},{{"In":"Graveyard"}},{{"OwnedBy":"{whose}"}}]}}"#
    );
    format!(
        r#"{{"Static":{{"affected":"This","mods":[{{"ModifyPT":{{"power":{count},"toughness":{count}}}}}]}}}}"#
    )
}

#[test]
fn a_bonus_for_each_card_is_one_for_each() {
    let clause = "This creature gets +1/+1 for each creature card in your opponents' graveyards.";
    assert!(written(&wight("Opponents"), clause));
}

#[rstest]
#[case::two("This creature gets +2/+2 for each creature card in your opponents' graveyards.")]
#[case::your_graveyard("This creature gets +1/+1 for each creature card in your graveyard.")]
#[case::another_card("This creature gets +1/+1 for each land card in your opponents' graveyards.")]
fn a_bonus_for_each_card_does_not_read_as_another_bonus(#[case] clause: &str) {
    assert!(!written(&wight("Opponents"), clause), "{clause}");
}

fn schematic(to: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"AnyOf":[{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},{{"ZoneChange":{{"what":["This"],"from":"Battlefield","to":"{to}"}}}}]}},"effects":[{{"CreateToken":{{"token":{{"Custom":{{"name":"Servo","types":["Artifact","Creature"],"subtypes":["Servo"],"power":1,"toughness":1}}}}}}}}]}}}}"#
    )
}

#[test]
fn a_card_that_enters_or_is_put_into_a_graveyard_triggers() {
    let clause = "When this artifact enters or is put into a graveyard from the battlefield, create a 1/1 colorless Servo artifact creature token.";
    assert!(written(&schematic("Graveyard"), clause));
}

#[rstest]
#[case::exile(
    "Exile",
    "When this artifact enters or is put into a graveyard from the battlefield, create a 1/1 colorless Servo artifact creature token."
)]
#[case::two_tokens(
    "Graveyard",
    "When this artifact enters or is put into a graveyard from the battlefield, create two 1/1 colorless Servo artifact creature tokens."
)]
fn a_card_that_enters_or_leaves_does_not_read_as_another_event(
    #[case] to: &str,
    #[case] clause: &str,
) {
    assert!(!written(&schematic(to), clause), "{clause}");
}

fn vraska(counter: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Sacrifice":{{"who":"Opponents","filter":[{{"AnyOf":[{{"Type":"Creature"}},{{"Type":"Planeswalker"}}]}}]}}}},{{"PutCounters":{{"on":"Opponents","counter":"{counter}"}}}}]}}}}"#
    )
}

#[test]
fn opponents_who_sacrifice_of_their_choice_also_get_counters() {
    let clause = "Each opponent sacrifices a creature or planeswalker of their choice and gets a poison counter.";
    assert!(written(&vraska("poison"), clause));
}

#[rstest]
#[case::another_counter(
    "charge",
    "Each opponent sacrifices a creature or planeswalker of their choice and gets a poison counter."
)]
#[case::another_type(
    "poison",
    "Each opponent sacrifices a creature of their choice and gets a poison counter."
)]
fn a_sacrifice_of_their_choice_does_not_read_as_another_step(
    #[case] counter: &str,
    #[case] clause: &str,
) {
    assert!(!written(&vraska(counter), clause), "{clause}");
}

fn setessan(draws: u32) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":[{{"Type":"Enchantment"}},{{"ControlledBy":"You"}}],"to":"Battlefield"}}}},"effects":[{{"PutCounters":{{"on":"This","counter":"+1/+1"}}}},{{"Draw":{{"count":{draws}}}}}]}}}}"#
    )
}

#[test]
fn counters_on_the_source_and_a_draw_are_joined_by_and() {
    let clause = "Whenever an enchantment you control enters, put a +1/+1 counter on this creature and draw a card.";
    assert!(written(&setessan(1), clause));
}

#[test]
fn counters_and_a_draw_of_another_count_do_not_read_back() {
    let clause = "Whenever an enchantment you control enters, put a +1/+1 counter on this creature and draw two cards.";
    assert!(!written(&setessan(1), clause));
}
