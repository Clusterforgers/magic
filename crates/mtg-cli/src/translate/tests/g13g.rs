//! Goal: round 13 pack g13g reads the players' ban on searching libraries, the skip of extra turns, counters that stay on a
//! card as it changes zones and a cast permission whose after-the-cast check repeats its cards; each near-miss keeps its gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn card(types: &str, lines: &str) -> String {
    format!("Name:T\nManaCost:R\nTypes:{types}\n{lines}\nOracle:x\n")
}

#[rstest]
#[case::players_cant_search(
    "S:Mode$ Continuous | Affected$ Player | AddKeyword$ CantSearchLibrary | Description$ x",
    Some(json!({"Rule": {"rule": {"CantSearch": "EachPlayer"}}}))
)]
#[case::opponents_cant_search(
    "S:Mode$ Continuous | Affected$ Opponent | AddKeyword$ CantSearchLibrary | Description$ x",
    Some(json!({"Rule": {"rule": {"CantSearch": "Opponents"}}}))
)]
#[case::you_cant_search_stays_open(
    "S:Mode$ Continuous | Affected$ You | AddKeyword$ CantSearchLibrary | Description$ x",
    None
)]
#[case::everyone_skips_extra_turns(
    "R:Event$ BeginTurn | ActiveZones$ Battlefield | ExtraTurn$ True | Skip$ True | Description$ x",
    Some(json!({"Rule": {"rule": {"SkipExtraTurns": "EachPlayer"}}}))
)]
#[case::opponents_skip_extra_turns(
    "R:Event$ BeginTurn | ActiveZones$ Battlefield | ValidPlayer$ Opponent | ExtraTurn$ True | Skip$ True | Description$ x",
    Some(json!({"Rule": {"rule": {"SkipExtraTurns": "Opponents"}}}))
)]
#[case::you_skip_extra_turns_stays_open(
    "R:Event$ BeginTurn | ActiveZones$ Battlefield | ValidPlayer$ You | ExtraTurn$ True | Skip$ True | Description$ x",
    None
)]
#[case::a_skipped_turn_that_is_not_extra_stays_open(
    "R:Event$ BeginTurn | ActiveZones$ Battlefield | ValidPlayer$ Opponent | Skip$ True | Description$ x",
    None
)]
#[case::counters_remain(
    "S:Mode$ CountersRemain | ValidCard$ Card.Self | EffectZone$ All | Description$ x",
    Some(json!({"Rule": {"rule": "CountersRemain"}}))
)]
#[case::counters_remain_on_another_card_stays_open(
    "S:Mode$ CountersRemain | ValidCard$ Creature | EffectZone$ All | Description$ x",
    None
)]
fn g13g_rules(#[case] line: &str, #[case] expected: Option<Value>) {
    let text = card("Enchantment", line);
    match expected {
        Some(rule) => {
            assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
            assert_eq!(abilities(&text), vec![rule]);
        }
        None => assert!(!gaps(&text).is_empty()),
    }
}

const LURRUS: &str = "S:Mode$ Continuous | EffectZone$ Battlefield | MayPlay$ True | Affected$ Permanent.nonLand+YouOwn+cmcLE2 | ValidAfterStack$ Spell.cmcLE2 | AffectedZone$ Graveyard | Description$ x";
const ZASK: &str = "S:Mode$ Continuous | Affected$ Land.YouOwn,Insect.YouOwn | ValidAfterStack$ Spell.Insect | MayPlay$ True | AffectedZone$ Graveyard | Description$ play lands and cast Insect spells";
const GLARB: &str = "S:Mode$ Continuous | Affected$ Land.TopLibrary+YouCtrl,Card.TopLibrary+nonLand+YouCtrl | ValidAfterStack$ Spell.cmcGE4 | AffectedZone$ Library | MayPlay$ True | Description$ play";
const ANOTHER_WORD: &str = "S:Mode$ Continuous | Affected$ Insect.YouOwn | ValidAfterStack$ Spell.Bird | MayPlay$ True | AffectedZone$ Graveyard | Description$ x";

#[rstest]
#[case::the_check_repeats_the_cards(LURRUS, true)]
#[case::the_check_repeats_the_type_beside_a_land(ZASK, true)]
#[case::the_check_adds_a_limit_the_cards_lack(GLARB, false)]
#[case::the_check_names_another_word(ANOTHER_WORD, false)]
#[case::a_limit_beside_the_check(
    "S:Mode$ Continuous | Affected$ Insect.YouOwn | ValidAfterStack$ Spell.Insect | MayPlayLimit$ 1 | MayPlay$ True | AffectedZone$ Graveyard | Description$ x",
    false
)]
fn g13g_after_the_cast_check(#[case] line: &str, #[case] reads: bool) {
    let text = card("Creature", line);
    let found = gaps(&text);
    assert_eq!(
        found.iter().all(|gap| !gap.contains("ValidAfterStack")),
        reads,
        "{found:?}"
    );
}

#[rstest]
#[case::no_player_gains_life(
    "R:Event$ GainLife | ActiveZones$ Battlefield | Prevent$ True | AILogic$ NoLife | Description$ x",
    Some(json!({"Replacement": {"when": {"LifeGain": {"who": "EachPlayer"}}, "by": "Prevent"}}))
)]
#[case::only_opponents_gain_no_life(
    "R:Event$ GainLife | ActiveZones$ Battlefield | ValidPlayer$ Opponent | Prevent$ True | Description$ x",
    Some(json!({"Replacement": {"when": {"LifeGain": {"who": "Opponents"}}, "by": "Prevent"}}))
)]
#[case::a_life_loss_instead_stays_open(
    "R:Event$ GainLife | ActiveZones$ Battlefield | ValidPlayer$ Opponent | ReplaceWith$ RLose | Description$ x\nSVar:RLose:DB$ LoseLife | Defined$ ReplacedPlayer | LifeAmount$ X",
    None
)]
fn g13g_life_gain_is_prevented(#[case] line: &str, #[case] expected: Option<Value>) {
    let text = card("Enchantment", line);
    match expected {
        Some(replacement) => {
            assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
            assert_eq!(abilities(&text), vec![replacement]);
        }
        None => assert!(!gaps(&text).is_empty()),
    }
}
