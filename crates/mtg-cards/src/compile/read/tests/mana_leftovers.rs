//! Goal: the mana wordings that count a tally of the turn (cards drawn, lands played), name the permanents controlled, mill as a cost, count the hand of a target player, give mana to another player or come with the attack of a creature or the one it equips read, each writes back as the card's text, and a near-miss stays with the table.
use super::*;

#[rstest]
#[case::cards_drawn(
    "{T}: Add {C} for each card you've drawn this turn.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]},"times":{"Tallied":{"who":"You","what":"Drawn"}}}}]}}"#
)]
#[case::a_land_was_played(
    "{T}: Add {U}. If you played a land this turn, add {B} instead.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Instead":{"base":{"Fixed":["Blue"]},"when":{"Compare":{"left":{"Tallied":{"who":"You","what":"LandsPlayed"}},"op":"Ge","right":1}},"instead":{"Fixed":["Black"]}}}}}]}}"#
)]
#[case::two_named_lands(
    "{T}: Add {C}. If you control an Urza's Power-Plant and an Urza's Tower, add {C}{C} instead.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Instead":{"base":{"Fixed":["Colorless"]},"when":{"All":[{"Compare":{"left":{"Count":[{"Subtype":"Urza's Power-Plant"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},{"Compare":{"left":{"Count":[{"Subtype":"Urza's Tower"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}]},"instead":{"Fixed":["Colorless","Colorless"]}}}}}]}}"#
)]
#[case::mill_for_colorless(
    "{T}, Mill a card: Add {C}.",
    r#"{"Activated":{"cost":["Tap",{"Mill":1}],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}"#
)]
#[case::mill_two(
    "{T}, Mill two cards: Add {C}.",
    r#"{"Activated":{"cost":["Tap",{"Mill":2}],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}"#
)]
#[case::mana_for_a_target_player(
    "{T}, Mill a card: Target player adds one mana of any color.",
    r#"{"Activated":{"cost":["Tap",{"Mill":1}],"targets":[{"what":"Player"}],"effects":[{"AddMana":{"who":{"Target":0},"mana":{"AnyColor":1}}}]}}"#
)]
#[case::x_counters_for_x_mana(
    "{T}, Remove X +1/+1 counters from this creature: Add X mana of any one color.",
    r#"{"Activated":{"cost":["Tap",{"RemoveCounters":{"counter":"+1/+1","count":"X"}}],"effects":[{"AddMana":{"mana":{"AnyColor":"X"}}}]}}"#
)]
fn tallied_mana_milled_costs_and_other_players_mana_read_and_write_back(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Activated, false, json);
}

#[rstest]
#[case::drawn_by_an_opponent("{T}: Add {C} for each card an opponent has drawn this turn.")]
#[case::lands_by_a_player("{T}: Add {U}. If a player played a land this turn, add {B} instead.")]
#[case::no_condition_word("{T}: Add {U}. You played a land this turn, add {B} instead.")]
#[case::mill_from_an_opponents_library("{T}, Mill a card from an opponent's library: Add {C}.")]
#[case::mana_for_a_creature("{T}, Mill a card: Target creature adds one mana of any color.")]
#[case::a_zone_word_stays_the_cards("Boast \u{2014} {1}: Add {C}.")]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert!(
        clause(text, ClauseKind::Activated, false).is_none(),
        "{text}"
    );
}

#[rstest]
#[case::a_target_opponents_hand(
    "Add {R} for each card in target opponent's hand.",
    r#"{"Spell":{"targets":[{"what":"Opponent"}],"effects":[{"AddMana":{"mana":{"Fixed":["Red"]},"times":{"Count":[{"In":"Hand"},{"OwnedBy":{"Target":0}}]}}}]}}"#
)]
#[case::a_target_players_hand(
    "Add {G} for each card in target player's hand.",
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"times":{"Count":[{"In":"Hand"},{"OwnedBy":{"Target":0}}]}}}]}}"#
)]
#[case::kept_and_exiled_with_counters(
    "Add {R} for each card in target opponent's hand. Until end of turn, you don't lose this mana as steps and phases end. Exile ~ with three time counters on it.",
    r#"{"Spell":{"targets":[{"what":"Opponent"}],"effects":[{"AddMana":{"mana":{"Fixed":["Red"]},"times":{"Count":[{"In":"Hand"},{"OwnedBy":{"Target":0}}]},"lasts":"Turn"}},{"Exile":{"what":"This","as":"exiled"}},{"PutCounters":{"on":{"Bound":"exiled"},"counter":{"Other":"time"},"count":3}}]}}"#
)]
fn mana_for_the_hand_of_a_target_and_an_exile_with_counters_read_and_write_back(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::a_graveyard("Add {R} for each card in target opponent's graveyard.")]
#[case::a_creatures_hand("Add {R} for each card in target creature's hand.")]
#[case::without_the_card_on_it("Add {R}. Exile ~ with three time counters.")]
#[case::another_card("Add {R}. Exile target card with three time counters on it.")]
fn near_misses_of_a_spell_stay_with_the_table(#[case] text: &str) {
    assert!(clause(text, ClauseKind::Spell, false).is_none(), "{text}");
}

#[rstest]
#[case::the_creature_or_the_one_it_equips(
    "Whenever this creature or equipped creature attacks, add an amount of {G} equal to its power. Until end of turn, you don't lose this mana as steps and phases end.",
    r#"{"Triggered":{"when":{"AnyOf":[{"Attacks":{"what":["This"]}},{"Attacks":{"what":[{"Is":"Attached"}]}}]},"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"times":{"Stat":{"of":"Triggering","stat":"Power"}},"lasts":"Turn"}}]}}"#
)]
#[case::the_equipped_creature(
    "Whenever equipped creature attacks, add an amount of {G} equal to its power.",
    r#"{"Triggered":{"when":{"Attacks":{"what":[{"Is":"Attached"}]}},"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"times":{"Stat":{"of":"Triggering","stat":"Power"}}}}]}}"#
)]
fn mana_equal_to_the_power_of_the_attacker_reads_and_writes_back(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::devouring_monster(
    "Devouring Monster \u{2014} {T}, Remove X +1/+1 counters from this creature: Add X mana of any one color."
)]
#[case::secrets_of_the_soul("Secrets of the Soul \u{2014} {T}: Add {B}.")]
fn an_ability_word_before_an_activated_ability_is_flavor(#[case] text: &str) {
    assert!(
        clause(text, ClauseKind::Activated, false).is_some(),
        "{text}"
    );
}

#[rstest]
#[case::the_cards_drawn(
    "Creatures you control get +X/+X, where X is the number of cards you've drawn this turn.",
    ClauseKind::Static,
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":{"Tallied":{"who":"You","what":"Drawn"}},"toughness":{"Tallied":{"who":"You","what":"Drawn"}}}}]}}"#
)]
#[case::a_bonus_for_each_card_drawn(
    "Draw a card. Until end of turn, target creature gains trample and gets +1/+0 for each card you've drawn this turn.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Draw":{}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Trample"},{"ModifyPT":{"power":{"Tallied":{"who":"You","what":"Drawn"}},"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
fn amounts_of_cards_drawn_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}
