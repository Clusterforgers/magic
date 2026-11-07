//! Goal: round 15 pack e15b reads the card's own entering changes: "it becomes day as ~ enters" (`DayTime$ Neither`), a reveal of one of two land types or it enters tapped (`Reveal<1/Island;Mountain/...>`), and the choice of a card type, a listed card type or a (basic) land type that later lines read as the chosen type, and a counter of a kind chosen as it enters; another shape stays a gap.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn enters(by: &Value) -> Value {
    json!({"Replacement": {"when": {"Enters": {"what": ["This"]}}, "by": by}})
}

const DAY: &str = "R:Event$ Moved | ValidCard$ Card.Self | Destination$ Battlefield | DayTime$ Neither | ReplaceWith$ DoDay | ReplacementResult$ Updated\n";

#[rstest]
#[case::becomes_day("SVar:DoDay:DB$ DayTime | Value$ Day\n", true)]
#[case::becomes_night("SVar:DoDay:DB$ DayTime | Value$ Night\n", false)]
#[case::something_else("SVar:DoDay:DB$ Draw\n", false)]
fn the_day_it_brings(#[case] part: &str, #[case] read: bool) {
    let text = format!("Name:T\nTypes:Creature Human\nPT:1/1\n{DAY}{part}");
    if read {
        assert_eq!(gaps(&text), Vec::<String>::new());
        assert_eq!(abilities(&text)[0], enters(&json!("DayIfNeither")));
    } else {
        assert_ne!(gaps(&text), Vec::<String>::new());
    }
}

#[test]
fn a_day_line_of_another_card_stays_a_gap() {
    let text = format!(
        "Name:T\nTypes:Creature Human\nPT:1/1\n{}SVar:DoDay:DB$ DayTime | Value$ Day\n",
        DAY.replace("Card.Self", "Creature.Other")
    );
    assert_ne!(gaps(&text), Vec::<String>::new());
}

#[rstest]
#[case::two_types(
    "Reveal<1/Plains;Island/Plains or Island>",
    Some(json!([{"AnyOf": [{"Subtype": "Plains"}, {"Subtype": "Island"}]}, {"In": "Hand"}, {"OwnedBy": "You"}]))
)]
#[case::one_type("Reveal<1/Elf/Elf>", Some(json!([{"Subtype": "Elf"}, {"In": "Hand"}, {"OwnedBy": "You"}])))]
#[case::two_cards("Reveal<2/Plains;Island/Plains or Island>", None)]
fn a_reveal_or_it_enters_tapped(#[case] cost: &str, #[case] revealed: Option<Value>) {
    let text = format!(
        "Name:T\nTypes:Land\nR:Event$ Moved | ValidCard$ Card.Self | Destination$ Battlefield | ReplaceWith$ DBTap | ReplacementResult$ Updated\nSVar:DBTap:DB$ Tap | ETB$ True | Defined$ Self | UnlessCost$ {cost} | UnlessPayer$ You\n"
    );
    match revealed {
        Some(of) => {
            assert_eq!(gaps(&text), Vec::<String>::new());
            assert_eq!(abilities(&text)[0], enters(&json!({"RevealOrTapped": of})));
        }
        None => assert_ne!(gaps(&text), Vec::<String>::new()),
    }
}

fn chooser(choose: &str, rest: &str) -> String {
    format!(
        "Name:T\nTypes:Artifact\nK:ETBReplacement:Other:ChooseCT\nSVar:ChooseCT:DB$ ChooseType | Defined$ You | {choose}\n{rest}"
    )
}

const FLYING: &str =
    "S:Mode$ Continuous | Affected$ Creature.ChosenType | AddKeyword$ Flying | Description$ x\n";

#[rstest]
#[case::any_card_type("Type$ Card", json!("CardType"))]
#[case::five_card_types(
    "Type$ Card | ValidTypes$ Artifact,Creature,Enchantment,Instant,Sorcery",
    json!({"CardTypeOf": 285})
)]
#[case::other_than_creature_or_land(
    "Type$ Card | InvalidTypes$ Creature,Land",
    json!({"CardTypeOf": 443})
)]
#[case::a_basic_land_type("Type$ Basic Land", json!("BasicLandType"))]
#[case::a_land_type("Type$ Land", json!("LandType"))]
fn a_type_chosen_as_it_enters(#[case] choose: &str, #[case] kind: Value) {
    let text = chooser(choose, FLYING);
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = abilities(&text);
    assert_eq!(found[0], enters(&json!({ "Choose": kind })));
    assert_eq!(
        found[1],
        json!({"Static": {"affected": {"Each": [{"Type": "Creature"}, "ChosenSubtype"]}, "mods": [{"AddKeyword": "Flying"}]}})
    );
}

#[rstest]
#[case::one_listed_type("Type$ Card | ValidTypes$ Artifact")]
#[case::an_unknown_type("Type$ Card | ValidTypes$ Artifact,Conspiracy")]
#[case::basic_lands_but_some("Type$ Basic Land | InvalidTypes$ Mountain,Forest,Plains")]
#[case::a_planeswalker_type("Type$ Planeswalker")]
fn another_type_choice_stays_a_gap(#[case] choose: &str) {
    assert_ne!(gaps(&chooser(choose, FLYING)), Vec::<String>::new());
}

#[test]
fn the_chosen_basic_land_type_replaces_the_land_types_first() {
    let text = "Name:T\nTypes:Enchantment Aura\nK:Enchant:Land\nK:ETBReplacement:Other:ChooseLT\nSVar:ChooseLT:DB$ ChooseType | Type$ Basic Land\nS:Mode$ Continuous | AffectedDefined$ Enchanted | AddType$ ChosenType | RemoveLandTypes$ True | Description$ x\n";
    assert_eq!(gaps(text), Vec::<String>::new());
    assert_eq!(
        abilities(text)[2],
        json!({"Static": {"affected": "Attached", "mods": [{"SetLandTypes": []}, "AddChosenSubtype"]}})
    );
}

#[rstest]
#[case::chosen_land_type("Type$ Land", true)]
#[case::chosen_card_type("Type$ Card", false)]
fn landwalk_of_the_chosen_type(#[case] choose: &str, #[case] read: bool) {
    let text = chooser(
        choose,
        "S:Mode$ Continuous | Affected$ Creature.YouCtrl | AddKeyword$ Landwalk:ChosenType | Description$ x\n",
    );
    if read {
        assert_eq!(gaps(&text), Vec::<String>::new());
        assert_eq!(
            abilities(&text)[1]["Static"]["mods"],
            json!([{"AddKeyword": {"Landwalk": ["ChosenSubtype"]}}])
        );
    } else {
        assert_ne!(gaps(&text), Vec::<String>::new());
    }
}

#[rstest]
#[case::two_kinds("Deathtouch,Lifelink", Some(json!([{"Other": "deathtouch"}, {"Other": "lifelink"}])))]
#[case::first_strike("Flying,First Strike", Some(json!([{"Other": "flying"}, {"Other": "first strike"}])))]
#[case::one_kind("Deathtouch", None)]
#[case::an_unknown_kind("Deathtouch,Wibble", None)]
fn a_counter_of_a_kind_chosen_as_it_enters(#[case] kinds: &str, #[case] read: Option<Value>) {
    let text = format!(
        "Name:T\nTypes:Creature Elf\nPT:1/1\nK:ETBReplacement:Other:CounterChoice\nSVar:CounterChoice:DB$ PutCounter | ETB$ True | CounterType$ {kinds}\n"
    );
    match read {
        Some(kinds) => {
            assert_eq!(gaps(&text), Vec::<String>::new());
            assert_eq!(
                abilities(&text)[0],
                enters(&json!({ "WithCounterOf": kinds }))
            );
        }
        None => assert_ne!(gaps(&text), Vec::<String>::new()),
    }
}

#[rstest]
#[case::two_of_three("ChooseDifferent$ 2")]
#[case::two_counters("CounterNum$ 2")]
fn a_counter_choice_of_another_shape_stays_a_gap(#[case] extra: &str) {
    let text = format!(
        "Name:T\nTypes:Creature Elf\nPT:1/1\nK:ETBReplacement:Other:CounterChoice\nSVar:CounterChoice:DB$ PutCounter | ETB$ True | CounterType$ Menace,Deathtouch,Lifelink | {extra}\n"
    );
    assert_ne!(gaps(&text), Vec::<String>::new());
}

#[rstest]
#[case::counters_on_itself_unnamed("", true)]
#[case::counters_on_itself_named(" | Defined$ Self", true)]
#[case::counters_elsewhere(" | Defined$ Enchanted", false)]
fn tapped_and_with_counters(#[case] defined: &str, #[case] read: bool) {
    let text = format!(
        "Name:T\nTypes:Artifact\nR:Event$ Moved | ValidCard$ Card.Self | Destination$ Battlefield | ReplaceWith$ EntersTapped | ReplacementResult$ Updated\nSVar:EntersTapped:DB$ Tap | Defined$ Self | ETB$ True | SubAbility$ DBAddCounter\nSVar:DBAddCounter:DB$ PutCounter | ETB$ True | CounterType$ COIN | CounterNum$ 3{defined}\n"
    );
    if read {
        assert_eq!(gaps(&text), Vec::<String>::new());
        assert_eq!(
            abilities(&text)[0],
            enters(
                &json!({"WithCounters": {"counter": {"Other": "coin"}, "count": 3, "tapped": true}})
            )
        );
    } else {
        assert_ne!(gaps(&text), Vec::<String>::new());
    }
}
