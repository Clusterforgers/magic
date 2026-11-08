//! Goal: round 8 builder a reads facts of objects that selectors and conditions ask for (mutations, colors, the kicker paid, prepared, transformed and double-faced objects, a name shared with your graveyard, a cast way, a keyword without a parameter), player keywords a pump gives, the drawer who decides, a check of the source itself, the attacked player's life and the source's controller, and leaves near shapes gaps.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a card with the given header and lines as compact JSON, and its gaps.
fn card(types: &str, lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:{types}\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

#[rstest]
#[case::times_mutated(
    "Creature Beast\nPT:2/2\nA:AB$ GainLife | Cost$ T | LifeAmount$ X | SpellDescription$ x\nSVar:X:Count$TimesMutated",
    r#""amount":{"Mutations":"This"}"#
)]
#[case::colors_of_the_affected(
    "Artifact Equipment\nS:Mode$ Continuous | AffectedDefined$ Equipped | Affected$ Creature | AddPower$ AffectedX\nSVar:AffectedX:Count$CardNumColors\nK:Equip:1",
    r#""power":{"Stat":{"of":"Affected","stat":"Colors"}}"#
)]
#[case::all_colors(
    "Sorcery\nA:SP$ DestroyAll | ValidCards$ Creature.!AllColors | SpellDescription$ x",
    r#"{"Not":{"Compare":{"op":"Eq","stat":"Colors","to":5}}}"#
)]
#[case::madness(
    "Creature Zombie\nPT:2/2\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self+madness | Execute$ G | TriggerDescription$ x\nSVar:G:DB$ GainLife | LifeAmount$ 1",
    r#""what":["This",{"CastWith":"Madness"}]"#
)]
#[case::prepared(
    "Creature Elf\nPT:2/2\nS:Mode$ Continuous | Affected$ Creature.IsPrepared+YouCtrl | AddPower$ 1",
    r#""Prepared",{"ControlledBy":"You"}]"#
)]
#[case::transformed(
    "Creature Elf\nPT:2/2\nS:Mode$ Continuous | Affected$ Permanent.Transformed+YouCtrl | AddKeyword$ Trample",
    r#"["Transformed",{"ControlledBy":"You"}]"#
)]
#[case::double_faced(
    "Sorcery\nA:SP$ ChangeZone | Origin$ Library | Destination$ Hand | ChangeType$ Card.DoubleFaced | ChangeNum$ 1 | SpellDescription$ x",
    r#""DoubleFaced""#
)]
#[case::without_a_parameter_keyword(
    "Enchantment\nS:Mode$ Continuous | Affected$ Card.nonLand+YouOwn+withoutForetell | AffectedZone$ Hand | AddPower$ 1",
    r#"{"Not":{"HasKeywordNamed":"Foretell"}}"#
)]
#[case::shares_a_name_with_your_graveyard(
    "Enchantment\nT:Mode$ SpellCast | ValidCard$ Instant.sharesNameWith YourGraveyard | ValidActivatingPlayer$ You | TriggerZones$ Battlefield | Execute$ G | TriggerDescription$ x\nSVar:G:DB$ GainLife | LifeAmount$ 1",
    r#"{"SharesName":{"Each":[{"In":"Graveyard"},{"OwnedBy":"You"}]}}"#
)]
#[case::source_controller(
    "Enchantment Aura\nA:AB$ Draw | Cost$ 1 | NumCards$ 2 | Defined$ SourceController | SpellDescription$ x",
    r#""who":{"ControllerOf":"This"}"#
)]
fn a_fact_of_an_object_is_read(#[case] lines: &str, #[case] expected: &str) {
    let (header, rest) = lines.split_once('\n').unwrap();
    let (json, found) = card(header, rest);
    assert_eq!(found, Vec::<String>::new(), "{json}");
    assert!(json.contains(expected), "{json}");
}

/// A creature with two kickers and an enters trigger on `valid`.
fn kicked(kickers: &str, valid: &str) -> (String, Vec<String>) {
    card(
        "Creature Human",
        &format!(
            "PT:2/2\nK:Kicker:{kickers}\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ {valid} | Execute$ G | TriggerDescription$ x\nSVar:G:DB$ GainLife | LifeAmount$ 3"
        ),
    )
}

#[rstest]
#[case::the_first("Card.Self+kicked 1", r#""if":{"KickedWith":"{2}{U}"}"#)]
#[case::the_second("Card.Self+kicked 2", r#""if":{"KickedWith":"{1}{B}"}"#)]
fn an_enters_trigger_of_a_numbered_kicker_checks_that_kicker(
    #[case] valid: &str,
    #[case] expected: &str,
) {
    let (json, found) = kicked("2 U:1 B", valid);
    assert_eq!(found, Vec::<String>::new(), "{json}");
    assert!(json.contains(expected), "{json}");
    assert!(json.contains(r#""what":["This"]"#), "{json}");
}

#[rstest]
#[case::a_single_kicker("1 B", "Card.Self+kicked 1")]
#[case::a_third_kicker("2 U:1 B", "Card.Self+kicked 3")]
fn a_numbered_kicker_the_card_does_not_have_stays_a_gap(
    #[case] kickers: &str,
    #[case] valid: &str,
) {
    assert_ne!(kicked(kickers, valid).1, Vec::<String>::new());
}

#[rstest]
#[case::hexproof("Defined$ You | KW$ Hexproof", r#"{"Rule":{"duration":"UntilEndOfTurn","rule":{"PlayersHave":{"keyword":"Hexproof","who":"You"}}}}"#)]
#[case::shroud_until_your_next_turn(
    "Defined$ You | KW$ Shroud | Duration$ UntilYourNextTurn",
    r#"{"Rule":{"duration":"UntilYourNextTurn","rule":{"PlayersHave":{"keyword":"Shroud","who":"You"}}}}"#
)]
fn a_pump_of_a_player_with_a_player_keyword_is_a_rule(#[case] keys: &str, #[case] expected: &str) {
    let (json, found) = card(
        "Instant",
        &format!("A:SP$ Pump | {keys} | SpellDescription$ x"),
    );
    assert_eq!(found, Vec::<String>::new(), "{json}");
    assert_eq!(json, format!(r#"[{{"Spell":{{"effects":[{expected}]}}}}]"#));
}

#[rstest]
#[case::another_keyword("Defined$ You | KW$ Flying")]
#[case::a_power_change("Defined$ You | KW$ Hexproof | NumAtt$ +1")]
fn a_pump_of_a_player_that_is_no_player_keyword_stays_a_gap(#[case] keys: &str) {
    let (_, found) = card(
        "Instant",
        &format!("A:SP$ Pump | {keys} | SpellDescription$ x"),
    );
    assert_ne!(found, Vec::<String>::new());
}

#[rstest]
#[case::the_named_player(
    "Defined$ TriggeredDefendingPlayer | ",
    r#"{"May":{"effects":[{"Draw":{"who":"Defending"}}],"who":"Defending"}}"#
)]
#[case::you("", r#"{"May":{"effects":[{"Draw":{}}]}}"#)]
fn the_player_who_draws_decides_whether_to(#[case] defined: &str, #[case] expected: &str) {
    let (json, found) = card(
        "Creature Elf",
        &format!(
            "PT:1/1\nT:Mode$ Attacks | ValidCard$ Card.Self | Execute$ D | TriggerDescription$ x\nSVar:D:DB$ Draw | {defined}NumCards$ 1 | OptionalDecider$ True"
        ),
    );
    assert_eq!(found, Vec::<String>::new(), "{json}");
    assert!(json.contains(expected), "{json}");
}

#[test]
fn a_decider_word_on_another_verb_stays_a_gap() {
    let (_, found) = card(
        "Creature Elf",
        "PT:1/1\nT:Mode$ Attacks | ValidCard$ Card.Self | Execute$ D | TriggerDescription$ x\nSVar:D:DB$ GainLife | LifeAmount$ 1 | OptionalDecider$ True",
    );
    assert_ne!(found, Vec::<String>::new());
}

#[rstest]
#[case::is_not(
    "EQ0",
    r#""if":{"Not":{"Matches":{"selector":[{"Not":"Prepared"}],"subject":"This"}}}"#
)]
#[case::is(
    "GE1",
    r#""if":{"Matches":{"selector":[{"Not":"Prepared"}],"subject":"This"}}"#
)]
fn a_check_of_the_source_itself_is_whether_it_matches(
    #[case] compare: &str,
    #[case] expected: &str,
) {
    let (json, found) = card(
        "Creature Elf",
        &format!(
            "PT:1/1\nT:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | CheckSVar$ Count$ValidSelf Card.!IsPrepared | SVarCompare$ {compare} | TriggerZones$ Battlefield | Execute$ G | TriggerDescription$ x\nSVar:G:DB$ GainLife | LifeAmount$ 1"
        ),
    );
    assert_eq!(found, Vec::<String>::new(), "{json}");
    assert!(json.contains(expected), "{json}");
}

#[test]
fn a_check_of_the_source_with_a_count_stays_a_gap() {
    let (_, found) = card(
        "Creature Elf",
        "PT:1/1\nT:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | CheckSVar$ Count$ValidSelf Card.!IsPrepared | SVarCompare$ GE2 | TriggerZones$ Battlefield | Execute$ G | TriggerDescription$ x\nSVar:G:DB$ GainLife | LifeAmount$ 1",
    );
    assert_ne!(found, Vec::<String>::new());
}

#[test]
fn no_opponent_with_more_life_than_the_attacked_player_compares_with_the_most() {
    let (json, found) = card(
        "Creature Elf",
        "PT:1/1\nT:Mode$ Attacks | ValidCard$ Card.Self | Attacked$ Player | Condition$ NoOpponentHasMoreLifeThanAttacked | Execute$ G | TriggerDescription$ x\nSVar:G:DB$ GainLife | LifeAmount$ 1",
    );
    assert_eq!(found, Vec::<String>::new(), "{json}");
    assert!(
        json.contains(r#""if":{"Compare":{"left":{"Life":"Against"},"op":"Ge","right":{"HighestLife":"Opponents"}}}"#),
        "{json}"
    );
}
