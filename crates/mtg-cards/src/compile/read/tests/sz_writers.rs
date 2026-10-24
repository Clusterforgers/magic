//! Goal: the writers of the sample of S to Z give the oracle wording of a draw by the controller of a creature, a cast from one's own graveyard, a search with the shuffle as its own sentence, a group of two creature types, a doubled power, damage "have this deal", a land type given alone, a destruction of one creature type, a bounce for each object, a goad as a sentence, damage by a creature put onto the battlefield, an amount less a number, a free cast for the turn, "non-outlaw" and the facts about life lost, lands and an attack, and a wrong player, count, type, zone or duration never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const SLIVER: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"Subtype":"Sliver"}],"to":"EachPlayer","combat":true}},"effects":[{"May":{"who":{"ControllerOf":"Triggering"},"effects":[{"Draw":{"who":{"ControllerOf":"Triggering"}}}]}}]}}"#;

#[rstest]
#[case::its_controller(
    SLIVER,
    "Whenever a Sliver deals combat damage to a player, its controller may draw a card."
)]
fn the_controller_of_the_damage_source_may_draw(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::you("Whenever a Sliver deals combat damage to a player, you may draw a card.")]
#[case::two_cards(
    "Whenever a Sliver deals combat damage to a player, its controller may draw two cards."
)]
#[case::an_opponent(
    "Whenever a Sliver deals combat damage to an opponent, its controller may draw a card."
)]
#[case::not_optional(
    "Whenever a Sliver deals combat damage to a player, its controller draws a card."
)]
#[case::another_type(
    "Whenever a Goblin deals combat damage to a player, its controller may draw a card."
)]
fn a_draw_by_the_controller_does_not_read_as_another_draw(#[case] clause: &str) {
    assert!(!written(SLIVER, clause), "{clause}");
}

const SECRETS: &str = r#"{"Triggered":{"when":{"SpellCast":{"what":[{"CastFrom":"Graveyard"}],"by":"You"}},"effects":[{"Draw":{}}]}}"#;
const THEIRS: &str = r#"{"Triggered":{"when":{"SpellCast":{"what":[{"CastFrom":"Graveyard"}],"by":"Opponents"}},"effects":[{"Draw":{}}]}}"#;

#[rstest]
#[case::your_graveyard(SECRETS, "Whenever you cast a spell from your graveyard, draw a card.")]
#[case::any_graveyard(SECRETS, "Whenever you cast a spell from a graveyard, draw a card.")]
fn a_cast_from_a_graveyard_names_the_graveyard_of_the_caster(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::an_opponent_graveyard(
    SECRETS,
    "Whenever you cast a spell from an opponent's graveyard, draw a card."
)]
#[case::an_opponent_casts_from_yours(
    THEIRS,
    "Whenever an opponent casts a spell from your graveyard, draw a card."
)]
#[case::another_card(
    SECRETS,
    "Whenever you cast a spell from your graveyard, draw two cards."
)]
fn a_cast_from_your_graveyard_does_not_read_as_another_cast(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(!written(json, clause), "{clause}");
}

fn spoils(tapped: bool) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Search":{{"filter":[{{"Subtype":"Forest"}}],"as":"land"}}}},{{"Move":{{"what":{{"Bound":"land"}},"to":{{"Battlefield":{{"tapped":{tapped}}}}}}}}},{{"Shuffle":{{}}}}]}}}}"#
    )
}

#[rstest]
#[case::untapped(
    false,
    "Search your library for a Forest card and put that card onto the battlefield. Then shuffle."
)]
#[case::tapped(
    true,
    "Search your library for a Forest card and put that card onto the battlefield tapped. Then shuffle."
)]
fn a_search_may_put_the_card_onto_the_battlefield_before_a_sentence_of_shuffling(
    #[case] tapped: bool,
    #[case] clause: &str,
) {
    assert!(written(&spoils(tapped), clause), "{clause}");
}

#[rstest]
#[case::tapped_for_an_untapped_card(
    false,
    "Search your library for a Forest card and put that card onto the battlefield tapped. Then shuffle."
)]
#[case::untapped_for_a_tapped_card(
    true,
    "Search your library for a Forest card and put that card onto the battlefield. Then shuffle."
)]
#[case::the_hand(
    false,
    "Search your library for a Forest card and put that card into your hand. Then shuffle."
)]
#[case::no_shuffle(
    false,
    "Search your library for a Forest card and put that card onto the battlefield."
)]
#[case::another_type(
    false,
    "Search your library for a Swamp card and put that card onto the battlefield. Then shuffle."
)]
fn a_search_with_a_sentence_of_shuffling_does_not_read_as_another_search(
    #[case] tapped: bool,
    #[case] clause: &str,
) {
    assert!(!written(&spoils(tapped), clause), "{clause}");
}

fn wave_knight(subtypes: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"Drew":{{"who":"You"}}}},"effects":[{{"PutCounters":{{"on":{{"Each":[{{"AnyOf":[{subtypes}]}},{{"ControlledBy":"You"}},"Other"]}},"counter":"+1/+1"}}}}]}}}}"#
    )
}

const MERFOLK_KNIGHT: &str = r#"{"Subtype":"Merfolk"},{"Subtype":"Knight"}"#;
const MERFOLK_ARTIFACT: &str = r#"{"Subtype":"Merfolk"},{"Type":"Artifact"}"#;

#[rstest]
#[case::and_or(
    MERFOLK_KNIGHT,
    "Whenever you draw a card, put a +1/+1 counter on each other Merfolk and/or Knight you control."
)]
#[case::or(
    MERFOLK_KNIGHT,
    "Whenever you draw a card, put a +1/+1 counter on each other Merfolk or Knight you control."
)]
fn two_creature_types_of_a_group_are_joined_with_and_or(
    #[case] subtypes: &str,
    #[case] clause: &str,
) {
    assert!(written(&wave_knight(subtypes), clause), "{clause}");
}

#[rstest]
#[case::and(
    MERFOLK_KNIGHT,
    "Whenever you draw a card, put a +1/+1 counter on each other Merfolk and Knight you control."
)]
#[case::without_other(
    MERFOLK_KNIGHT,
    "Whenever you draw a card, put a +1/+1 counter on each Merfolk and/or Knight you control."
)]
#[case::another_type(
    MERFOLK_KNIGHT,
    "Whenever you draw a card, put a +1/+1 counter on each other Merfolk and/or Elf you control."
)]
#[case::a_card_type(
    MERFOLK_ARTIFACT,
    "Whenever you draw a card, put a +1/+1 counter on each other Merfolk and/or artifact you control."
)]
fn a_group_with_and_or_does_not_read_as_another_group(
    #[case] subtypes: &str,
    #[case] clause: &str,
) {
    assert!(!written(&wave_knight(subtypes), clause), "{clause}");
}

fn double_power(stat: &str, duration: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"ModifyPT":{{"power":{{"Stat":{{"of":{{"Target":0}},"stat":"{stat}"}}}},"toughness":0}}}}],"duration":"{duration}"}}}}]}}}}"#
    )
}

#[test]
fn the_power_of_a_target_may_be_doubled() {
    let clause = "Double the power of target creature until end of turn.";
    assert!(written(&double_power("Power", "UntilEndOfTurn"), clause));
}

#[rstest]
#[case::toughness(
    "Toughness",
    "UntilEndOfTurn",
    "Double the power of target creature until end of turn."
)]
#[case::until_your_next_turn(
    "Power",
    "UntilYourNextTurn",
    "Double the power of target creature until end of turn."
)]
#[case::both_stats(
    "Power",
    "UntilEndOfTurn",
    "Double the power and toughness of target creature until end of turn."
)]
#[case::no_duration("Power", "UntilEndOfTurn", "Double the power of target creature.")]
#[case::another_object(
    "Power",
    "UntilEndOfTurn",
    "Double the power of target land until end of turn."
)]
fn a_doubled_power_does_not_read_as_another_change(
    #[case] stat: &str,
    #[case] duration: &str,
    #[case] clause: &str,
) {
    assert!(!written(&double_power(stat, duration), clause), "{clause}");
}

fn tread(amount: u32) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}],"to":"Battlefield"}}}},"targets":[{{"what":"AnyTarget"}}],"effects":[{{"May":{{"effects":[{{"DealDamage":{{"to":{{"Target":0}},"amount":{amount}}}}}]}}}}]}}}}"#
    )
}

#[test]
fn the_source_may_be_had_to_deal_damage() {
    let clause = "Whenever a creature you control enters, you may have this enchantment deal 5 damage to any target.";
    assert!(written(&tread(5), clause));
}

#[rstest]
#[case::another_number(
    "Whenever a creature you control enters, you may have this enchantment deal 4 damage to any target."
)]
#[case::another_recipient(
    "Whenever a creature you control enters, you may have this enchantment deal 5 damage to target creature."
)]
#[case::not_optional(
    "Whenever a creature you control enters, have this enchantment deal 5 damage to any target."
)]
fn damage_the_source_may_deal_does_not_read_as_other_damage(#[case] clause: &str) {
    assert!(!written(&tread(5), clause), "{clause}");
}

fn each_land_is(subtype: &str, others: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":{{"Each":[{others}]}},"mods":[{{"AddSubtype":"{subtype}"}}]}}}}"#
    )
}

#[rstest]
#[case::forest("Forest", "a Forest")]
#[case::island("Island", "an Island")]
fn a_basic_land_type_given_alone_is_added_to_the_land_types(
    #[case] subtype: &str,
    #[case] noun: &str,
) {
    let json = each_land_is(subtype, r#"{"Type":"Land"}"#);
    let clause = format!("Each land is {noun} in addition to its other land types.");
    assert!(written(&json, &clause), "{clause}");
}

#[rstest]
#[case::another_type(
    "Forest",
    r#"{"Type":"Land"}"#,
    "Each land is a Swamp in addition to its other land types."
)]
#[case::a_creature_type(
    "Elf",
    r#"{"Type":"Land"}"#,
    "Each land is an Elf in addition to its other land types."
)]
#[case::another_group(
    "Forest",
    r#"{"Type":"Creature"}"#,
    "Each land is a Forest in addition to its other land types."
)]
fn a_land_type_does_not_read_as_another_type_or_group(
    #[case] subtype: &str,
    #[case] group: &str,
    #[case] clause: &str,
) {
    assert!(!written(&each_land_is(subtype, group), clause), "{clause}");
}

fn apocalypse(subtype: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Move":{{"what":{{"Each":[{{"Type":"Creature"}},{{"Subtype":"Zombie"}},{{"In":"Graveyard"}},{{"OwnedBy":"You"}}]}},"to":{{"Battlefield":{{"tapped":true}}}}}}}},{{"Destroy":{{"what":{{"Each":[{{"Type":"Creature"}},{{"Subtype":"{subtype}"}}]}}}}}}]}}}}"#
    )
}

#[rstest]
#[case::joined(
    "Return all Zombie creature cards from your graveyard to the battlefield tapped, then destroy all Humans."
)]
#[case::apart(
    "Return all Zombie creature cards from your graveyard to the battlefield tapped. Then destroy all Humans."
)]
fn all_creatures_of_one_type_are_destroyed_as_that_type(#[case] clause: &str) {
    assert!(written(&apocalypse("Human"), clause), "{clause}");
}

#[rstest]
#[case::another_type(
    "Return all Zombie creature cards from your graveyard to the battlefield tapped, then destroy all Elves."
)]
#[case::every_creature(
    "Return all Zombie creature cards from your graveyard to the battlefield tapped, then destroy all creatures."
)]
#[case::untapped(
    "Return all Zombie creature cards from your graveyard to the battlefield, then destroy all Humans."
)]
fn a_destruction_of_one_type_does_not_read_as_another_destruction(#[case] clause: &str) {
    assert!(!written(&apocalypse("Human"), clause), "{clause}");
}

fn scourge(group: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{{"Move":{{"what":{{"Each":[{{"Type":"Creature"}},{group},{{"Compare":{{"stat":"Toughness","op":"Le","to":{{"Count":[{{"Subtype":"Island"}},{{"ControlledBy":"You"}}]}}}}}}]}},"to":"Hand"}}}}]}}}}"#
    )
}

#[rstest]
#[case::each(
    r#"{"ControlledBy":"Opponents"}"#,
    "When this creature enters, return each creature your opponents control with toughness X or less to its owner's hand, where X is the number of Islands you control."
)]
#[case::all(
    r#"{"ControlledBy":"Opponents"}"#,
    "When this creature enters, return all creatures your opponents control with toughness X or less to their owners' hands, where X is the number of Islands you control."
)]
fn a_group_returned_to_hand_is_each_object_to_its_owner(#[case] group: &str, #[case] clause: &str) {
    assert!(written(&scourge(group), clause), "{clause}");
}

#[rstest]
#[case::another_group(
    "When this creature enters, return each creature you control with toughness X or less to its owner's hand, where X is the number of Islands you control."
)]
#[case::plural_owners(
    "When this creature enters, return each creature your opponents control with toughness X or less to their owners' hands, where X is the number of Islands you control."
)]
#[case::the_library(
    "When this creature enters, return each creature your opponents control with toughness X or less to its owner's library, where X is the number of Islands you control."
)]
#[case::another_count(
    "When this creature enters, return each creature your opponents control with toughness X or less to its owner's hand, where X is the number of Swamps you control."
)]
fn a_return_of_each_does_not_read_as_another_return(#[case] clause: &str) {
    assert!(
        !written(&scourge(r#"{"ControlledBy":"Opponents"}"#), clause),
        "{clause}"
    );
}

fn sly(duration: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{U}}"}},"Tap"],"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"ControlledBy":"Opponents"}}]}}}}],"effects":[{{"Continuous":{{"affected":{{"Target":0}},"mods":["CantBeBlocked"],"duration":"{duration}"}}}},{{"Goad":{{"what":{{"Target":0}}}}}}]}}}}"#
    )
}

#[test]
fn a_creature_that_cannot_be_blocked_until_your_next_turn_is_goaded_in_a_sentence_of_its_own() {
    let clause = "{U}, {T}: Until your next turn, target creature an opponent controls can't be blocked. Goad that creature.";
    assert!(written(&sly("UntilYourNextTurn"), clause));
}

#[rstest]
#[case::end_of_turn(
    "UntilYourNextTurn",
    "{U}, {T}: Target creature an opponent controls can't be blocked this turn. Goad that creature."
)]
#[case::the_wrong_duration(
    "UntilEndOfTurn",
    "{U}, {T}: Until your next turn, target creature an opponent controls can't be blocked. Goad that creature."
)]
#[case::no_goad(
    "UntilYourNextTurn",
    "{U}, {T}: Until your next turn, target creature an opponent controls can't be blocked."
)]
fn a_block_restriction_keeps_its_duration(#[case] duration: &str, #[case] clause: &str) {
    assert!(!written(&sly(duration), clause), "{clause}");
}

#[test]
fn a_block_restriction_for_the_turn_is_still_written_that_way() {
    let clause =
        "{U}, {T}: Target creature an opponent controls can't be blocked this turn. Then goad it.";
    assert!(written(&sly("UntilEndOfTurn"), clause));
}

fn greedily(group: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"In":"Graveyard"}}]}}}}],"effects":[{{"Move":{{"what":{{"Target":0}},"to":{{"Battlefield":{{"under":"You"}}}}}}}},{{"DealDamage":{{"source":{{"Target":0}},"to":{{"Each":[{group}]}},"amount":{{"Stat":{{"of":{{"Target":0}},"stat":"Power"}}}}}}}}]}}}}"#
    )
}

const OTHER_CREATURES: &str = r#"{"Type":"Creature"},{"Not":{"Is":{"Target":0}}}"#;

#[test]
fn a_creature_put_onto_the_battlefield_deals_damage_equal_to_its_power_to_each_other_creature() {
    let clause = "Put target creature card from a graveyard onto the battlefield under your control. That creature deals damage equal to its power to each other creature.";
    assert!(written(&greedily(OTHER_CREATURES), clause));
}

#[rstest]
#[case::every_creature(
    OTHER_CREATURES,
    "Put target creature card from a graveyard onto the battlefield under your control. That creature deals damage equal to its power to each creature."
)]
#[case::toughness(
    OTHER_CREATURES,
    "Put target creature card from a graveyard onto the battlefield under your control. That creature deals damage equal to its toughness to each other creature."
)]
#[case::yours(
    r#"{"Type":"Creature"},{"ControlledBy":"You"},{"Not":{"Is":{"Target":0}}}"#,
    "Put target creature card from a graveyard onto the battlefield under your control. That creature deals damage equal to its power to each other creature."
)]
fn damage_by_the_creature_does_not_read_as_other_damage(#[case] group: &str, #[case] clause: &str) {
    assert!(!written(&greedily(group), clause), "{clause}");
}

fn viseling(sum: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep","whose":"Opponents"}}}},"effects":[{{"DealDamage":{{"to":"Triggering","amount":{{"Sum":[{{"Count":[{{"In":"Hand"}},{{"OwnedBy":"Triggering"}}]}},{sum}]}}}}}}]}}}}"#
    )
}

#[test]
fn damage_may_be_the_cards_in_a_hand_less_a_number() {
    let clause = "At the beginning of each opponent's upkeep, ~ deals X damage to that player, where X is the number of cards in their hand minus 4.";
    assert!(written(&viseling("-4"), clause));
}

#[rstest]
#[case::another_number(
    "-3",
    "At the beginning of each opponent's upkeep, ~ deals X damage to that player, where X is the number of cards in their hand minus 4."
)]
#[case::your_hand(
    "-4",
    "At the beginning of each opponent's upkeep, ~ deals X damage to that player, where X is the number of cards in your hand minus 4."
)]
#[case::plus(
    "-4",
    "At the beginning of each opponent's upkeep, ~ deals X damage to that player, where X is the number of cards in their hand plus 4."
)]
#[case::no_number(
    "-4",
    "At the beginning of each opponent's upkeep, ~ deals X damage to that player, where X is the number of cards in their hand."
)]
fn a_count_less_a_number_does_not_read_as_another_amount(#[case] sum: &str, #[case] clause: &str) {
    assert!(!written(&viseling(sum), clause), "{clause}");
}

fn goods(duration: &str, free: bool) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":"Opponent"}}],"effects":[{{"ExileUntil":{{"who":{{"Target":0}},"filter":[{{"Not":{{"Type":"Land"}}}}],"as":"found","exiled_as":"exiled"}}}},{{"Rule":{{"rule":{{"MayCast":{{"from":"Exile","only":{{"Bound":"found"}},"free":{free}}}}},"duration":"{duration}"}}}}]}}}}"#
    )
}

#[test]
fn an_opponent_exiles_until_a_card_is_found_and_it_may_be_cast_this_turn() {
    let clause = "Target opponent exiles cards from the top of their library until they exile a nonland card. Until end of turn, you may cast that card without paying its mana cost.";
    assert!(written(&goods("UntilEndOfTurn", true), clause));
}

#[rstest]
#[case::until_your_next_turn(
    "UntilYourNextTurn",
    true,
    "Target opponent exiles cards from the top of their library until they exile a nonland card. Until end of turn, you may cast that card without paying its mana cost."
)]
#[case::not_free(
    "UntilEndOfTurn",
    false,
    "Target opponent exiles cards from the top of their library until they exile a nonland card. Until end of turn, you may cast that card without paying its mana cost."
)]
#[case::another_card(
    "UntilEndOfTurn",
    true,
    "Target opponent exiles cards from the top of their library until they exile a creature card. Until end of turn, you may cast that card without paying its mana cost."
)]
#[case::target_player(
    "UntilEndOfTurn",
    true,
    "Target player exiles cards from the top of their library until they exile a nonland card. Until end of turn, you may cast that card without paying its mana cost."
)]
fn a_cast_for_the_turn_does_not_read_as_another_cast(
    #[case] duration: &str,
    #[case] free: bool,
    #[case] clause: &str,
) {
    assert!(!written(&goods(duration, free), clause), "{clause}");
}

fn sheriff(group: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"Not":{{"AnyOf":[{group}]}}}}]}}}}],"effects":[{{"Destroy":{{"what":{{"Target":0}}}}}}]}}}}"#
    )
}

const OUTLAWS: &str = r#"{"Subtype":"Assassin"},{"Subtype":"Mercenary"},{"Subtype":"Pirate"},{"Subtype":"Rogue"},{"Subtype":"Warlock"}"#;

#[test]
fn a_creature_that_is_none_of_the_outlaw_types_is_non_outlaw() {
    assert!(written(
        &sheriff(OUTLAWS),
        "Destroy target non-outlaw creature."
    ));
}

#[rstest]
#[case::outlaw_creature(OUTLAWS, "Destroy target outlaw creature.")]
#[case::one_type(OUTLAWS, "Destroy target non-Pirate creature.")]
#[case::fewer_types(
    r#"{"Subtype":"Assassin"},{"Subtype":"Pirate"}"#,
    "Destroy target non-outlaw creature."
)]
fn a_non_outlaw_does_not_read_as_another_group(#[case] group: &str, #[case] clause: &str) {
    assert!(!written(&sheriff(group), clause), "{clause}");
}

fn at_end_step(condition: &str, amount: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"End"}}}},"if":{{"Compare":{{"left":{condition},"op":"Ge","right":{amount}}}}},"effects":[{{"May":{{"effects":[{{"Draw":{{}}}}]}}}}]}}}}"#
    )
}

const OPPONENT_LOST_LIFE: &str = r#"{"Tallied":{"who":"Opponents","what":"LifeLost"}}"#;
const YOU_LOST_LIFE: &str = r#"{"Tallied":{"who":"You","what":"LifeLost"}}"#;

#[test]
fn an_opponent_who_lost_life_this_turn_is_a_condition() {
    let clause = "At the beginning of each end step, if an opponent lost 3 or more life this turn, you may draw a card.";
    assert!(written(&at_end_step(OPPONENT_LOST_LIFE, "3"), clause));
}

#[rstest]
#[case::another_number(
    OPPONENT_LOST_LIFE,
    "3",
    "At the beginning of each end step, if an opponent lost 4 or more life this turn, you may draw a card."
)]
#[case::you(
    OPPONENT_LOST_LIFE,
    "3",
    "At the beginning of each end step, if you lost 3 or more life this turn, you may draw a card."
)]
#[case::gained(
    OPPONENT_LOST_LIFE,
    "3",
    "At the beginning of each end step, if an opponent gained 3 or more life this turn, you may draw a card."
)]
#[case::the_player_of_the_data(
    YOU_LOST_LIFE,
    "3",
    "At the beginning of each end step, if an opponent lost 3 or more life this turn, you may draw a card."
)]
fn life_lost_this_turn_does_not_read_as_another_fact(
    #[case] condition: &str,
    #[case] amount: &str,
    #[case] clause: &str,
) {
    assert!(
        !written(&at_end_step(condition, amount), clause),
        "{clause}"
    );
}

#[test]
fn an_attack_this_turn_is_a_condition() {
    let json = at_end_step(r#"{"Tallied":{"who":"You","what":"Attackers"}}"#, "1");
    let clause =
        "At the beginning of each end step, if you attacked this turn, you may draw a card.";
    assert!(written(&json, clause));
    let two = at_end_step(r#"{"Tallied":{"who":"You","what":"Attackers"}}"#, "2");
    assert!(!written(&two, clause));
}

fn wayfarer(op: &str, mine: &str, theirs: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{W}}"}},"Tap"],"condition":{{"Compare":{{"left":{{"Largest":{{"who":"Opponents","of":[{theirs},{{"ControlledBy":"You"}}]}}}},"op":"{op}","right":{{"Count":[{mine},{{"ControlledBy":"You"}}]}}}}}},"effects":[{{"Draw":{{}}}}]}}}}"#
    )
}

#[test]
fn an_opponent_who_controls_more_lands_than_you_is_a_condition() {
    let json = wayfarer("Gt", r#"{"Type":"Land"}"#, r#"{"Type":"Land"}"#);
    assert!(written(
        &json,
        "{W}, {T}: Draw a card. Activate only if an opponent controls more lands than you."
    ));
}

#[rstest]
#[case::fewer(
    "Lt",
    r#"{"Type":"Land"}"#,
    r#"{"Type":"Land"}"#,
    "{W}, {T}: Draw a card. Activate only if an opponent controls more lands than you."
)]
#[case::creatures(
    "Gt",
    r#"{"Type":"Land"}"#,
    r#"{"Type":"Land"}"#,
    "{W}, {T}: Draw a card. Activate only if an opponent controls more creatures than you."
)]
#[case::another_count(
    "Gt",
    r#"{"Type":"Creature"}"#,
    r#"{"Type":"Land"}"#,
    "{W}, {T}: Draw a card. Activate only if an opponent controls more lands than you."
)]
#[case::you_control_more(
    "Gt",
    r#"{"Type":"Land"}"#,
    r#"{"Type":"Land"}"#,
    "{W}, {T}: Draw a card. Activate only if you control more lands than an opponent."
)]
fn a_comparison_of_lands_does_not_read_as_another_comparison(
    #[case] op: &str,
    #[case] mine: &str,
    #[case] theirs: &str,
    #[case] clause: &str,
) {
    assert!(!written(&wayfarer(op, mine, theirs), clause), "{clause}");
}
