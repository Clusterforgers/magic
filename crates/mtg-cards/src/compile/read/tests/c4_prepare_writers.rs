//! Goal: the readers and writers of the prepare, chosen-player and life cards read and write the wordings of the life lost this way, the smallest hand at the table, a spell cast from the hand, two events of one trigger, damage to the chosen player (one replacement for the player and one for their permanents), an exile followed by a token copy, counters on up to X targets and a creature that prepares the card when it hurts a player, and a wrong player, group, count, order or step never reads back.
use super::*;
use crate::compile::read::static_pair_texts;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

#[rstest]
#[case::each_opponent(
    "Each opponent loses X life. You gain life equal to the life lost this way.",
    ClauseKind::Spell,
    true,
    r#"{"Spell":{"effects":[{"LoseLife":{"who":"Opponents","amount":"X"}},{"GainLife":{"amount":"LifeLostThisWay"}}]}}"#
)]
#[case::half_their_life(
    "Target opponent loses half their life, rounded up. You gain life equal to the life lost this way.",
    ClauseKind::Spell,
    false,
    r#"{"Spell":{"targets":[{"what":"Opponent"}],"effects":[{"LoseLife":{"who":{"Target":0},"amount":{"Half":{"of":{"Life":{"Target":0}},"up":true}}}},{"GainLife":{"amount":"LifeLostThisWay"}}]}}"#
)]
#[case::the_devotion_drain_of_the_gray_merchant(
    "When this creature enters, each opponent loses X life, where X is your devotion to black. You gain life equal to the life lost this way.",
    ClauseKind::Triggered,
    false,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"LoseLife":{"who":"Opponents","amount":{"Devotion":["Black"]}}},{"GainLife":{"amount":"LifeLostThisWay"}}]}}"#
)]
#[case::life_equal_to_the_devotion(
    "When this creature enters, you gain life equal to your devotion to black.",
    ClauseKind::Triggered,
    false,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"GainLife":{"amount":{"Devotion":["Black"]}}}]}}"#
)]
#[case::a_smallest_hand_of_a_player(
    "At the beginning of your upkeep, if a player has one or fewer cards in hand, this creature becomes prepared.",
    ClauseKind::Triggered,
    false,
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"LowestHand":"EachPlayer"},"op":"Le","right":1}},"effects":[{"Prepare":{"what":"This"}}]}}"#
)]
#[case::an_empty_hand_of_an_opponent(
    "At the beginning of your upkeep, if an opponent has no cards in hand, draw a card.",
    ClauseKind::Triggered,
    false,
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"LowestHand":"Opponents"},"op":"Le","right":0}},"effects":[{"Draw":{}}]}}"#
)]
#[case::a_big_spell_from_the_hand(
    "Whenever you cast an instant or sorcery spell with mana value 5 or greater from your hand, this creature becomes prepared.",
    ClauseKind::Triggered,
    false,
    r#"{"Triggered":{"when":{"SpellCast":{"what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"Compare":{"stat":"ManaValue","op":"Ge","to":5}},{"CastFrom":"Hand"}],"by":"You"}},"effects":[{"Prepare":{"what":"This"}}]}}"#
)]
#[case::two_events_of_one_trigger(
    "When this creature enters and whenever one or more creature tokens you control deal combat damage to a player, this creature becomes prepared.",
    ClauseKind::Triggered,
    false,
    r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"Batch":{"DealsDamage":{"source":[{"Type":"Creature"},"Token",{"ControlledBy":"You"}],"to":"EachPlayer","combat":true}}}]},"effects":[{"Prepare":{"what":"This"}}]}}"#
)]
fn the_wordings_of_the_prepare_cards_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] x_free: bool,
    #[case] json: &str,
) {
    reads_as(text, kind, x_free, json);
}

#[rstest]
#[case::the_life_lost_without_this_way("You gain life equal to the life lost.", ClauseKind::Spell)]
#[case::the_life_lost_this_turn(
    "You gain life equal to the life lost this turn.",
    ClauseKind::Spell
)]
#[case::each_player_has_few_cards(
    "At the beginning of your upkeep, if each player has one or fewer cards in hand, draw a card.",
    ClauseKind::Triggered
)]
#[case::a_player_has_more_cards(
    "At the beginning of your upkeep, if a player has two or more cards in hand, draw a card.",
    ClauseKind::Triggered
)]
#[case::an_opponent_casts_from_their_hand(
    "Whenever an opponent casts a spell from their hand, draw a card.",
    ClauseKind::Triggered
)]
#[case::from_the_graveyard_is_no_hand(
    "Whenever you cast a spell from your graveyard and from your hand, draw a card.",
    ClauseKind::Triggered
)]
#[case::enters_or_whenever(
    "When this creature enters or whenever one or more creature tokens you control deal combat damage to a player, draw a card.",
    ClauseKind::Triggered
)]
#[case::an_event_that_is_no_when(
    "When this creature enters and one or more creature tokens you control deal combat damage to a player, draw a card.",
    ClauseKind::Triggered
)]
fn near_misses_of_the_prepare_wordings_stay_with_the_table(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}

fn damage_to(to: &str, amount: &str, source: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"DealsDamage":{{"source":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}],"to":"EachPlayer","combat":true}}}},"effects":[{{"DealDamage":{{"source":"{source}","to":"{to}","amount":{amount}}}}}]}}}}"#
    )
}

#[rstest]
#[case::the_chosen_player("Chosen", r#""Triggering""#, "Dealer", true)]
#[case::you("You", r#""Triggering""#, "Dealer", false)]
#[case::a_fixed_amount("Chosen", "3", "Dealer", false)]
#[case::the_trigger_card_as_source("Chosen", r#""Triggering""#, "This", false)]
fn damage_that_repeats_to_the_chosen_player_is_one_sentence(
    #[case] to: &str,
    #[case] amount: &str,
    #[case] source: &str,
    #[case] read: bool,
) {
    let clause = "Whenever a creature you control deals combat damage to a player, it deals that much damage to the chosen player.";
    assert_eq!(written(&damage_to(to, amount, source), clause), read);
}

fn replacement(to: &str, factor: u32) -> String {
    format!(
        r#"{{"Replacement":{{"when":{{"Damage":{{"to":{to}}}}},"by":{{"Multiply":{factor}}}}}}}"#
    )
}

const PLAYER: &str = r#""Chosen""#;
const THEIRS: &str = r#"{"Each":[{"ControlledBy":"Chosen"}]}"#;
const MINE: &str = r#"{"Each":[{"ControlledBy":"You"}]}"#;

#[rstest]
#[case::the_player_and_their_permanents(PLAYER, 2, THEIRS, 2, true)]
#[case::the_other_way_round(THEIRS, 2, PLAYER, 2, false)]
#[case::triple(PLAYER, 3, THEIRS, 3, false)]
#[case::my_permanents(PLAYER, 2, MINE, 2, false)]
#[case::double_and_triple(PLAYER, 2, THEIRS, 3, false)]
fn two_replacements_of_damage_are_one_sentence_about_the_chosen_player(
    #[case] first: &str,
    #[case] first_factor: u32,
    #[case] second: &str,
    #[case] second_factor: u32,
    #[case] read: bool,
) {
    let clause = "If a source would deal damage to the chosen player or a permanent they control, it deals double that damage instead.";
    let (first, second) = (
        ability(&replacement(first, first_factor)),
        ability(&replacement(second, second_factor)),
    );
    let found = static_pair_texts(&first, &second)
        .iter()
        .any(|wording| same_text(wording, clause));
    assert_eq!(found, read);
}

fn exile_and_copy(second: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"AnyOf":[{{"Type":"Artifact"}},{{"Type":"Creature"}}]}},{{"In":"Graveyard"}},{{"OwnedBy":"You"}}]}}}}],"effects":[{{"Exile":{{"what":{{"Target":0}}}}}},{second}]}}}}"#
    )
}

#[rstest]
#[case::a_copy_of_the_target(r#"{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0}}}}}"#, true)]
#[case::a_treasure(r#"{"CreateToken":{"token":{"Named":"treasure"}}}"#, false)]
#[case::a_tapped_copy(
    r#"{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0}}},"tapped":true}}"#,
    false
)]
#[case::a_card_drawn(r#"{"Draw":{}}"#, false)]
fn an_exile_and_a_token_that_copies_the_card_are_two_sentences(
    #[case] second: &str,
    #[case] read: bool,
) {
    let clause = "Exile target artifact and/or creature card from your graveyard. Create a token that's a copy of it.";
    assert_eq!(written(&exile_and_copy(second), clause), read);
    let or = "Exile target artifact or creature card from your graveyard. Create a token that's a copy of it.";
    assert_eq!(written(&exile_and_copy(second), or), read);
}

fn counters_on_up_to(min: u32, counter: &str, grant: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}},"bounds":{{"min":{min},"max":"X"}}}}],"effects":[{{"PutCounters":{{"on":{{"Target":0}},"counter":"{counter}"}}}},{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"AddKeyword":"{grant}"}}],"duration":"UntilEndOfTurn"}}}},{{"Draw":{{}}}}]}}}}"#
    )
}

#[rstest]
#[case::up_to_x(0, "+1/+1", "Flying", true)]
#[case::at_least_one(1, "+1/+1", "Flying", false)]
#[case::another_counter(0, "-1/-1", "Flying", false)]
#[case::another_grant(0, "+1/+1", "Haste", false)]
fn counters_on_up_to_x_creatures_are_one_sentence_and_a_group_grant(
    #[case] min: u32,
    #[case] counter: &str,
    #[case] grant: &str,
    #[case] read: bool,
) {
    let clause = "Put a +1/+1 counter on each of up to X target creatures. Those creatures gain flying until end of turn. Draw a card.";
    assert_eq!(
        written(&counters_on_up_to(min, counter, grant), clause),
        read
    );
}

fn prepares_the_card(once: bool, combat: bool, effect: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"GroupAttacks":{{"what":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}}}},"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"AddKeyword":"Deathtouch"}}],"duration":"UntilEndOfTurn"}}}},{{"Choose":{{"from":{{"Target":0}},"as":"pumped"}}}},{{"DelayedTrigger":{{"when":{{"DealsDamage":{{"source":[{{"Is":{{"Bound":"pumped"}}}}],"to":"EachPlayer","combat":{combat}}}}},"once":{once},"effects":[{effect}]}}}}]}}}}"#
    )
}

#[rstest]
#[case::prepares_the_card(false, true, r#"{"Prepare":{"what":"This"}}"#, true)]
#[case::once(true, true, r#"{"Prepare":{"what":"This"}}"#, false)]
#[case::any_damage(false, false, r#"{"Prepare":{"what":"This"}}"#, false)]
#[case::another_effect(false, true, r#"{"Draw":{}}"#, false)]
fn a_creature_that_hurts_a_player_this_combat_prepares_the_card(
    #[case] once: bool,
    #[case] combat: bool,
    #[case] effect: &str,
    #[case] read: bool,
) {
    let clause = "Whenever you attack, target creature gains deathtouch until end of turn. Whenever that creature deals combat damage to a player this combat, this creature becomes prepared.";
    assert_eq!(
        written(&prepares_the_card(once, combat, effect), clause),
        read
    );
}
