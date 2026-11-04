//! Goal: the zone-change keys en2 maps (remembered picks, counters and control on entering, the search of another zone, a
//! move from either of two zones, a library position, a dig into the library) mean what the card prints, and a shape the
//! node cannot say stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn spell(line: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ {line}\nSVar:Win:DB$ GainLife | LifeAmount$ 2\nSVar:Lose:DB$ LoseLife | LifeAmount$ 2\n"
    )
}

/// The effects of the one spell the line makes.
fn effects(line: &str) -> Value {
    let script = spell(line);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    normal(found[0].clone())["Spell"]["effects"].clone()
}

fn is_gap(line: &str, gap: &str) {
    let script = spell(line);
    assert_eq!(abilities(&script), Vec::<Value>::new(), "{script}");
    assert!(
        gaps(&script).iter().any(|found| found == gap),
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::exile_from_the_hand(
    "ChangeZone | Origin$ Hand | Destination$ Exile | ChangeType$ Card | Hidden$ True | Mandatory$ True | RememberChanged$ True",
    json!([
        {"Choose": {"from": {"Each": [{"In": "Hand"}, {"OwnedBy": "You"}]}, "as": "picked"}},
        {"Exile": {"what": {"Bound": "picked"}, "as": "exiled2"}}])
)]
#[case::return_to_the_hand(
    "ChangeZone | Origin$ Graveyard | Destination$ Hand | ChangeType$ Creature.YouOwn | Hidden$ True | Optional$ True | RememberChanged$ True",
    json!([
        {"Choose": {"from": {"Each": [{"Type": "Creature"}, {"OwnedBy": "You"}, {"In": "Graveyard"}]},
            "count": {"min": 0, "max": 1}, "as": "picked"}},
        {"Move": {"what": {"Bound": "picked"}, "to": "Hand", "as": "moved2"}}])
)]
fn a_pick_from_a_hidden_zone_remembers_what_it_moved(#[case] line: &str, #[case] expected: Value) {
    let found = effects(line);
    assert_eq!(found, normal_effects(&expected));
}

/// The effects as the DSL writes them back.
fn normal_effects(effects: &Value) -> Value {
    let wrapped = json!({"Spell": {"effects": effects}});
    normal(wrapped)["Spell"]["effects"].clone()
}

#[rstest]
#[case::forget_the_others(
    "ChangeZone | Origin$ Hand | Destination$ Exile | ChangeType$ Card | Hidden$ True | RememberChanged$ True | ForgetOtherRemembered$ True",
    "key:ChangeZone.Choose.ForgetOtherRemembered"
)]
fn a_remembering_pick_with_another_key_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    is_gap(line, gap);
}

#[rstest]
#[case::counters(
    "ChangeZone | Origin$ Graveyard | Destination$ Battlefield | ValidTgts$ Creature.YouOwn | WithCountersType$ P1P1 | WithCountersAmount$ 2",
    json!({"counters": [["+1/+1", 2]]})
)]
#[case::one_counter_without_an_amount(
    "ChangeZone | Origin$ Graveyard | Destination$ Battlefield | ValidTgts$ Creature.YouOwn | WithCountersType$ FINALITY",
    json!({"counters": [[{"Other": "finality"}, 1]]})
)]
#[case::control_and_counters(
    "ChangeZone | Origin$ Graveyard | Destination$ Battlefield | ValidTgts$ Creature | GainControl$ True | WithCountersType$ CORPSE",
    json!({"under": "You", "counters": [[{"Other": "corpse"}, 1]]})
)]
fn a_card_entering_the_battlefield_comes_with_its_counters_and_under_control(
    #[case] line: &str,
    #[case] enters: Value,
) {
    let found = effects(line);
    let to = &found[0]["Move"]["to"]["Battlefield"];
    for (key, value) in enters.as_object().into_iter().flatten() {
        assert_eq!(&to[key], value, "{found}");
    }
}

#[rstest]
#[case::counted_amount(
    "ChangeZone | Origin$ Graveyard | Destination$ Battlefield | ValidTgts$ Creature.YouOwn | WithCountersType$ P1P1 | WithCountersAmount$ X",
    "key:ChangeZone.WithCountersAmount"
)]
#[case::all_is_not_a_kind(
    "ChangeZone | Origin$ Graveyard | Destination$ Battlefield | ValidTgts$ Creature.YouOwn | WithCountersType$ ALL",
    "key:ChangeZone.WithCountersType"
)]
#[case::counters_to_the_hand(
    "ChangeZone | Origin$ Graveyard | Destination$ Hand | ValidTgts$ Creature.YouOwn | WithCountersType$ P1P1",
    "key:ChangeZone.WithCountersType"
)]
#[case::control_of_a_card_in_the_hand(
    "ChangeZone | Origin$ Graveyard | Destination$ Hand | ValidTgts$ Creature | GainControl$ True",
    "key:ChangeZone.GainControl"
)]
fn counters_and_control_beyond_the_node_stay_gaps(#[case] line: &str, #[case] gap: &str) {
    is_gap(line, gap);
}

#[rstest]
#[case::control_of_a_target(
    "ChangeZone | Origin$ Graveyard | Destination$ Battlefield | GainControl$ True | ValidTgts$ Creature",
    json!([{"Move": {"what": {"Target": 0}, "to": {"Battlefield": {"under": "You"}}}}])
)]
#[case::control_of_a_remembered_card(
    "ChangeZone | Origin$ Exile | Destination$ Battlefield | GainControl$ True | Defined$ Self",
    json!([{"Move": {"what": "This", "to": {"Battlefield": {"under": "You"}}}}])
)]
fn a_card_put_onto_the_battlefield_under_your_control_enters_under_you(
    #[case] line: &str,
    #[case] expected: Value,
) {
    let found = effects(line);
    assert_eq!(found[0]["Move"]["to"], expected[0]["Move"]["to"], "{found}");
    assert_eq!(
        found[0]["Move"]["what"], expected[0]["Move"]["what"],
        "{found}"
    );
}

#[rstest]
#[case::graveyard(
    "ChangeZone | Origin$ Library | OriginAlternative$ Graveyard | Destination$ Hand | ChangeType$ Card.YouOwn+Creature | Optional$ True | SpellDescription$ You may search your library and/or graveyard for a creature card.",
    vec!["Graveyard"]
)]
#[case::graveyard_and_hand(
    "ChangeZone | Origin$ Library | OriginAlternative$ Graveyard,Hand | Destination$ Battlefield | ChangeType$ Aura.YouOwn | Hidden$ True | ShuffleNonMandatory$ True",
    vec!["Graveyard", "Hand"]
)]
fn a_search_of_the_library_and_other_zones_lists_them_and_shuffles_only_after_a_library_search(
    #[case] line: &str,
    #[case] also: Vec<&str>,
) {
    let found = effects(line);
    // A "may" wraps the steps of an optional search.
    let found = match found[0].get("May") {
        Some(may) => may["effects"].clone(),
        None => found,
    };
    let found = found.as_array().cloned().unwrap_or_default();
    let search = found
        .iter()
        .find_map(|effect| effect.get("Search"))
        .unwrap_or(&Value::Null);
    assert_eq!(search["also"], json!(also), "{found:?}");
    let shuffle = found.last().cloned().unwrap_or_default();
    assert_eq!(
        shuffle["If"]["condition"],
        json!({"Searched": {"zone": "Library"}}),
        "{found:?}"
    );
    assert_eq!(shuffle["If"]["then"], json!([{"Shuffle": {}}]), "{found:?}");
}

#[rstest]
#[case::sideboard(
    "ChangeZone | Origin$ Library | OriginAlternative$ Graveyard,Sideboard | Destination$ Hand | ChangeType$ Card",
    "key:ChangeZone.Search.OriginAlternative"
)]
#[case::exile(
    "ChangeZone | Origin$ Library | OriginAlternative$ Exile | Destination$ Hand | ChangeType$ Card",
    "key:ChangeZone.Search.OriginAlternative"
)]
#[case::twice(
    "ChangeZone | Origin$ Library | OriginAlternative$ Graveyard,Graveyard | Destination$ Hand | ChangeType$ Card",
    "key:ChangeZone.Search.OriginAlternative"
)]
fn a_search_of_a_zone_the_node_cannot_search_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    is_gap(line, gap);
}

#[rstest]
#[case::either_zone_target(
    "ChangeZone | Origin$ Graveyard,Exile | Destination$ Hand | ValidTgts$ Creature.YouOwn",
    json!({"targets": [{"what": {"Object": [{"Type": "Creature"}, {"OwnedBy": "You"},
        {"AnyOf": [{"In": "Graveyard"}, {"In": "Exile"}]}]}}],
        "effects": [{"Move": {"what": {"Target": 0}, "to": "Hand"}}]})
)]
#[case::either_zone_the_card_itself(
    "ChangeZone | Origin$ Graveyard,Exile | Destination$ Battlefield | Defined$ Self | GainControl$ True",
    json!({"effects": [{"Move": {"what": "This", "to": {"Battlefield": {"under": "You"}}}}]})
)]
#[case::pick_from_either_zone(
    "ChangeZone | Origin$ Graveyard,Exile | Destination$ Hand | ChangeType$ Land.YouOwn | Hidden$ True | Mandatory$ True",
    json!({"effects": [
        {"Choose": {"from": {"Each": [{"Type": "Land"}, {"OwnedBy": "You"}, {"AnyOf": [{"In": "Graveyard"}, {"In": "Exile"}]}]},
            "as": "picked"}},
        {"Move": {"what": {"Bound": "picked"}, "to": "Hand"}}]})
)]
fn a_move_from_either_of_two_zones_reads_both(#[case] line: &str, #[case] body: Value) {
    let script = spell(line);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let want = normal(json!({ "Spell": body }));
    let got = normal(found[0].clone());
    assert_eq!(
        got["Spell"]["effects"][0], want["Spell"]["effects"][0],
        "{got}"
    );
    assert_eq!(got["Spell"]["targets"], want["Spell"]["targets"], "{got}");
}

#[rstest]
#[case::battlefield_beside_the_graveyard(
    "ChangeZone | Origin$ Graveyard,Battlefield | Destination$ Hand | ValidTgts$ Creature",
    "zone:Graveyard,Battlefield"
)]
#[case::the_same_zone_twice(
    "ChangeZone | Origin$ Exile,Exile | Destination$ Hand | ValidTgts$ Creature",
    "zone:Origin=Exile,Exile"
)]
fn a_list_of_zones_beyond_the_hand_graveyard_and_exile_stays_a_gap(
    #[case] line: &str,
    #[case] gap: &str,
) {
    is_gap(line, gap);
}

#[rstest]
#[case::dig_into_the_library_on_top(
    "Dig | DigNum$ 4 | ChangeNum$ 1 | Optional$ True | ChangeValid$ Creature | DestinationZone$ Library | LibraryPosition$ 0",
    json!({"Library": {"place": "Top"}}),
    json!({"Library": {"place": "Bottom", "order": "Chosen"}})
)]
#[case::dig_into_the_library_on_the_bottom(
    "Dig | DigNum$ 4 | ChangeNum$ 1 | Optional$ True | ChangeValid$ Creature | DestinationZone$ Library | LibraryPosition$ -1",
    json!({"Library": {"place": "Bottom"}}),
    json!({"Library": {"place": "Bottom", "order": "Chosen"}})
)]
#[case::the_rest_on_top(
    "Dig | DigNum$ 3 | ChangeNum$ 1 | ChangeValid$ Land | DestinationZone$ Hand | LibraryPosition2$ 0",
    json!("Hand"),
    json!({"Library": {"place": "Top", "order": "Chosen"}})
)]
#[case::the_rest_on_the_bottom_by_position(
    "Dig | DigNum$ 3 | ChangeNum$ 1 | ChangeValid$ Land | DestinationZone$ Hand | LibraryPosition2$ -1",
    json!("Hand"),
    json!({"Library": {"place": "Bottom", "order": "Chosen"}})
)]
fn a_dig_puts_what_it_takes_and_the_rest_where_the_library_position_says(
    #[case] line: &str,
    #[case] taken: Value,
    #[case] rest: Value,
) {
    let found = effects(line);
    let moves: Vec<&Value> = found
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|effect| effect.get("Move"))
        .collect();
    assert_eq!(moves.len(), 2, "{found}");
    assert_eq!(moves[0]["to"], taken, "{found}");
    assert_eq!(moves[1]["to"], rest, "{found}");
}

#[rstest]
#[case::counted(
    "Dig | DigNum$ 4 | ChangeNum$ 1 | ChangeValid$ Creature | DestinationZone$ Library | LibraryPosition$ 2",
    "key:Dig.LibraryPosition"
)]
#[case::the_rest_third_from_the_top(
    "Dig | DigNum$ 4 | ChangeNum$ 1 | ChangeValid$ Creature | DestinationZone$ Hand | LibraryPosition2$ 2",
    "key:Dig.LibraryPosition2"
)]
#[case::the_rest_in_the_graveyard_by_position(
    "Dig | DigNum$ 4 | ChangeNum$ 1 | ChangeValid$ Creature | DestinationZone$ Hand | DestinationZone2$ Graveyard | LibraryPosition2$ 0",
    "key:Dig.LibraryPosition2"
)]
fn a_dig_position_beyond_the_node_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    is_gap(line, gap);
}
