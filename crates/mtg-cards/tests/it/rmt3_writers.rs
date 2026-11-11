//! Goal: the rmt3 writers read the rules of the cards they were made for back as the printed text, and a near miss (another
//! count, duration, group, kind of counter or a missing half of a joined sentence) is not accepted.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common::{printed, token_sources};

use mtg_cards::Caps;
use mtg_cards::load::Library;
use mtg_cards::proposal::{Verdict, check};
use rstest::rstest;

/// The verdict of the checker on `rules` for the corpus card `name`.
fn verdict(name: &str, rules: &str) -> Verdict {
    let cards = printed();
    let names: BTreeSet<String> = cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect();
    let tokens = Library::from_sources(&token_sources()).unwrap();
    let card = cards.iter().find(|card| card.name == name).unwrap();
    let report = check(rules, card, &tokens, &names, &Caps::everything());
    report.verdict()
}

/// One face of activated abilities with these costs and effects.
fn activated(abilities: &[(&str, &str)]) -> String {
    let list: Vec<String> = abilities
        .iter()
        .map(|(cost, effects)| format!(r#"{{"Activated":{{"cost":{cost},"effects":{effects}}}}}"#))
        .collect();
    format!(r#"{{"faces":[{{"abilities":[{}]}}]}}"#, list.join(","))
}

const ZAMECK_DRAW: (&str, &str) = (
    r#"[{"Mana":"{G}{U}"},{"RemoveCounters":{"counter":"+1/+1","from":[{"Type":"Creature"}]}}]"#,
    r#"[{"Draw":{}}]"#,
);

/// A shield of `count` counters of `counter` over creatures `whose` for `duration`.
fn shield(whose: &str, counter: &str, count: u32, duration: &str) -> String {
    format!(
        r#"[{{"Shield":{{"when":{{"Enters":{{"what":[{{"Type":"Creature"}},{{"ControlledBy":"{whose}"}}]}}}},"by":{{"WithCounters":{{"counter":"{counter}","count":{count}}}}},"duration":"{duration}"}}}}]"#
    )
}

#[rstest]
#[case::as_printed("You", "+1/+1", 1, "UntilEndOfTurn", true)]
#[case::two_counters("You", "+1/+1", 2, "UntilEndOfTurn", false)]
#[case::until_your_next_turn("You", "+1/+1", 1, "UntilYourNextTurn", false)]
#[case::opponents_creatures("Opponents", "+1/+1", 1, "UntilEndOfTurn", false)]
#[case::minus_counters("You", "-1/-1", 1, "UntilEndOfTurn", false)]
fn the_turns_shield_over_entering_creatures_reads_back(
    #[case] whose: &str,
    #[case] counter: &str,
    #[case] count: u32,
    #[case] duration: &str,
    #[case] reads: bool,
) {
    let rules = activated(&[
        (r#""{G}{U}""#, &shield(whose, counter, count, duration)),
        ZAMECK_DRAW,
    ]);
    assert_eq!(
        verdict("Zameck Guildmage", &rules) == Verdict::Accepted,
        reads
    );
}

/// Arlinn's faces with the +1 ability made of `plus`.
fn arlinn(plus: &str) -> String {
    let wolves = r#"{"Loyalty":{"cost":{"Minus":3},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Wolf","colors":["Green"],"types":["Creature"],"subtypes":["Wolf"],"power":2,"toughness":2}},"count":2}}]}}"#;
    let back = r#"{"abilities":[{"Keyword":"Nightbound"},{"Loyalty":{"cost":{"Plus":2},"effects":[{"AddMana":{"mana":{"Fixed":["Red","Green"]}}}]}},{"Loyalty":{"cost":"Zero","effects":[{"Continuous":{"affected":"This","mods":[{"SetPT":{"power":5,"toughness":5}},{"SetCreatureTypes":["Werewolf"]},{"AddType":"Creature"},{"AddKeyword":"Trample"},{"AddKeyword":"Indestructible"},{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}}]}}]}"#;
    format!(
        r#"{{"faces":[{{"abilities":[{{"Keyword":"Daybound"}},{{"Loyalty":{{"cost":{{"Plus":1}},"effects":{plus}}}}},{wolves}]}},{back}]}}"#
    )
}

const FLASH: &str = r#"{"Rule":{"rule":{"MayCast":{"what":[{"Type":"Creature"}],"flash":true}},"duration":"UntilYourNextTurn"}}"#;
const SORCERY_FLASH: &str = r#"{"Rule":{"rule":{"MayCast":{"what":[{"Type":"Sorcery"}],"flash":true}},"duration":"UntilYourNextTurn"}}"#;

#[rstest]
#[case::as_printed(format!("[{FLASH},{}]", &shield("You", "+1/+1", 1, "UntilYourNextTurn")[1..].trim_end_matches(']')), true)]
#[case::shield_for_the_turn_only(format!("[{FLASH},{}]", &shield("You", "+1/+1", 1, "UntilEndOfTurn")[1..].trim_end_matches(']')), false)]
#[case::sorcery_spells(format!("[{SORCERY_FLASH},{}]", &shield("You", "+1/+1", 1, "UntilYourNextTurn")[1..].trim_end_matches(']')), false)]
#[case::no_shield(format!("[{FLASH}]"), false)]
fn flash_and_entering_counters_until_your_next_turn_read_back_as_one_sentence(
    #[case] plus: String,
    #[case] reads: bool,
) {
    let found = verdict(
        "Arlinn, the Pack's Hope // Arlinn, the Moon's Fury",
        &arlinn(&plus),
    );
    assert_eq!(found == Verdict::Accepted, reads, "{found:?}");
}

const TORGAL_MANA: &str =
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#;

/// Torgal with the cast trigger `when` and the count `count`.
fn torgal(when: &str, count: &str) -> String {
    format!(
        r#"{{"faces":[{{"abilities":[{{"Triggered":{{"when":{when},"effects":[{{"SpellEntersWith":{{"spell":"Triggering","count":{count}}}}}]}}}},{TORGAL_MANA}]}}]}}"#
    )
}

const FIRST_HUMAN: &str = r#"{"Nth":{"event":{"SpellCast":{"what":[{"Type":"Creature"},{"Subtype":"Human"}],"by":"You"}},"nth":1}}"#;
const ANY_HUMAN: &str =
    r#"{"SpellCast":{"what":[{"Type":"Creature"},{"Subtype":"Human"}],"by":"You"}}"#;
const DOGS_AND_WOLVES: &str =
    r#"{"Count":[{"AnyOf":[{"Subtype":"Dog"},{"Subtype":"Wolf"}]},{"ControlledBy":"You"}]}"#;
const DOGS: &str = r#"{"Count":[{"Subtype":"Dog"},{"ControlledBy":"You"}]}"#;

#[rstest]
#[case::as_printed(FIRST_HUMAN, DOGS_AND_WOLVES, true)]
#[case::every_human(ANY_HUMAN, DOGS_AND_WOLVES, false)]
#[case::dogs_only(FIRST_HUMAN, DOGS, false)]
#[case::one_counter(FIRST_HUMAN, "1", false)]
fn a_counted_number_of_counters_for_the_cast_spell_reads_back(
    #[case] when: &str,
    #[case] count: &str,
    #[case] reads: bool,
) {
    assert_eq!(
        verdict("Torgal, A Fine Hound", &torgal(when, count)) == Verdict::Accepted,
        reads
    );
}

/// Runadi with the cast trigger counting the spell's mana value less `less`.
fn runadi(less: i32) -> String {
    format!(
        r#"{{"faces":[{{"abilities":[{{"Triggered":{{"when":{{"SpellCast":{{"what":[{{"Type":"Creature"}},{{"Compare":{{"stat":"ManaValue","op":"Ge","to":5}}}}],"by":"You"}}}},"effects":[{{"SpellEntersWith":{{"spell":"Triggering","count":{{"Sum":[{{"Stat":{{"of":"Triggering","stat":"ManaValue"}}}},{less}]}}}}}}]}}}},{{"Static":{{"affected":{{"Each":[{{"Type":"Creature"}},{{"ControlledBy":"You"}},{{"CounterCount":{{"kind":"+1/+1","op":"Ge","to":3}}}}]}},"mods":[{{"AddKeyword":"Haste"}}]}}}},{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{{"Fixed":["Green"]}}}}}}]}}}}]}}]}}"#
    )
}

#[rstest]
#[case::minus_four(-4, true)]
#[case::minus_three(-3, false)]
#[case::plus_four(4, false)]
fn the_mana_value_less_a_number_reads_back(#[case] less: i32, #[case] reads: bool) {
    assert_eq!(
        verdict("Runadi, Behemoth Caller", &runadi(less)) == Verdict::Accepted,
        reads
    );
}

/// Chocobo Camp's mana ability with the next cast of `what` giving `count` counters.
fn chocobo(what: &str, count: u32) -> String {
    format!(
        r#"{{"faces":[{{"abilities":[{{"Replacement":{{"when":{{"Enters":{{"what":["This"]}}}},"by":"Tapped","condition":{{"Not":{{"Compare":{{"left":{{"Count":[{{"Type":"Creature"}},{{"Supertype":"Legendary"}},{{"ControlledBy":"You"}}]}},"op":"Ge","right":1}}}}}}}}}},{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{{"Fixed":["Green"]}}}}}},{{"DelayedTrigger":{{"when":{{"SpellCast":{{"what":{what},"by":"You"}}}},"this_turn":true,"effects":[{{"SpellEntersWith":{{"spell":"Triggering","count":{count}}}}}]}}}}]}}}},{{"Activated":{{"cost":[{{"Mana":"{{2}}{{G}}{{G}}"}},"Tap"],"effects":[{{"CreateToken":{{"token":{{"Custom":{{"name":"Bird","colors":["Green"],"types":["Creature"],"subtypes":["Bird"],"power":2,"toughness":2,"abilities":[{{"Triggered":{{"when":{{"ZoneChange":{{"what":[{{"Type":"Land"}},{{"ControlledBy":"You"}}],"to":"Battlefield"}}}},"effects":[{{"Continuous":{{"affected":"This","mods":[{{"ModifyPT":{{"power":1,"toughness":0}}}}],"duration":"UntilEndOfTurn"}}}}]}}}}]}}}}}}}}]}}}}]}}]}}"#
    )
}

#[rstest]
#[case::as_printed(r#"[{"Type":"Creature"},{"Subtype":"Bird"}]"#, 1, true)]
#[case::any_creature(r#"[{"Type":"Creature"}]"#, 1, false)]
#[case::two_counters(r#"[{"Type":"Creature"},{"Subtype":"Bird"}]"#, 2, false)]
fn the_next_cast_of_a_kind_gives_its_spell_counters(
    #[case] what: &str,
    #[case] count: u32,
    #[case] reads: bool,
) {
    let found = verdict("Chocobo Camp", &chocobo(what, count));
    assert_eq!(found == Verdict::Accepted, reads, "{found:?}");
}

/// A one-target spell whose effect is the rule that the target skips `action` for `duration`.
fn skip_spell(target: &str, who: &str, action: &str, duration: &str) -> String {
    format!(
        r#"{{"faces":[{{"abilities":[{{"Spell":{{"targets":[{{"what":"{target}"}}],"effects":[{{"Rule":{{"rule":{{"Cant":{{"who":{who},"action":{action}}}}},"duration":"{duration}"}}}}]}}}}]}}]}}"#
    )
}

const TARGET: &str = r#"{"Target":0}"#;
const DRAW: &str = r#"{"SkipStep":"Draw"}"#;
const COMBAT: &str = r#""SkipCombat""#;

#[rstest]
#[case::fatigue("Fatigue", "Player", TARGET, DRAW, "UntilSkipped", true)]
#[case::fatigue_for_good("Fatigue", "Player", TARGET, DRAW, "Forever", false)]
#[case::fatigue_upkeep(
    "Fatigue",
    "Player",
    TARGET,
    r#"{"SkipStep":"Upkeep"}"#,
    "UntilSkipped",
    false
)]
#[case::fatigue_opponent("Fatigue", "Opponent", TARGET, DRAW, "UntilSkipped", false)]
#[case::fatigue_yourself("Fatigue", "Player", r#""You""#, DRAW, "UntilSkipped", false)]
#[case::silence(
    "Moment of Silence",
    "Player",
    TARGET,
    COMBAT,
    "UntilSkippedThisTurn",
    true
)]
#[case::silence_any_turn("Moment of Silence", "Player", TARGET, COMBAT, "UntilSkipped", false)]
#[case::silence_draw(
    "Moment of Silence",
    "Player",
    TARGET,
    DRAW,
    "UntilSkippedThisTurn",
    false
)]
fn a_skip_of_the_next_step_or_combat_reads_back(
    #[case] name: &str,
    #[case] target: &str,
    #[case] who: &str,
    #[case] action: &str,
    #[case] duration: &str,
    #[case] reads: bool,
) {
    let found = verdict(name, &skip_spell(target, who, action, duration));
    assert_eq!(found == Verdict::Accepted, reads, "{found:?}");
}

/// Avizoa's ability with the skip of `who`.
fn avizoa(who: &str) -> String {
    format!(
        r#"{{"faces":[{{"abilities":[{{"Keyword":"Flying"}},{{"Activated":{{"cost":"{{0}}","limit":"OncePerTurn","effects":[{{"Continuous":{{"affected":"This","mods":[{{"ModifyPT":{{"power":2,"toughness":2}}}}],"duration":"UntilEndOfTurn"}}}},{{"Rule":{{"rule":{{"Cant":{{"who":{who},"action":{{"SkipStep":"Untap"}}}}}},"duration":"UntilSkipped"}}}}]}}}}]}}]}}"#
    )
}

#[rstest]
#[case::you(r#""You""#, true)]
#[case::opponents(r#""Opponents""#, false)]
fn your_next_untap_step_reads_back(#[case] who: &str, #[case] reads: bool) {
    assert_eq!(verdict("Avizoa", &avizoa(who)) == Verdict::Accepted, reads);
}

/// A draw replaced by `instead` while `count` cards of the library are left.
fn empty_draw(instead: &str, left: u32) -> String {
    format!(
        r#"{{"faces":[{{"abilities":[{{"Replacement":{{"when":{{"Draw":{{}}}},"by":{{"Instead":{instead}}},"condition":{{"Compare":{{"left":{{"Count":[{{"In":"Library"}},{{"OwnedBy":"You"}}]}},"op":"Eq","right":{left}}}}}}}}}]}}]}}"#
    )
}

#[rstest]
#[case::a_win(r#"[{"WinGame":{}}]"#, 0, true)]
#[case::a_win_with_cards_left(r#"[{"WinGame":{}}]"#, 1, false)]
#[case::a_loss(r#"[{"LoseGame":{}}]"#, 0, false)]
fn a_draw_from_an_empty_library_replaced_by_a_win_reads_back(
    #[case] instead: &str,
    #[case] left: u32,
    #[case] reads: bool,
) {
    assert_eq!(
        verdict("Laboratory Maniac", &empty_draw(instead, left)) == Verdict::Accepted,
        reads
    );
}

/// Perilous Predicament with its two sacrifices of `second`.
fn predicament(second: &str) -> String {
    format!(
        r#"{{"faces":[{{"abilities":[{{"Spell":{{"effects":[{{"Sacrifice":{{"who":"Opponents","filter":[{{"Type":"Artifact"}},{{"Type":"Creature"}}]}}}},{{"Sacrifice":{{"who":{second},"filter":[{{"Not":{{"Type":"Artifact"}}}},{{"Type":"Creature"}}]}}}}]}}}}]}}]}}"#
    )
}

#[rstest]
#[case::the_same_players(r#""Opponents""#, true)]
#[case::another_player(r#""You""#, false)]
fn a_second_sacrifice_of_the_same_players_leaves_out_its_verb(
    #[case] second: &str,
    #[case] reads: bool,
) {
    assert_eq!(
        verdict("Perilous Predicament", &predicament(second)) == Verdict::Accepted,
        reads
    );
}

/// Winding Canyons with its permission to cast `kind` spells with flash for `duration`.
fn canyons(kind: &str, duration: &str) -> String {
    format!(
        r#"{{"faces":[{{"abilities":[{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{{"Fixed":["Colorless"]}}}}}}]}}}},{{"Activated":{{"cost":[{{"Mana":"{{2}}"}},"Tap"],"effects":[{{"Rule":{{"rule":{{"MayCast":{{"what":[{{"Type":"{kind}"}}],"flash":true}}}},"duration":"{duration}"}}}}]}}}}]}}]}}"#
    )
}

#[rstest]
#[case::creatures_this_turn("Creature", "UntilEndOfTurn", true)]
#[case::sorceries("Sorcery", "UntilEndOfTurn", false)]
#[case::until_your_next_turn("Creature", "UntilYourNextTurn", false)]
fn a_permission_to_cast_a_type_with_flash_for_a_while_reads_back(
    #[case] kind: &str,
    #[case] duration: &str,
    #[case] reads: bool,
) {
    assert_eq!(
        verdict("Winding Canyons", &canyons(kind, duration)) == Verdict::Accepted,
        reads
    );
}
