//! Goal: the writers of the second sample of S to Z give the oracle wording of a sacrifice by every player, a token created in the sentence of the step before it, a bigger blow under a condition, the creature of an event, the card itself as "it" after being tapped for mana, a base size with an evasion, a tap of another creature, a search for a card of two types, a rule that lets spells have flash, spells that can't be countered, a permanent that is put into a graveyard, the caster of a spell, a second gift, and two players who draw; a wrong player, count, kind, amount, group or order never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn sacrifice(who: &str, count: u32, kind: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Sacrifice":{{"who":"{who}","filter":[{{"Type":"{kind}"}}],"count":{count}}}}}]}}}}"#
    )
}

#[rstest]
#[case::two_creatures("EachPlayer", 2, "Creature", "Each player sacrifices two creatures.")]
#[case::three_of_their_choice(
    "EachPlayer",
    3,
    "Creature",
    "Each player sacrifices three creatures of their choice."
)]
#[case::one_artifact("EachPlayer", 1, "Artifact", "Each player sacrifices an artifact.")]
#[case::each_opponent("Opponents", 2, "Creature", "Each opponent sacrifices two creatures.")]
#[case::each_other_player("Opponents", 1, "Creature", "Each other player sacrifices a creature.")]
fn a_sacrifice_by_every_player_reads_with_the_number_and_the_plural(
    #[case] who: &str,
    #[case] count: u32,
    #[case] kind: &str,
    #[case] clause: &str,
) {
    assert!(written(&sacrifice(who, count, kind), clause), "{clause}");
}

#[rstest]
#[case::another_count("EachPlayer", 2, "Creature", "Each player sacrifices three creatures.")]
#[case::another_kind("EachPlayer", 2, "Creature", "Each player sacrifices two artifacts.")]
#[case::another_player("EachPlayer", 2, "Creature", "Each opponent sacrifices two creatures.")]
#[case::you_alone("EachPlayer", 2, "Creature", "You sacrifice two creatures.")]
#[case::singular_for_two("EachPlayer", 2, "Creature", "Each player sacrifices a creature.")]
#[case::opponents_one_is_the_old_writer(
    "Opponents",
    1,
    "Creature",
    "Each player sacrifices a creature."
)]
fn a_sacrifice_by_every_player_does_not_read_as_another_number_kind_or_player(
    #[case] who: &str,
    #[case] count: u32,
    #[case] kind: &str,
    #[case] clause: &str,
) {
    assert!(!written(&sacrifice(who, count, kind), clause), "{clause}");
}

const FOOD_FEAST: &str = r#"{"Spell":{"effects":[{"Sacrifice":{"who":"EachPlayer","filter":[{"Type":"Creature"}],"count":3}},{"CreateToken":{"token":{"Named":"food"},"count":3}}]}}"#;

#[test]
fn a_token_created_after_a_sacrifice_by_every_player_is_created_by_you() {
    assert!(written(
        FOOD_FEAST,
        "Each player sacrifices three creatures of their choice. You create three Food tokens."
    ));
    assert!(!written(
        FOOD_FEAST,
        "Each player sacrifices three creatures of their choice. Each player creates three Food tokens."
    ));
}

fn grows_and_creates(first: &str, token: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{first},{{"CreateToken":{{"token":{{"Named":"{token}"}}}}}}]}}}}"#
    )
}

const COUNTER: &str = r#"{"PutCounters":{"on":"This","counter":"+1/+1"}}"#;

#[rstest]
#[case::counter_and_treasure(
    COUNTER,
    "treasure",
    "When ~ enters, put a +1/+1 counter on ~ and create a Treasure token."
)]
#[case::life_and_food(
    r#"{"GainLife":{"amount":2}}"#,
    "food",
    "When ~ enters, you gain 2 life and create a Food token."
)]
#[case::two_sentences(
    COUNTER,
    "treasure",
    "When ~ enters, put a +1/+1 counter on ~. You create a Treasure token."
)]
fn a_token_may_be_created_in_the_sentence_of_the_step_before_it(
    #[case] first: &str,
    #[case] token: &str,
    #[case] clause: &str,
) {
    assert!(
        written(&grows_and_creates(first, token), clause),
        "{clause}"
    );
}

#[rstest]
#[case::another_token(
    COUNTER,
    "treasure",
    "When ~ enters, put a +1/+1 counter on ~ and create a Food token."
)]
#[case::swapped(
    COUNTER,
    "treasure",
    "When ~ enters, create a Treasure token and put a +1/+1 counter on ~."
)]
#[case::another_step(
    r#"{"Destroy":{"what":"This"}}"#,
    "treasure",
    "When ~ enters, destroy ~ and create a Treasure token."
)]
fn a_token_joined_with_and_does_not_read_as_another_token_step_or_order(
    #[case] first: &str,
    #[case] token: &str,
    #[case] clause: &str,
) {
    assert!(
        !written(&grows_and_creates(first, token), clause),
        "{clause}"
    );
}

fn heat(small: u32, big: u32, types: u32) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"AnyOf":[{{"Type":"Creature"}},{{"Type":"Planeswalker"}}]}}]}}}}],"effects":[{{"If":{{"condition":{{"Compare":{{"left":{{"CardTypes":[{{"In":"Graveyard"}},{{"OwnedBy":"You"}}]}},"op":"Ge","right":{types}}}}},"then":[{{"DealDamage":{{"to":{{"Target":0}},"amount":{big}}}}}],"otherwise":[{{"DealDamage":{{"to":{{"Target":0}},"amount":{small}}}}}]}}}}]}}}}"#
    )
}

#[rstest]
#[case::with_the_word(
    "~ deals 2 damage to target creature or planeswalker. Delirium \u{2014} ~ deals 6 damage instead if there are four or more card types among cards in your graveyard."
)]
#[case::without_the_word(
    "~ deals 2 damage to target creature or planeswalker. ~ deals 6 damage instead if there are four or more card types among cards in your graveyard."
)]
fn a_bigger_blow_under_a_count_of_the_graveyard_reads_as_two_sentences(#[case] clause: &str) {
    assert!(written(&heat(2, 6, 4), clause), "{clause}");
}

#[rstest]
#[case::fewer_types(
    "~ deals 2 damage to target creature or planeswalker. ~ deals 6 damage instead if there are three or more card types among cards in your graveyard."
)]
#[case::another_big_amount(
    "~ deals 2 damage to target creature or planeswalker. ~ deals 5 damage instead if there are four or more card types among cards in your graveyard."
)]
#[case::swapped_amounts(
    "~ deals 6 damage to target creature or planeswalker. ~ deals 2 damage instead if there are four or more card types among cards in your graveyard."
)]
#[case::no_condition(
    "~ deals 2 damage to target creature or planeswalker. ~ deals 6 damage instead."
)]
fn a_bigger_blow_does_not_read_as_another_count_amount_or_order(#[case] clause: &str) {
    assert!(!written(&heat(2, 6, 4), clause), "{clause}");
}

fn grows(on: &str, event: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"DealsDamage":{{"source":[{event}],"to":"EachPlayer","combat":true}}}},"effects":[{{"PutCounters":{{"on":{on},"counter":"+1/+1"}}}}]}}}}"#
    )
}

const YOUR_CREATURE: &str = r#"{"Type":"Creature"},{"ControlledBy":"You"}"#;

#[test]
fn the_creature_of_an_event_that_gets_a_counter_is_that_creature() {
    let clause = "Whenever a creature you control deals combat damage to a player, put a +1/+1 counter on that creature.";
    assert!(written(&grows(r#""Triggering""#, YOUR_CREATURE), clause));
    assert!(!written(&grows(r#""This""#, YOUR_CREATURE), clause));
    assert!(!written(
        &grows(r#""Triggering""#, YOUR_CREATURE),
        "Whenever a creature you control deals combat damage to a player, put a +1/+1 counter on target creature."
    ));
}

fn mana_trigger(what: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"TappedForMana":{{"what":[{what}],"by":"You"}}}},"effects":[{{"DealDamage":{{"source":"This","to":"Opponents","amount":1}}}}]}}}}"#
    )
}

#[test]
fn the_card_tapped_for_mana_calls_itself_it_but_a_land_does_not() {
    assert!(written(
        &mana_trigger(r#""This""#),
        "Whenever you tap this creature for mana, it deals 1 damage to each opponent."
    ));
    assert!(!written(
        &mana_trigger(r#"{"Type":"Land"}"#),
        "Whenever you tap a land for mana, it deals 1 damage to each opponent."
    ));
}

fn arm(blockers: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":"Attached","mods":[{{"SetPT":{{"power":7,"toughness":7}}}},{{"CantBeBlockedBy":[{blockers}]}}]}}}}"#
    )
}

#[rstest]
#[case::power_two_or_less(
    r#"{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":2}}"#,
    "Equipped creature has base power and toughness 7/7 and can't be blocked by creatures with power 2 or less."
)]
fn a_base_size_and_an_evasion_are_one_sentence(#[case] blockers: &str, #[case] clause: &str) {
    assert!(written(&arm(blockers), clause), "{clause}");
}

#[rstest]
#[case::power_three(
    r#"{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":2}}"#,
    "Equipped creature has base power and toughness 7/7 and can't be blocked by creatures with power 3 or less."
)]
#[case::greater(
    r#"{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":2}}"#,
    "Equipped creature has base power and toughness 7/7 and can't be blocked by creatures with power 2 or greater."
)]
#[case::another_size(
    r#"{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":2}}"#,
    "Equipped creature has base power and toughness 6/6 and can't be blocked by creatures with power 2 or less."
)]
#[case::any_permanent(
    r#"{"Compare":{"stat":"Power","op":"Le","to":2}}"#,
    "Equipped creature has base power and toughness 7/7 and can't be blocked by creatures with power 2 or less."
)]
fn a_base_size_and_an_evasion_do_not_read_as_another_group_or_size(
    #[case] blockers: &str,
    #[case] clause: &str,
) {
    assert!(!written(&arm(blockers), clause), "{clause}");
}

fn tap_another(filter: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":[{{"TapPermanents":{{"filter":[{filter}]}}}}],"effects":[{{"Continuous":{{"affected":"This","mods":["CantBeBlocked"],"duration":"UntilEndOfTurn"}}}}]}}}}"#
    )
}

#[test]
fn a_cost_taps_another_untapped_creature_of_a_kind() {
    let clause =
        "Tap another untapped Rogue you control: This creature can't be blocked this turn.";
    assert!(written(
        &tap_another(r#"{"Subtype":"Rogue"},"Other""#),
        clause
    ));
    assert!(!written(&tap_another(r#"{"Subtype":"Rogue"}"#), clause));
    assert!(!written(
        &tap_another(r#"{"Subtype":"Goblin"},"Other""#),
        clause
    ));
}

fn summoner(types: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{{"May":{{"effects":[{{"Search":{{"filter":[{types}],"reveal":true,"as":"card"}}}},{{"Move":{{"what":{{"Bound":"card"}},"to":"Hand"}}}},{{"Shuffle":{{}}}}]}}}}]}}}}"#
    )
}

#[test]
fn a_search_names_a_card_of_two_types_as_one_phrase() {
    let clause = "When this creature enters, you may search your library for an artifact creature card, reveal it, put it into your hand, then shuffle.";
    assert!(written(
        &summoner(r#"{"Type":"Artifact"},{"Type":"Creature"}"#),
        clause
    ));
    assert!(!written(&summoner(r#"{"Type":"Artifact"}"#), clause));
    assert!(!written(
        &summoner(r#"{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]}"#),
        clause
    ));
}

fn flash(what: &str, flash: bool) -> String {
    let flash = if flash { r#","flash":true"# } else { "" };
    format!(r#"{{"Rule":{{"rule":{{"MayCast":{{"what":[{what}]{flash}}}}}}}}}"#)
}

#[rstest]
#[case::artifacts(
    r#"{"Type":"Artifact"}"#,
    "You may cast artifact spells as though they had flash."
)]
#[case::all_spells("", "You may cast spells as though they had flash.")]
#[case::noncreature(
    r#"{"Not":{"Type":"Creature"}}"#,
    "You may cast noncreature spells as though they had flash."
)]
fn the_permission_to_cast_with_flash_names_its_kind(#[case] what: &str, #[case] clause: &str) {
    assert!(written(&flash(what, true), clause), "{clause}");
}

#[rstest]
#[case::another_kind(
    r#"{"Type":"Artifact"}"#,
    true,
    "You may cast creature spells as though they had flash."
)]
#[case::all_for_a_kind(
    r#"{"Type":"Artifact"}"#,
    true,
    "You may cast spells as though they had flash."
)]
#[case::no_flash(
    r#"{"Type":"Artifact"}"#,
    false,
    "You may cast artifact spells as though they had flash."
)]
fn the_permission_to_cast_with_flash_does_not_read_as_another_kind(
    #[case] what: &str,
    #[case] with_flash: bool,
    #[case] clause: &str,
) {
    assert!(!written(&flash(what, with_flash), clause), "{clause}");
}

fn uncounterable(group: &str) -> String {
    format!(r#"{{"Rule":{{"rule":{{"CantBeCountered":{{"Each":[{group}]}}}}}}}}"#)
}

#[test]
fn spells_of_a_kind_you_control_can_not_be_countered() {
    let creatures = uncounterable(r#""Spell",{"Type":"Creature"},{"ControlledBy":"You"}"#);
    assert!(written(
        &creatures,
        "Creature spells you control can't be countered."
    ));
    assert!(!written(
        &creatures,
        "Artifact spells you control can't be countered."
    ));
    assert!(!written(&creatures, "Creature spells can't be countered."));
    assert!(!written(
        &creatures,
        "Spells you control can't be countered."
    ));
}

fn leaves(what: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":[{what}],"from":"Battlefield","to":"Graveyard"}}}},"effects":[{{"PutCounters":{{"on":"This","counter":"+1/+1"}}}}]}}}}"#
    )
}

#[test]
fn a_permanent_that_is_not_a_creature_is_put_into_a_graveyard_not_dying() {
    let clause = "Whenever an enchantment you control is put into a graveyard from the battlefield, put a +1/+1 counter on ~.";
    assert!(written(
        &leaves(r#"{"Type":"Enchantment"},{"ControlledBy":"You"}"#),
        clause
    ));
    assert!(!written(
        &leaves(r#"{"Type":"Creature"},{"ControlledBy":"You"}"#),
        "Whenever a creature you control is put into a graveyard from the battlefield, put a +1/+1 counter on ~."
    ));
    assert!(!written(
        &leaves(r#"{"Type":"Artifact"},{"ControlledBy":"You"}"#),
        clause
    ));
}

fn casts(nth: u32, by: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"Nth":{{"event":{{"SpellCast":{{"what":[{{"Not":{{"Type":"Creature"}}}}],"by":"{by}"}}}},"nth":{nth}}}}},"effects":[{{"LoseLife":{{"who":{{"ControllerOf":"Triggering"}},"amount":{{"Stat":{{"of":"Triggering","stat":"ManaValue"}}}}}}}}]}}}}"#
    )
}

#[rstest]
#[case::first_by_an_opponent(
    1,
    "Opponents",
    "Whenever an opponent casts their first noncreature spell each turn, that player loses life equal to that spell's mana value."
)]
#[case::second_by_an_opponent(
    2,
    "Opponents",
    "Whenever an opponent casts their second noncreature spell each turn, that player loses life equal to that spell's mana value."
)]
fn the_caster_and_the_spell_of_a_cast_event_are_that_player_and_that_spell(
    #[case] nth: u32,
    #[case] by: &str,
    #[case] clause: &str,
) {
    assert!(written(&casts(nth, by), clause), "{clause}");
}

#[rstest]
#[case::another_ordinal(
    1,
    "Opponents",
    "Whenever an opponent casts their second noncreature spell each turn, that player loses life equal to that spell's mana value."
)]
#[case::your_spell(
    1,
    "Opponents",
    "Whenever you cast your first noncreature spell each turn, that player loses life equal to that spell's mana value."
)]
#[case::another_stat(
    1,
    "Opponents",
    "Whenever an opponent casts their first noncreature spell each turn, that player loses life equal to that spell's power."
)]
fn the_caster_and_the_spell_of_a_cast_event_do_not_read_as_another_event_or_stat(
    #[case] nth: u32,
    #[case] by: &str,
    #[case] clause: &str,
) {
    assert!(!written(&casts(nth, by), clause), "{clause}");
}

fn rage(trample: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"AddKeyword":"DoubleStrike"}}],"duration":"UntilEndOfTurn"}}}},{{"If":{{"condition":{{"Compare":{{"left":{{"Count":[{{"Type":"Creature"}},{{"ControlledBy":"You"}},{{"Compare":{{"stat":"Power","op":"Ge","to":4}}}}]}},"op":"Ge","right":1}}}},"then":[{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"AddKeyword":"{trample}"}}],"duration":"UntilEndOfTurn"}}}}]}}}}]}}}}"#
    )
}

#[rstest]
#[case::with_the_ability_word(
    "Target creature gains double strike until end of turn. Ferocious \u{2014} That creature also gains trample until end of turn if you control a creature with power 4 or greater."
)]
#[case::without_it(
    "Target creature gains double strike until end of turn. That creature also gains trample until end of turn if you control a creature with power 4 or greater."
)]
fn a_second_gift_for_a_big_creature_reads_as_also_gains(#[case] clause: &str) {
    assert!(written(&rage("Trample"), clause), "{clause}");
}

#[rstest]
#[case::another_keyword(
    "Target creature gains double strike until end of turn. That creature also gains haste until end of turn if you control a creature with power 4 or greater."
)]
#[case::another_power(
    "Target creature gains double strike until end of turn. That creature also gains trample until end of turn if you control a creature with power 5 or greater."
)]
fn a_second_gift_does_not_read_as_another_keyword_or_power(#[case] clause: &str) {
    assert!(!written(&rage("Trample"), clause), "{clause}");
}

fn rendezvous(who: &str, count: u32) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":"{who}"}}],"effects":[{{"Draw":{{"count":{count}}}}},{{"Draw":{{"who":{{"Target":0}},"count":{count}}}}}]}}}}"#
    )
}

#[test]
fn you_and_a_target_player_each_draw_the_same_cards() {
    assert!(written(
        &rendezvous("Opponent", 3),
        "You and target opponent each draw three cards."
    ));
    assert!(written(
        &rendezvous("Player", 2),
        "You and target player each draw two cards."
    ));
    assert!(!written(
        &rendezvous("Opponent", 3),
        "You and target player each draw three cards."
    ));
    assert!(!written(
        &rendezvous("Opponent", 3),
        "You and target opponent each draw two cards."
    ));
}

fn immortal(who: &str) -> String {
    format!(
        r#"{{"Replacement":{{"when":{{"Enters":{{"what":["This"]}}}},"by":{{"WithCounters":{{"counter":"+1/+1","count":{{"Tallied":{{"who":"{who}","what":"CreaturesDied"}}}}}}}}}}}}"#
    )
}

#[test]
fn a_permanent_enters_with_a_counter_for_each_creature_that_died() {
    let clause = "~ enters with a +1/+1 counter on it for each creature that died this turn.";
    assert!(written(&immortal("EachPlayer"), clause));
    assert!(!written(&immortal("You"), clause));
}
