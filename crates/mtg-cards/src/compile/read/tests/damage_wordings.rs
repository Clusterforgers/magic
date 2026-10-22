//! Goal: damage to a group of objects and a group of players ("each creature and each player"), damage with the life gained with it ("and you gain 3 life"), "it deals" from a card that dies, "without flying", and "you draw a card and you lose 1 life" read and write back the same text, a cost in a guild's order is the same cost, and three recipients, a target with a second recipient, another player's life and a count on the damage stay with the table.
use super::*;

#[rstest]
#[case::creatures_and_players(
    "~ deals 3 damage to each creature and each player.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"DealDamage":{"to":{"Each":[{"Type":"Creature"}]},"amount":3}},{"DealDamage":{"to":"EachPlayer","amount":3}}]}}"#
)]
#[case::without_flying(
    "~ deals 3 damage to each creature without flying.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"DealDamage":{"to":{"Each":[{"Type":"Creature"},{"Not":{"HasKeyword":"Flying"}}]},"amount":3}}]}}"#
)]
#[case::and_you_gain_life(
    "~ deals 3 damage to any target and you gain 3 life.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}},{"GainLife":{"amount":3}}]}}"#
)]
#[case::an_activation(
    "{T}: This creature deals 4 damage to each creature without flying and each player.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":["Tap"],"effects":[{"DealDamage":{"to":{"Each":[{"Type":"Creature"},{"Not":{"HasKeyword":"Flying"}}]},"amount":4}},{"DealDamage":{"to":"EachPlayer","amount":4}}]}}"#
)]
#[case::the_land_trigger(
    "When this land enters, it deals 1 damage to target opponent.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Opponent"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}}]}}"#
)]
#[case::each_other_creature(
    "When this creature enters, it deals 1 damage to each other creature.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"DealDamage":{"to":{"Each":[{"Type":"Creature"},"Other"]},"amount":1}}]}}"#
)]
#[case::each_opponent_and_gain(
    "When this creature enters, it deals 2 damage to each opponent and you gain 2 life.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"DealDamage":{"to":"Opponents","amount":2}},{"GainLife":{"amount":2}}]}}"#
)]
#[case::draw_and_lose(
    "When this creature enters, you draw a card and you lose 1 life.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Draw":{}},{"LoseLife":{"amount":1}}]}}"#
)]
#[case::a_cost_in_a_guilds_order(
    "{G}{W}, {T}, Sacrifice this artifact: Draw a card.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":[{"Mana":"{W}{G}"},"Tap","SacrificeSelf"],"effects":[{"Draw":{}}]}}"#
)]
fn the_wordings_read_into_the_dsl(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case("~ deals 2 damage to each creature and each player.")]
#[case("~ deals 1 damage to each creature and each opponent.")]
#[case("~ deals 5 damage to each creature without flying and each player.")]
#[case("~ deals 2 damage to each creature with flying.")]
#[case("~ deals 2 damage to target creature without flying.")]
#[case("~ deals 2 damage to you and each creature you control.")]
#[case("~ deals 4 damage to any target and you gain 4 life.")]
#[case("~ deals 2 damage to target creature and you gain 2 life.")]
#[case("~ deals 1 damage to each opponent and you gain 1 life.")]
#[case("~ deals 3 damage to target player or planeswalker and you gain 3 life.")]
#[case("You draw a card and you lose 1 life.")]
#[case("You draw two cards and you lose 2 life.")]
#[case("You draw a card and you gain 1 life.")]
#[case("You gain 2 life and you draw a card.")]
#[case("Target player draws a card and you lose 1 life.")]
fn spell_wordings_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case("~ deals X damage to each creature and each player.")]
#[case("~ deals X damage to any target and you gain X life.")]
#[case("You draw X cards and you lose X life.")]
fn x_reads_where_the_cost_has_one(#[case] text: &str) {
    golden(text, ClauseKind::Spell, true);
    assert!(clause(text, ClauseKind::Spell, false).is_none(), "{text}");
}

#[rstest]
#[case("When this land enters, it deals 1 damage to target opponent.")]
#[case("When this creature enters, it deals 1 damage to each other creature.")]
#[case("When this creature enters, it deals 2 damage to each opponent and you gain 2 life.")]
#[case("When this creature enters, ~ deals 2 damage to any target and you gain 2 life.")]
#[case("When this creature dies, it deals 3 damage to each creature and each player.")]
#[case("When this creature dies, it deals 4 damage to target opponent.")]
#[case("When this creature enters, you draw a card and you lose 1 life.")]
#[case("When this creature enters, you draw two cards and you lose 2 life.")]
#[case("Whenever this creature attacks, you draw a card and you lose 1 life.")]
#[case("Whenever another creature you control dies, you draw a card and you lose 1 life.")]
#[case(
    "Whenever another creature you control dies, ~ deals 1 damage to each opponent and you gain 1 life."
)]
fn trigger_wordings_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case("{B}: This creature deals 1 damage to each creature and each player.")]
#[case("{2}{B}: ~ deals 1 damage to each creature and each player.")]
#[case("{T}: This creature deals 4 damage to each creature without flying and each player.")]
#[case("{R}, {T}: ~ deals 2 damage to any target and you gain 2 life.")]
#[case("{G}{W}, {T}, Sacrifice this artifact: Draw a card.")]
#[case("{W}{G}, {T}, Sacrifice this artifact: Draw a card.")]
#[case("{R}{G}, {T}, Sacrifice this artifact: Draw a card.")]
#[case("{U}{B}, {T}, Sacrifice this artifact: You draw a card and you lose 1 life.")]
fn activated_wordings_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Activated, false);
}

#[rstest]
#[case::three_recipients("~ deals 3 damage to each creature, each planeswalker, and each player.")]
#[case::a_target_and_players("~ deals 3 damage to target creature and each player.")]
#[case::players_and_a_target("~ deals 3 damage to each creature and target player.")]
#[case::two_groups_of_objects("~ deals 3 damage to each Goblin and each Elf.")]
#[case::two_groups_of_players("~ deals 3 damage to each player and each opponent.")]
#[case::you_after_the_objects("~ deals 3 damage to each creature and you.")]
#[case::losing_life("~ deals 3 damage to any target and you lose 3 life.")]
#[case::another_player_gains("~ deals 3 damage to any target and each opponent gains 3 life.")]
#[case::a_target_gains("~ deals 3 damage to any target and target player gains 3 life.")]
#[case::life_first("You gain 3 life and ~ deals 3 damage to any target.")]
#[case::damage_for_each("~ deals 1 damage to each opponent for each creature you control.")]
#[case::a_third_effect("~ deals 3 damage to any target and you gain 3 life and draw a card.")]
#[case::without_a_keyword_word("~ deals 3 damage to each creature without.")]
#[case::without_a_stat("~ deals 3 damage to each creature without power 2.")]
fn the_near_misses_stay_with_the_table(#[case] text: &str) {
    assert!(clause(text, ClauseKind::Spell, false).is_none(), "{text}");
}

#[rstest]
#[case::another_player_draws("You draw a card and target opponent loses 1 life.")]
#[case::a_third_verb("You draw a card and you lose 1 life and you gain 1 life.")]
#[case::the_subject_alone("You draw a card and you.")]
#[case::a_stranger("You draw a card and they lose 1 life.")]
fn the_near_misses_of_the_repeated_subject_stay_with_the_table(#[case] text: &str) {
    assert!(clause(text, ClauseKind::Spell, false).is_none(), "{text}");
}

#[rstest]
#[case("{G}{W}", "{W}{G}")]
#[case("{R}{G}, {T}: Add {R} or {G}.", "{G}{R}, {T}: Add {R} or {G}.")]
#[case("{2}{G}{W}", "{G}{2}{W}")]
#[case("{G/W}{W/B}", "{W/B}{G/W}")]
fn a_run_of_mana_symbols_has_no_order(#[case] left: &str, #[case] right: &str) {
    assert!(same_text(left, right), "{left} / {right}");
}

#[rstest]
#[case("{G}{W}", "{G}{G}")]
#[case("{G}{W}", "{G}, {W}")]
#[case("{G}, {W}", "{W}, {G}")]
#[case("{T}: Add {G}.", "{T}: Add {W}.")]
#[case("{1}{G}", "{2}{G}")]
fn other_differences_in_symbols_stay(#[case] left: &str, #[case] right: &str) {
    assert!(!same_text(left, right), "{left} / {right}");
}
