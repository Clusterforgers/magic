//! Goal: the writers of the proposal checker give the oracle wording of loyalty abilities, of an exile whose cards a later step counts ("for each creature card exiled this way"), of a card in the graveyard that pays to return, of a move a land type changes and of the groups an earlier step named ("each of those creatures", "each of them", "for each opponent you have"), and a wrong cost, count, kind or land type never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const ENTERS: &str = r#"{"ZoneChange":{"what":["This"],"to":"Battlefield"}}"#;
const NON_ELEMENTALS: &str = r#"{"Each":[{"Type":"Creature"},{"Not":{"Subtype":"Elemental"}}]}"#;

fn loyalty(cost: &str, targets: &str, effects: &str) -> String {
    format!(r#"{{"Loyalty":{{"cost":{cost},"targets":{targets},"effects":{effects}}}}}"#)
}

#[rstest]
#[case::chandra_minus_three(
    loyalty(r#"{"Minus":3}"#, "[]", &format!(r#"[{{"DealDamage":{{"to":{NON_ELEMENTALS},"amount":3}}}}]"#)),
    "\u{2212}3: ~ deals 3 damage to each non-Elemental creature."
)]
#[case::plus_one(loyalty(r#"{"Plus":1}"#, "[]", r#"[{"Draw":{}}]"#), "+1: Draw a card.")]
#[case::zero(
    loyalty(r#""Zero""#, "[]", r#"[{"Draw":{"count":3}}]"#),
    "0: Draw three cards."
)]
#[case::minus_x_with_a_target(
    loyalty(
        r#""MinusX""#,
        r#"[{"what":{"Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]}}]"#,
        r#"[{"DealDamage":{"to":{"Target":0},"amount":"X"}}]"#
    ),
    "\u{2212}X: ~ deals X damage to target creature or planeswalker."
)]
#[case::two_sentences(
    loyalty(r#"{"Plus":2}"#, "[]", r#"[{"Draw":{}},{"GainLife":{"amount":2}}]"#),
    "+2: Draw a card and you gain 2 life."
)]
fn a_loyalty_ability_is_written_as_the_oracle_words_it(#[case] json: String, #[case] clause: &str) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::another_cost("\u{2212}2: ~ deals 3 damage to each non-Elemental creature.")]
#[case::a_plus("+3: ~ deals 3 damage to each non-Elemental creature.")]
#[case::no_cost("~ deals 3 damage to each non-Elemental creature.")]
#[case::another_amount("\u{2212}3: ~ deals 4 damage to each non-Elemental creature.")]
#[case::another_subtype("\u{2212}3: ~ deals 3 damage to each non-Goblin creature.")]
#[case::minus_x_for_three("\u{2212}X: ~ deals 3 damage to each non-Elemental creature.")]
fn a_loyalty_ability_does_not_read_as_another_cost_or_effect(#[case] clause: &str) {
    let json = loyalty(
        r#"{"Minus":3}"#,
        "[]",
        &format!(r#"[{{"DealDamage":{{"to":{NON_ELEMENTALS},"amount":3}}}}]"#),
    );
    assert!(!written(&json, clause), "{clause}");
}

const PLAYER_GRAVEYARD: &str = r#"{"Each":[{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]}"#;

fn exiled_count(kinds: &str) -> String {
    format!(r#"{{"Count":[{{"In":"Exile"}},{{"Is":{{"Bound":"exiled"}}}}{kinds}]}}"#)
}

fn exile_then(effect: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{ENTERS},"targets":[{{"what":"Player"}}],"effects":[{{"Exile":{{"what":{PLAYER_GRAVEYARD},"as":"exiled"}}}},{effect}]}}}}"#
    )
}

fn counters_for(kinds: &str) -> String {
    exile_then(&format!(
        r#"{{"PutCounters":{{"on":"This","counter":"+1/+1","count":{}}}}}"#,
        exiled_count(kinds)
    ))
}

fn tokens_for(kinds: &str) -> String {
    exile_then(&format!(
        r#"{{"CreateToken":{{"token":{{"Custom":{{"name":"Insect","types":["Artifact","Creature"],"subtypes":["Insect"],"power":1,"toughness":1}}}},"count":{}}}}}"#,
        exiled_count(kinds)
    ))
}

#[rstest]
#[case::malanthrope(
    counters_for(r#",{"Type":"Creature"}"#),
    "When this creature enters, exile target player's graveyard. Put a +1/+1 counter on this creature for each creature card exiled this way."
)]
#[case::canoptek_scarab_swarm(
    tokens_for(r#",{"AnyOf":[{"Type":"Artifact"},{"Type":"Land"}]}"#),
    "When this creature enters, exile target player's graveyard. For each artifact or land card exiled this way, create a 1/1 colorless Insect artifact creature token."
)]
#[case::any_card(
    counters_for(""),
    "When this creature enters, exile target player's graveyard. Put a +1/+1 counter on this creature for each card exiled this way."
)]
#[case::tokens_after_the_count(
    tokens_for(r#",{"Type":"Land"}"#),
    "When this creature enters, exile target player's graveyard. Create a 1/1 colorless Insect artifact creature token for each land card exiled this way."
)]
fn cards_exiled_this_way_are_counted_as_the_oracle_words_it(
    #[case] json: String,
    #[case] clause: &str,
) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::another_kind(
    "When this creature enters, exile target player's graveyard. Put a +1/+1 counter on this creature for each land card exiled this way."
)]
#[case::the_graveyard_counted_again(
    "When this creature enters, exile target player's graveyard. Put a +1/+1 counter on this creature for each creature card in your graveyard."
)]
#[case::once_for_the_exile(
    "When this creature enters, exile target player's graveyard. Put a +1/+1 counter on this creature."
)]
#[case::another_counter(
    "When this creature enters, exile target player's graveyard. Put a +2/+2 counter on this creature for each creature card exiled this way."
)]
fn cards_exiled_this_way_do_not_read_as_another_kind_or_count(#[case] clause: &str) {
    assert!(
        !written(&counters_for(r#",{"Type":"Creature"}"#), clause),
        "{clause}"
    );
}

const PAY_TO_RETURN: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"from":"Graveyard","effects":[{"Pay":{"cost":[{"PayLife":1}],"paid":[{"Move":{"what":"This","to":"Hand"}}]}}]}}"#;

#[rstest]
#[case::master_of_death(
    "At the beginning of your upkeep, if this card is in your graveyard, you may pay 1 life. If you do, return it to your hand.",
    true
)]
#[case::to_its_owners_hand(
    "At the beginning of your upkeep, if this card is in your graveyard, you may pay 1 life. If you do, return ~ to its owner's hand.",
    true
)]
#[case::without_the_graveyard(
    "At the beginning of your upkeep, you may pay 1 life. If you do, return it to your hand.",
    false
)]
#[case::another_payment(
    "At the beginning of your upkeep, if this card is in your graveyard, you may pay 2 life. If you do, return it to your hand.",
    false
)]
#[case::another_step(
    "At the beginning of your end step, if this card is in your graveyard, you may pay 1 life. If you do, return it to your hand.",
    false
)]
#[case::the_battlefield_instead(
    "At the beginning of your upkeep, if this card is in your graveyard, you may pay 1 life. If you do, return it to the battlefield.",
    false
)]
fn a_card_in_the_graveyard_that_pays_to_return(#[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(PAY_TO_RETURN, clause), reads, "{clause}");
}

fn landfall(then: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":[{{"Type":"Land"}},{{"ControlledBy":"You"}}],"to":"Battlefield"}}}},"targets":[{{"what":{{"Object":[{{"In":"Graveyard"}},{{"OwnedBy":"You"}},{{"AnyOf":[{{"Type":"Artifact"}},{{"Type":"Battle"}},{{"Type":"Creature"}},{{"Type":"Enchantment"}},{{"Type":"Planeswalker"}}]}}]}}}}],"effects":[{{"If":{{"condition":{{"Matches":{{"subject":"Triggering","selector":[{{"Subtype":"{then}"}}]}}}},"then":[{{"May":{{"effects":[{{"Move":{{"what":{{"Target":0}},"to":{{"Battlefield":{{}}}}}}}}]}}}}],"otherwise":[{{"May":{{"effects":[{{"Move":{{"what":{{"Target":0}},"to":"Hand"}}}}]}}}}]}}}}]}}}}"#
    )
}

#[rstest]
#[case::emeria_shepherd(
    "Whenever a land you control enters, you may return target nonland permanent card from your graveyard to your hand. If that land is a Plains, you may return that nonland permanent card to the battlefield instead.",
    "Plains",
    true
)]
#[case::a_forest(
    "Whenever a land you control enters, you may return target nonland permanent card from your graveyard to your hand. If that land is a Forest, you may return that nonland permanent card to the battlefield instead.",
    "Forest",
    true
)]
#[case::another_land_type(
    "Whenever a land you control enters, you may return target nonland permanent card from your graveyard to your hand. If that land is a Forest, you may return that nonland permanent card to the battlefield instead.",
    "Plains",
    false
)]
#[case::no_instead(
    "Whenever a land you control enters, you may return target nonland permanent card from your graveyard to your hand. If that land is a Plains, you may return that nonland permanent card to the battlefield.",
    "Plains",
    false
)]
#[case::another_card(
    "Whenever a land you control enters, you may return target creature card from your graveyard to your hand. If that land is a Plains, you may return that creature card to the battlefield instead.",
    "Plains",
    false
)]
fn a_move_a_land_type_changes(#[case] clause: &str, #[case] land: &str, #[case] reads: bool) {
    assert_eq!(written(&landfall(land), clause), reads, "{clause}");
}

const OPPONENTS: &str = r#"{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}"#;

#[rstest]
#[case::each_of_those_creatures(
    format!(r#"{{"Triggered":{{"when":{ENTERS},"effects":[{{"Pay":{{"cost":"{{5}}","paid":[{{"Tap":{{"what":{OPPONENTS}}}}},{{"PutCounters":{{"on":{OPPONENTS},"counter":{{"Other":"stun"}}}}}}]}}}}]}}}}"#),
    "When ~ enters, you may pay {5}. If you do, tap all creatures your opponents control, then put a stun counter on each of those creatures.",
    true
)]
#[case::the_group_named_again(
    format!(r#"{{"Triggered":{{"when":{ENTERS},"effects":[{{"Pay":{{"cost":"{{5}}","paid":[{{"Tap":{{"what":{OPPONENTS}}}}},{{"PutCounters":{{"on":{OPPONENTS},"counter":{{"Other":"stun"}}}}}}]}}}}]}}}}"#),
    "When ~ enters, you may pay {5}. If you do, tap all creatures your opponents control, then put a stun counter on each creature your opponents control.",
    true
)]
#[case::another_group_for_those(
    format!(r#"{{"Triggered":{{"when":{ENTERS},"effects":[{{"Pay":{{"cost":"{{5}}","paid":[{{"Tap":{{"what":{OPPONENTS}}}}},{{"PutCounters":{{"on":{{"Each":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}},"counter":{{"Other":"stun"}}}}}}]}}}}]}}}}"#),
    "When ~ enters, you may pay {5}. If you do, tap all creatures your opponents control, then put a stun counter on each of those creatures.",
    false
)]
#[case::each_of_them(
    r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Subtype":"Insect"},{"ControlledBy":"You"}],"at_least":1}},"effects":[{"PutCounters":{"on":"Triggering","counter":"+1/+1"}}]}}"#.to_owned(),
    "Whenever you attack with one or more Insects, put a +1/+1 counter on each of them.",
    true
)]
#[case::each_of_them_for_a_single_trigger(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#.to_owned(),
    "Whenever ~ attacks, put a +1/+1 counter on each of them.",
    false
)]
#[case::for_each_opponent_you_have(
    r#"{"Activated":{"cost":[{"Mana":"{5}"},"Tap"],"effects":[{"CreateToken":{"token":{"Custom":{"name":"Rat","colors":["Black"],"types":["Creature"],"subtypes":["Rat"],"power":1,"toughness":1}},"count":{"Players":"Opponents"}}}]}}"#.to_owned(),
    "{5}, {T}: Create a 1/1 black Rat creature token for each opponent you have.",
    true
)]
#[case::for_each_opponent_alone(
    r#"{"Activated":{"cost":[{"Mana":"{5}"},"Tap"],"effects":[{"CreateToken":{"token":{"Custom":{"name":"Rat","colors":["Black"],"types":["Creature"],"subtypes":["Rat"],"power":1,"toughness":1}},"count":{"Players":"Opponents"}}}]}}"#.to_owned(),
    "{5}, {T}: Create a 1/1 black Rat creature token for each player you have.",
    false
)]
#[case::counter_and_life(
    r#"{"Triggered":{"when":{"Sacrificed":{"what":[],"by":"EachPlayer"}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}},{"GainLife":{"amount":1}}]}}"#.to_owned(),
    "Whenever a player sacrifices a permanent, put a +1/+1 counter on ~ and you gain 1 life.",
    true
)]
fn the_group_an_earlier_step_named(
    #[case] json: String,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(&json, clause), reads, "{clause}");
}
