//! Goal: the writers of the proposal checker give the oracle wording of an intervening condition about a hand or a graveyard, of the creature that died or the target after "where X is", of a basic land type given with the type land, of a negation repeated in a list of two types, of damage to each of up to N targets, of two events of one trigger, of the counters an object enters with that a count decides, of cards exiled and then played and of the top cards split three ways, and a wrong player, count, type, event or duration never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// "At the beginning of each player's upkeep, if that player has no cards in hand, that player draws three cards."
const NO_CARDS_DRAW: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"EachPlayer"}},"if":{"Compare":{"left":{"Count":[{"In":"Hand"},{"OwnedBy":"Triggering"}]},"op":"Eq","right":0}},"effects":[{"Draw":{"who":"Triggering","count":3}}]}}"#;

/// "Whenever ~ attacks, if there are four or more card types among cards in your graveyard, other attacking creatures
/// get +4/+4 until end of turn."
const DELIRIUM: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"if":{"Compare":{"left":{"CardTypes":[{"In":"Graveyard"},{"OwnedBy":"You"}]},"op":"Ge","right":4}},"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},"Attacking","Other"]},"mods":[{"ModifyPT":{"power":4,"toughness":4}}],"duration":"UntilEndOfTurn"}}]}}"#;

#[rstest]
#[case::the_hand_of_the_player_of_the_event(
    NO_CARDS_DRAW,
    "At the beginning of each player's upkeep, if that player has no cards in hand, that player draws three cards."
)]
#[case::they_for_that_player(
    NO_CARDS_DRAW,
    "At the beginning of each player's upkeep, if that player has no cards in hand, they draw three cards."
)]
#[case::card_types_in_the_graveyard(
    DELIRIUM,
    "Whenever ~ attacks, if there are four or more card types among cards in your graveyard, other attacking creatures get +4/+4 until end of turn."
)]
fn an_intervening_condition_is_worded_after_the_event(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::a_card_in_hand(
    NO_CARDS_DRAW,
    "At the beginning of each player's upkeep, if that player has one or more cards in hand, that player draws three cards."
)]
#[case::your_hand(
    NO_CARDS_DRAW,
    "At the beginning of each player's upkeep, if you have no cards in hand, that player draws three cards."
)]
#[case::another_step(
    NO_CARDS_DRAW,
    "At the beginning of each player's end step, if that player has no cards in hand, that player draws three cards."
)]
#[case::another_number_of_cards(
    NO_CARDS_DRAW,
    "At the beginning of each player's upkeep, if that player has no cards in hand, that player draws two cards."
)]
#[case::no_condition(
    NO_CARDS_DRAW,
    "At the beginning of each player's upkeep, that player draws three cards."
)]
#[case::five_card_types(
    DELIRIUM,
    "Whenever ~ attacks, if there are five or more card types among cards in your graveyard, other attacking creatures get +4/+4 until end of turn."
)]
#[case::cards_in_the_graveyard(
    DELIRIUM,
    "Whenever ~ attacks, if there are four or more cards in your graveyard, other attacking creatures get +4/+4 until end of turn."
)]
fn a_wrong_player_count_or_step_does_not_read_as_the_condition(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(!written(json, clause), "{clause}");
}

/// "Whenever a creature you control dies, put X +1/+1 counters on target creature you control, where X is ..."
const DEATHS_PRESENCE: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":{"Stat":{"of":"Triggering","stat":"Power"}}}}]}}"#;

/// "Destroy target artifact. It can't be regenerated. Create X 1/1 green Saproling creature tokens, where X is ..."
const ARTIFACT_MUTATION: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Artifact"}]}}],"effects":[{"Destroy":{"what":{"Target":0},"regenerate":false}},{"CreateToken":{"token":{"Custom":{"name":"Saproling","colors":["Green"],"types":["Creature"],"subtypes":["Saproling"],"power":1,"toughness":1}},"count":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}}]}}"#;

#[rstest]
#[case::the_creature_that_died(
    DEATHS_PRESENCE,
    "Whenever a creature you control dies, put X +1/+1 counters on target creature you control, where X is the power of the creature that died."
)]
#[case::the_target_by_its_type(
    ARTIFACT_MUTATION,
    "Destroy target artifact. It can't be regenerated. Create X 1/1 green Saproling creature tokens, where X is that artifact's mana value."
)]
fn the_object_of_a_where_clause_is_named(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::another_characteristic(
    DEATHS_PRESENCE,
    "Whenever a creature you control dies, put X +1/+1 counters on target creature you control, where X is the toughness of the creature that died."
)]
#[case::the_creature_that_entered(
    DEATHS_PRESENCE,
    "Whenever a creature you control dies, put X +1/+1 counters on target creature you control, where X is the power of the creature that entered."
)]
#[case::another_card_type(
    ARTIFACT_MUTATION,
    "Destroy target artifact. It can't be regenerated. Create X 1/1 green Saproling creature tokens, where X is that creature's mana value."
)]
#[case::another_characteristic_of_the_target(
    ARTIFACT_MUTATION,
    "Destroy target artifact. It can't be regenerated. Create X 1/1 green Saproling creature tokens, where X is that artifact's power."
)]
fn a_wrong_object_or_characteristic_does_not_read_in_a_where_clause(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(!written(json, clause), "{clause}");
}

/// A static ability that gives the type land and a land type.
fn lands(subtype: &str, mods: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"Type":"Creature"}},{{"Not":"Token"}},{{"ControlledBy":"You"}}]}},"mods":[{mods}]}}}}"#
    )
    .replace("SUBTYPE", subtype)
}

const FOREST_LAND: &str = r#"{"AddType":"Land"},{"AddSubtype":"SUBTYPE"}"#;
const FOREST_LAND_REVERSED: &str = r#"{"AddSubtype":"SUBTYPE"},{"AddType":"Land"}"#;

#[rstest]
#[case::forest("Forest", FOREST_LAND)]
#[case::forest_in_the_other_order("Forest", FOREST_LAND_REVERSED)]
fn a_basic_land_type_is_given_with_the_type_land(#[case] subtype: &str, #[case] mods: &str) {
    assert!(
        written(
            &lands(subtype, mods),
            "Nontoken creatures you control are Forest lands in addition to their other types."
        ),
        "{subtype}"
    );
}

#[rstest]
#[case::another_land_type(
    "Forest",
    FOREST_LAND,
    "Nontoken creatures you control are Island lands in addition to their other types."
)]
#[case::the_type_creature(
    "Forest",
    FOREST_LAND,
    "Nontoken creatures you control are Forest creatures in addition to their other types."
)]
#[case::a_subtype_that_is_no_land_type(
    "Elf",
    FOREST_LAND,
    "Nontoken creatures you control are Elf lands in addition to their other types."
)]
#[case::another_group(
    "Forest",
    FOREST_LAND,
    "Creature tokens you control are Forest lands in addition to their other types."
)]
fn another_land_type_or_group_does_not_read_as_a_basic_land_type(
    #[case] subtype: &str,
    #[case] mods: &str,
    #[case] clause: &str,
) {
    assert!(!written(&lands(subtype, mods), clause), "{clause}");
}

/// "{G}, Sacrifice this creature: Exile target ..." with the given target.
fn exile_with(preds: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{G}}"}},"SacrificeSelf"],"targets":[{{"what":{{"Object":[{preds}]}}}}],"effects":[{{"Exile":{{"what":{{"Target":0}}}}}}]}}}}"#
    )
}

const NONCREATURE_ARTIFACT_OR_ENCHANTMENT: &str =
    r#"{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]},{"Not":{"Type":"Creature"}}"#;

#[rstest]
#[case::negation_twice(
    NONCREATURE_ARTIFACT_OR_ENCHANTMENT,
    "{G}, Sacrifice ~: Exile target noncreature artifact or noncreature enchantment."
)]
#[case::negation_once(
    NONCREATURE_ARTIFACT_OR_ENCHANTMENT,
    "{G}, Sacrifice ~: Exile target noncreature artifact or enchantment."
)]
fn a_negation_heads_a_list_of_two_types(#[case] preds: &str, #[case] clause: &str) {
    assert!(written(&exile_with(preds), clause), "{clause}");
}

#[rstest]
#[case::another_type("{G}, Sacrifice ~: Exile target noncreature artifact or noncreature land.")]
#[case::another_negation(
    "{G}, Sacrifice ~: Exile target nonland artifact or noncreature enchantment."
)]
#[case::the_negation_on_the_second_type_only(
    "{G}, Sacrifice ~: Exile target artifact or noncreature enchantment."
)]
fn a_wrong_type_or_negation_does_not_read_in_a_list_of_two_types(#[case] clause: &str) {
    assert!(
        !written(&exile_with(NONCREATURE_ARTIFACT_OR_ENCHANTMENT), clause),
        "{clause}"
    );
}

/// A spell that deals damage to its one target specification with the given bounds.
fn damage_to(what: &str, bounds: &str, amount: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{what},"bounds":{bounds}}}],"effects":[{{"DealDamage":{{"to":{{"Target":0}},"amount":{amount}}}}}]}}}}"#
    )
}

const UP_TO_THREE: &str = r#"{"min":0,"max":3}"#;
const UP_TO_TWO: &str = r#"{"min":0,"max":2}"#;
const CREATURE: &str = r#"{"Object":[{"Type":"Creature"}]}"#;

#[rstest]
#[case::any_target(
    damage_to(r#""AnyTarget""#, UP_TO_THREE, r#""X""#),
    "~ deals X damage to each of up to three targets."
)]
#[case::creatures(
    damage_to(CREATURE, UP_TO_TWO, "2"),
    "~ deals 2 damage to each of up to two target creatures."
)]
fn damage_is_dealt_to_each_of_up_to_a_number_of_targets(
    #[case] json: String,
    #[case] clause: &str,
) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::fewer_targets("~ deals X damage to each of up to two targets.")]
#[case::creatures_for_any_target("~ deals X damage to each of up to three target creatures.")]
#[case::exactly_three("~ deals X damage to each of three targets.")]
#[case::one_target("~ deals X damage to any target.")]
#[case::another_amount("~ deals 3 damage to each of up to three targets.")]
fn another_number_or_kind_of_targets_does_not_read_as_each_of_up_to_three_targets(
    #[case] clause: &str,
) {
    assert!(
        !written(&damage_to(r#""AnyTarget""#, UP_TO_THREE, r#""X""#), clause),
        "{clause}"
    );
}

/// "Whenever you cycle or discard a card, you may pay {1}. If you do, create a 2/2 blue Drake creature token with
/// flying." with the given events.
fn drake_haven(first: &str, second: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"AnyOf":[{first},{second}]}},"effects":[{{"Pay":{{"cost":"{{1}}","paid":[{{"CreateToken":{{"token":{{"Custom":{{"name":"Drake","colors":["Blue"],"types":["Creature"],"subtypes":["Drake"],"power":2,"toughness":2,"abilities":[{{"Keyword":"Flying"}}]}}}}}}}}]}}}}]}}}}"#
    )
}

const CYCLED: &str = r#"{"Cycled":{"what":[],"by":"You"}}"#;
const DISCARDED: &str = r#"{"Discarded":{"what":[],"by":"You"}}"#;

#[rstest]
#[case::cycle_or_discard(
    CYCLED,
    DISCARDED,
    "Whenever you cycle or discard a card, you may pay {1}. If you do, create a 2/2 blue Drake creature token with flying."
)]
fn two_events_of_a_trigger_share_their_words(
    #[case] first: &str,
    #[case] second: &str,
    #[case] clause: &str,
) {
    assert!(written(&drake_haven(first, second), clause), "{clause}");
}

#[rstest]
#[case::another_event(
    "Whenever you cycle or exile a card, you may pay {1}. If you do, create a 2/2 blue Drake creature token with flying."
)]
#[case::one_event(
    "Whenever you cycle a card, you may pay {1}. If you do, create a 2/2 blue Drake creature token with flying."
)]
#[case::another_player(
    "Whenever an opponent cycles or discards a card, you may pay {1}. If you do, create a 2/2 blue Drake creature token with flying."
)]
#[case::another_size(
    "Whenever you cycle or discard a card, you may pay {1}. If you do, create a 3/3 blue Drake creature token with flying."
)]
fn another_event_or_player_does_not_read_as_two_events_of_a_trigger(#[case] clause: &str) {
    assert!(
        !written(&drake_haven(CYCLED, DISCARDED), clause),
        "{clause}"
    );
}

/// "~ enters with a +1/+1 counter on it for each Zombie card in your graveyard." with the given count.
fn enters_with(count: &str) -> String {
    format!(
        r#"{{"Replacement":{{"when":{{"Enters":{{"what":["This"]}}}},"by":{{"WithCounters":{{"counter":"+1/+1","count":{count}}}}}}}}}"#
    )
}

const ZOMBIES_IN_YOUR_GRAVEYARD: &str =
    r#"{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Subtype":"Zombie"}]}"#;

#[rstest]
#[case::for_each_card_of_a_subtype(
    ZOMBIES_IN_YOUR_GRAVEYARD,
    "This creature enters with a +1/+1 counter on it for each Zombie card in your graveyard."
)]
fn an_object_enters_with_a_counter_for_each_thing_counted(
    #[case] count: &str,
    #[case] clause: &str,
) {
    assert!(written(&enters_with(count), clause), "{clause}");
}

#[rstest]
#[case::another_subtype(
    "This creature enters with a +1/+1 counter on it for each Vampire card in your graveyard."
)]
#[case::another_zone(
    "This creature enters with a +1/+1 counter on it for each Zombie card in an opponent's graveyard."
)]
#[case::a_fixed_number(
    "This creature enters with two +1/+1 counters on it for each Zombie card in your graveyard."
)]
#[case::no_count("This creature enters with a +1/+1 counter on it.")]
fn another_count_does_not_read_as_the_counters_an_object_enters_with(#[case] clause: &str) {
    assert!(
        !written(&enters_with(ZOMBIES_IN_YOUR_GRAVEYARD), clause),
        "{clause}"
    );
}

/// A spell that exiles the top cards of the library and lets its controller play them for the given duration.
fn exile_and_play(count: u32, duration: &str, extra: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"ExileTop":{{"count":{count},"as":"impulse"}}}},{{"Rule":{{"rule":{{"MayCast":{{"from":"Exile","only":{{"Bound":"impulse"}},"play":true}}}},"duration":"{duration}"}}}}{extra}]}}}}"#
    )
}

const ADDITIONAL_LAND: &str = r#",{"Rule":{"rule":{"AdditionalLandPlays":{"who":"You","count":1}},"duration":"UntilEndOfTurn"}}"#;

#[rstest]
#[case::cards_exiled_this_way(
    exile_and_play(5, "UntilEndOfYourNextTurn", ""),
    "Exile the top five cards of your library. You may play cards exiled this way until the end of your next turn."
)]
#[case::and_an_additional_land(
    exile_and_play(5, "UntilEndOfYourNextTurn", ADDITIONAL_LAND),
    "Exile the top five cards of your library. You may play cards exiled this way until the end of your next turn. You may play an additional land this turn."
)]
fn exiled_cards_are_played_in_the_words_of_the_oracle(#[case] json: String, #[case] clause: &str) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::another_number(
    "Exile the top four cards of your library. You may play cards exiled this way until the end of your next turn."
)]
#[case::another_duration(
    "Exile the top five cards of your library. You may play cards exiled this way until end of turn."
)]
#[case::cast_only(
    "Exile the top five cards of your library. You may cast cards exiled this way until the end of your next turn."
)]
#[case::a_second_land(
    "Exile the top five cards of your library. You may play cards exiled this way until the end of your next turn. You may play two additional lands this turn."
)]
fn another_number_duration_or_permission_does_not_read_as_cards_exiled_and_played(
    #[case] clause: &str,
) {
    assert!(
        !written(
            &exile_and_play(5, "UntilEndOfYourNextTurn", ADDITIONAL_LAND),
            clause
        ),
        "{clause}"
    );
}

/// The top three cards looked at and split: one to the hand, one to the bottom, one exiled and playable this turn.
const SPLIT_THREE_WAYS: &str = r#"{"Spell":{"effects":[{"LookAtTop":{"count":3,"as":"top"}},{"Choose":{"from":{"Bound":"top"},"as":"kept","rest_as":"others"}},{"Move":{"what":{"Bound":"kept"},"to":"Hand"}},{"Choose":{"from":{"Bound":"others"},"as":"bottomed","rest_as":"exiled"}},{"Move":{"what":{"Bound":"bottomed"},"to":{"Library":{"place":"Bottom"}}}},{"Exile":{"what":{"Bound":"exiled"},"as":"exiled_card"}},{"Rule":{"rule":{"MayCast":{"from":"Exile","only":{"Bound":"exiled_card"},"play":true}},"duration":"UntilEndOfTurn"}}]}}"#;

#[test]
fn the_top_cards_are_split_three_ways() {
    assert!(written(
        SPLIT_THREE_WAYS,
        "Look at the top three cards of your library. Put one of them into your hand, put one of them on the bottom of your library, and exile one of them. You may play the exiled card this turn."
    ));
}

#[rstest]
#[case::two_cards(
    "Look at the top two cards of your library. Put one of them into your hand, put one of them on the bottom of your library, and exile one of them. You may play the exiled card this turn."
)]
#[case::on_top(
    "Look at the top three cards of your library. Put one of them into your hand, put one of them on top of your library, and exile one of them. You may play the exiled card this turn."
)]
#[case::to_the_graveyard(
    "Look at the top three cards of your library. Put one of them into your hand, put one of them into your graveyard, and exile one of them. You may play the exiled card this turn."
)]
#[case::until_your_next_turn(
    "Look at the top three cards of your library. Put one of them into your hand, put one of them on the bottom of your library, and exile one of them. You may play the exiled card until the end of your next turn."
)]
fn another_split_does_not_read_as_the_three_ways(#[case] clause: &str) {
    assert!(!written(SPLIT_THREE_WAYS, clause), "{clause}");
}
