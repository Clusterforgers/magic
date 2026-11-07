//! Goal: round 8 pack g8d reads players picked out by a property (`PlayersWhere`), death shields over what a spell dealt damage to, and verbs done a number of times, and leaves every other word a gap by name.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a card with the given type line, script lines and Oracle text, as compact JSON, and its gaps.
fn card(types: &str, lines: &str, oracle: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:{types}\nPT:2/2\n{lines}\nOracle:{oracle}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

const CREATURE_WORDS: &str =
    "If a creature dealt damage this way would die this turn, exile it instead.";
const PERMANENT_WORDS: &str =
    "If a permanent dealt damage this way would die this turn, exile it instead.";
const CREATURE_SHIELD: &str = r#"{"Shield":{"by":{"MoveTo":"Exile"},"duration":"UntilEndOfTurn","when":{"ZoneChange":{"from":"Battlefield","to":"Graveyard","what":[{"Type":"Creature"},{"DamagedBy":"This"}]}}}}"#;
const PERMANENT_SHIELD: &str = r#"{"Shield":{"by":{"MoveTo":"Exile"},"duration":"UntilEndOfTurn","when":{"ZoneChange":{"from":"Battlefield","to":"Graveyard","what":[{"DamagedBy":"This"}]}}}}"#;

#[rstest]
#[case::one_target_creature(
    "A:SP$ DealDamage | ValidTgts$ Any | NumDmg$ 2 | RememberDamaged$ True | ReplaceDyingDefined$ Remembered.Creature | SubAbility$ C\nSVar:C:DB$ Cleanup | ClearRemembered$ True",
    CREATURE_WORDS,
    CREATURE_SHIELD
)]
#[case::one_target_permanent(
    "A:SP$ DealDamage | ValidTgts$ Any | NumDmg$ 1 | RememberDamaged$ True | ReplaceDyingDefined$ Remembered | SubAbility$ C\nSVar:C:DB$ Cleanup | ClearRemembered$ True",
    PERMANENT_WORDS,
    PERMANENT_SHIELD
)]
#[case::each_creature(
    "A:SP$ DamageAll | ValidCards$ Creature | NumDmg$ 3 | RememberDamaged$ True | ReplaceDyingDefined$ Remembered | SubAbility$ C\nSVar:C:DB$ Cleanup | ClearRemembered$ True",
    CREATURE_WORDS,
    CREATURE_SHIELD
)]
fn what_the_spell_dealt_damage_to_is_exiled_instead_of_dying(
    #[case] lines: &str,
    #[case] oracle: &str,
    #[case] shield: &str,
) {
    let (json, found) = card("Instant", lines, oracle);
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(shield), "{json}");
}

#[rstest]
#[case::other_words(
    "A:SP$ DealDamage | ValidTgts$ Any | NumDmg$ 2 | RememberDamaged$ True | ReplaceDyingDefined$ Remembered | SubAbility$ C\nSVar:C:DB$ Cleanup | ClearRemembered$ True",
    "If a permanent dealt damage by Subject would die this turn, exile it instead.",
    "key:ReplaceDyingDefined=Remembered-other-words"
)]
#[case::another_source(
    "A:SP$ DealDamage | ValidTgts$ Any | NumDmg$ 2 | DamageSource$ Enchanted | RememberDamaged$ True | ReplaceDyingDefined$ Remembered.Creature",
    CREATURE_WORDS,
    "key:ReplaceDyingDefined=Remembered.Creature"
)]
fn a_death_shield_the_words_do_not_state_stays_a_gap(
    #[case] lines: &str,
    #[case] oracle: &str,
    #[case] gap: &str,
) {
    let (_, found) = card("Instant", lines, oracle);
    assert!(found.iter().any(|name| name == gap), "{found:?}");
}

#[rstest]
#[case::explores_twice(
    "DB$ Explore | Num$ 2",
    r#"[{"Explore":{"what":"This"}},{"Explore":{"what":"This"}}]"#
)]
#[case::explores_x_times(
    "DB$ Explore | Num$ X",
    r#"[{"RepeatWhile":{"condition":{"Compare":{"left":0,"op":"Eq","right":0}},"effects":[{"Explore":{"what":"This"}}],"most":1000,"times":"X"}}]"#
)]
#[case::incubates_twice(
    "DB$ Incubate | Amount$ 2 | Times$ 2",
    r#"[{"Incubate":{"count":2}},{"Incubate":{"count":2}}]"#
)]
#[case::incubates_once("DB$ Incubate | Amount$ 3", r#"[{"Incubate":{"count":3}}]"#)]
fn a_verb_is_done_as_many_times_as_it_says(#[case] line: &str, #[case] effects: &str) {
    let (json, found) = card(
        "Creature Elf",
        &format!(
            "T:Mode$ ChangesZone | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ E\nSVar:E:{line}\nSVar:X:Count$xPaid"
        ),
        "",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(&format!(r#""effects":{effects}"#)), "{json}");
}

#[rstest]
#[case::lost_life(
    "PlayerCountOpponents$HasPropertyLostLifeThisTurn",
    r#"{"Players":{"PlayersWhere":{"test":{"Compare":{"left":{"Tallied":{"what":"LifeLost","who":"You"}},"op":"Ge","right":1}},"who":"Opponents"}}}"#
)]
#[case::dealt_damage(
    "PlayerCountRegisteredOpponents$HasPropertywasDealtDamageThisTurn",
    r#"{"Players":{"PlayersWhere":{"test":{"Compare":{"left":{"Tallied":{"what":"DamageTaken","who":"You"}},"op":"Ge","right":1}},"who":"Opponents"}}}"#
)]
#[case::corrupted(
    "PlayerCountOpponents$HasPropertyIsCorrupted",
    r#"{"Players":{"PlayersWhere":{"test":{"Compare":{"left":{"Counters":{"counter":"poison","on":"You"}},"op":"Ge","right":3}},"who":"Opponents"}}}"#
)]
#[case::graveyard(
    "PlayerCountPlayers$HasPropertyHasCardsInGraveyard_Instant,Sorcery_GE1",
    r#""who":"EachPlayer""#
)]
#[case::monarch("PlayerCountPlayers$HasPropertyisMonarch", r#"{"Players":"Monarch"}"#)]
fn a_count_of_players_with_a_property(#[case] svar: &str, #[case] expected: &str) {
    let (json, found) = card(
        "Sorcery",
        &format!("A:SP$ Draw | NumCards$ X\nSVar:X:{svar}"),
        "",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[test]
fn the_graveyard_count_is_of_the_cards_the_player_owns_there() {
    let (json, _) = card(
        "Sorcery",
        "A:SP$ Draw | NumCards$ X\nSVar:X:PlayerCountPlayers$HasPropertyHasCardsInGraveyard_Card_GE7",
        "",
    );
    assert!(
        json.contains(r#"{"Compare":{"left":{"Count":[{"OwnedBy":"You"},{"In":"Graveyard"}]},"op":"Ge","right":7}}"#),
        "{json}"
    );
}

#[rstest]
#[case::more_lands("PlayerCountOpponents$HasPropertywithMoreLandsThanYou")]
#[case::combat_damage("PlayerCountOpponents$HasPropertywasDealtCombatDamageThisTurn")]
#[case::monarch_among_opponents("PlayerCountOpponents$HasPropertyisMonarch")]
#[case::no_least("PlayerCountPlayers$HasPropertyHasCardsInGraveyard_Card_GE0")]
fn a_property_the_table_lacks_stays_a_gap_by_name(#[case] svar: &str) {
    let (_, found) = card(
        "Sorcery",
        &format!("A:SP$ Draw | NumCards$ X\nSVar:X:{svar}"),
        "",
    );
    assert_eq!(found, [format!("amount:{svar}")]);
}

#[rstest]
#[case::not_their_turn(
    "Player.NonActive",
    r#"{"PlayersWhere":{"test":{"Not":"YourTurn"},"who":"EachPlayer"}}"#
)]
#[case::opponent_not_their_turn(
    "Opponent.NonActive",
    r#"{"PlayersWhere":{"test":{"Not":"YourTurn"},"who":"Opponents"}}"#
)]
#[case::opponent_on_their_turn(
    "Opponent.Active",
    r#"{"PlayersWhere":{"test":"YourTurn","who":"Opponents"}}"#
)]
fn a_cast_trigger_reads_the_players_with_a_property(#[case] word: &str, #[case] who: &str) {
    let (json, found) = card(
        "Creature Elf",
        &format!(
            "T:Mode$ SpellCast | ValidCard$ Card | ValidActivatingPlayer$ {word} | TriggerZones$ Battlefield | Execute$ D\nSVar:D:DB$ Draw"
        ),
        "",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(&format!(r#""by":{who}"#)), "{json}");
}

#[test]
fn players_cast_only_during_their_own_turns() {
    let (json, found) = card(
        "Enchantment",
        "S:Mode$ CantBeCast | ValidCard$ Card | Caster$ Player.NonActive",
        "",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(
        json.contains(r#""who":{"PlayersWhere":{"test":{"Not":"YourTurn"},"who":"EachPlayer"}}"#),
        "{json}"
    );
}

#[rstest]
#[case::another_group("Card.NonActive")]
#[case::another_property("Player.withMoreLandsThanYou")]
fn another_player_word_stays_a_gap(#[case] word: &str) {
    let (_, found) = card(
        "Creature Elf",
        &format!(
            "T:Mode$ SpellCast | ValidCard$ Card | ValidActivatingPlayer$ {word} | TriggerZones$ Battlefield | Execute$ D\nSVar:D:DB$ Draw"
        ),
        "",
    );
    assert_eq!(found, [format!("player:{word}")]);
}
