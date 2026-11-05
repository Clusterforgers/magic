//! Goal: the keys fg5ka maps mean what the card prints, and a shape the node cannot say stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn spell(line: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ {line}\nSVar:Win:DB$ GainLife | LifeAmount$ 2\nSVar:Lose:DB$ LoseLife | LifeAmount$ 2\n"
    )
}

#[rstest]
#[case::counter_unless_life(
    "Counter | ValidTgts$ Card | TargetType$ Spell | UnlessCost$ PayLife<3>",
    json!({"Pay": {"who": {"ControllerOf": {"Target": 0}}, "cost": [{"PayLife": 3}],
        "unpaid": [{"Counter": {"what": {"Target": 0}}}]}})
)]
#[case::you_unless_discard(
    "GainLife | LifeAmount$ 2 | UnlessCost$ Discard<1/Card> | UnlessPayer$ You",
    json!({"Pay": {"cost": [{"Discard": {}}], "unpaid": [{"GainLife": {"amount": 2}}]}})
)]
#[case::you_unless_sacrifice(
    "GainLife | LifeAmount$ 2 | UnlessCost$ Sac<1/Creature> | UnlessPayer$ You",
    json!({"Pay": {"cost": [{"Sacrifice": {"filter": [{"Type": "Creature"}]}}],
        "unpaid": [{"GainLife": {"amount": 2}}]}})
)]
fn an_unless_cost_of_items_is_paid_by_the_payer(#[case] line: &str, #[case] node: Value) {
    let script = spell(line);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let effects = &found[0]["Spell"]["effects"];
    assert_eq!(effects[0], node, "{effects}");
    normal(found[0].clone());
}

#[rstest]
#[case::energy_x(
    "GainLife | LifeAmount$ 2 | UnlessCost$ PayEnergy<X> | UnlessPayer$ You",
    "amount:X"
)]
#[case::life_x(
    "GainLife | LifeAmount$ 2 | UnlessCost$ PayLife<X> | UnlessPayer$ You",
    "amount:X"
)]
#[case::mana_x(
    "GainLife | LifeAmount$ 2 | UnlessCost$ X | UnlessPayer$ You",
    "amount:X"
)]
#[case::their_chooser_without_target(
    "ChangeZone | DefinedPlayer$ You | Chooser$ Targeted | Origin$ Hand | Destination$ Exile | ChangeType$ Card",
    "key:ChangeZone.Choose.Chooser"
)]
#[case::owner_filter_of_another_player(
    "ChangeZone | ValidTgts$ Opponent | DefinedPlayer$ Targeted | Origin$ Graveyard | Destination$ Exile | ChangeType$ Card.YouOwn",
    "key:ChangeZone.Choose.DefinedPlayer"
)]
fn a_shape_beyond_the_node_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    let script = spell(line);
    assert_eq!(abilities(&script), Vec::<Value>::new(), "{script}");
    assert!(
        gaps(&script).iter().any(|found| found == gap),
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::you_choose_from_their_hand(
    "ChangeZone | ValidTgts$ Opponent | DefinedPlayer$ Targeted | Origin$ Hand | Destination$ Exile | Chooser$ You | ChangeType$ Card.nonLand | ChangeNum$ 1 | Mandatory$ True",
    json!({"targets": [{"what": "Opponent"}], "effects": [
        {"Choose": {"from": {"Each": [{"Not": {"Type": "Land"}}, {"In": "Hand"}, {"OwnedBy": {"Target": 0}}]}, "as": "picked"}},
        {"Exile": {"what": {"Bound": "picked"}}}]})
)]
#[case::they_choose_from_their_hand(
    "ChangeZone | ValidTgts$ Opponent | DefinedPlayer$ Targeted | Origin$ Hand | Destination$ Exile | Chooser$ Targeted | ChangeType$ Card | ChangeNum$ 1 | Mandatory$ True",
    json!({"targets": [{"what": "Opponent"}], "effects": [
        {"Choose": {"who": {"Target": 0}, "from": {"Each": [{"In": "Hand"}, {"OwnedBy": {"Target": 0}}]}, "as": "picked"}},
        {"Exile": {"what": {"Bound": "picked"}}}]})
)]
#[case::your_own_hand(
    "ChangeZone | DefinedPlayer$ You | Origin$ Hand | Destination$ Exile | ChangeType$ Card | ChangeNum$ 1 | Mandatory$ True",
    json!({"effects": [
        {"Choose": {"from": {"Each": [{"In": "Hand"}, {"OwnedBy": "You"}]}, "as": "picked"}},
        {"Exile": {"what": {"Bound": "picked"}}}]})
)]
fn a_defined_player_owns_the_cards_a_chooser_picks(#[case] line: &str, #[case] body: Value) {
    let script = spell(line);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert_eq!(normal(found[0].clone()), normal(json!({ "Spell": body })));
}

#[rstest]
#[case::top_without_a_position("", json!({"Library": {"place": "Top"}}))]
#[case::top_by_position(" | LibraryPosition$ 0", json!({"Library": {"place": "Top"}}))]
#[case::bottom(" | LibraryPosition$ -1", json!({"Library": {"place": "Bottom"}}))]
#[case::second_from_the_top(" | LibraryPosition$ 1", json!({"Library": {"place": {"Below": 1}}}))]
#[case::third_from_the_top(" | LibraryPosition$ 2", json!({"Library": {"place": {"Below": 2}}}))]
#[case::shuffled_in(" | Shuffle$ True", json!({"Library": {"place": "Shuffled"}}))]
#[case::shuffled_in_whatever_the_position(
    " | LibraryPosition$ 0 | Shuffle$ True",
    json!({"Library": {"place": "Shuffled"}})
)]
fn a_card_put_into_a_library_goes_where_the_script_says(#[case] extra: &str, #[case] to: Value) {
    let script = spell(&format!(
        "ChangeZone | ValidTgts$ Creature | Origin$ Battlefield | Destination$ Library{extra}"
    ));
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let want = json!({"Spell": {"targets": [{"what": {"Object": [{"Type": "Creature"}]}}],
        "effects": [{"Move": {"what": {"Target": 0}, "to": to}}]}});
    assert_eq!(normal(found[0].clone()), normal(want));
}

#[rstest]
#[case::fractional(" | LibraryPosition$ 1.5")]
#[case::counted(" | LibraryPosition$ X")]
fn a_library_position_beyond_top_and_bottom_stays_a_gap(#[case] extra: &str) {
    let script = spell(&format!(
        "ChangeZone | ValidTgts$ Creature | Origin$ Battlefield | Destination$ Library{extra}"
    ));
    assert_eq!(abilities(&script), Vec::<Value>::new(), "{script}");
    assert!(
        gaps(&script)
            .iter()
            .any(|found| found == "key:ChangeZone.LibraryPosition"),
        "{:?}",
        gaps(&script)
    );
}

const BOTTOM: &str = r#"{"Library": {"place": "Bottom", "order": "Chosen"}}"#;

#[rstest]
#[case::all_matching(
    "Dig | DigNum$ 3 | ChangeNum$ All | ChangeValid$ Creature | DestinationZone$ Hand",
    json!([{"LookAtTop": {"count": 3, "as": "top"}},
        {"Move": {"what": {"Each": [{"In": "Library"}, {"Is": {"Bound": "top"}}, {"Type": "Creature"}]}, "to": "Hand"}},
        {"Move": {"what": {"Bound": "top"}, "to": serde_json::from_str::<Value>(BOTTOM).unwrap()}}])
)]
#[case::all_revealed(
    "Dig | DigNum$ 3 | ChangeNum$ All | Reveal$ True | DestinationZone$ Graveyard",
    json!([{"LookAtTop": {"count": 3, "as": "top"}},
        {"Reveal": {"what": {"Bound": "top"}}},
        {"Move": {"what": {"Bound": "top"}, "to": "Graveyard"}}])
)]
#[case::any_number(
    "Dig | DigNum$ 3 | ChangeNum$ Any | ChangeValid$ Land | DestinationZone$ Hand",
    json!([{"LookAtTop": {"count": 3, "as": "top"}},
        {"Choose": {"from": {"Each": [{"In": "Library"}, {"Is": {"Bound": "top"}}, {"Type": "Land"}]},
            "count": {"min": 0, "max": null}, "as": "kept2"}},
        {"Move": {"what": {"Bound": "kept2"}, "to": "Hand"}},
        {"Move": {"what": {"Bound": "top"}, "to": serde_json::from_str::<Value>(BOTTOM).unwrap()}}])
)]
fn a_dig_takes_all_or_any_number_of_the_cards_it_looks_at(
    #[case] line: &str,
    #[case] effects: Value,
) {
    let script = spell(line);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert_eq!(
        normal(found[0].clone()),
        normal(json!({"Spell": {"effects": effects}}))
    );
}

#[rstest]
#[case::optional_all("Dig | DigNum$ 3 | ChangeNum$ All | Optional$ True | DestinationZone$ Hand")]
#[case::counted("Dig | DigNum$ 3 | ChangeNum$ X | DestinationZone$ Hand")]
fn a_dig_that_takes_a_count_beyond_the_node_stays_a_gap(#[case] line: &str) {
    let script = spell(line);
    assert_eq!(abilities(&script), Vec::<Value>::new(), "{script}");
    assert!(
        gaps(&script)
            .iter()
            .any(|found| found == "key:Dig.ChangeNum"),
        "{:?}",
        gaps(&script)
    );
}

fn creature(trigger: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Elf\nT:{trigger} | Execute$ Draw1\nSVar:Draw1:DB$ Draw | NumCards$ 1\n"
    )
}

#[rstest]
#[case::targets_this(
    "Mode$ SpellCast | ValidCard$ Card | ValidActivatingPlayer$ You | TargetsValid$ Card.Self",
    json!({"SpellCast": {"by": "You", "what": [{"Targeting": ["This"]}]}})
)]
#[case::targets_your_creature(
    "Mode$ SpellCast | ValidCard$ Instant,Sorcery | ValidActivatingPlayer$ Player | TargetsValid$ Creature.YouCtrl+inZoneBattlefield",
    json!({"SpellCast": {"by": "EachPlayer", "what": [{"AnyOf": [{"Type": "Instant"}, {"Type": "Sorcery"}]},
        {"Targeting": [{"Type": "Creature"}, {"ControlledBy": "You"}, {"In": "Battlefield"}]}]}})
)]
#[case::no_spell_filter(
    "Mode$ SpellCast | ValidActivatingPlayer$ You | TargetsValid$ Card.Self",
    json!({"SpellCast": {"by": "You", "what": [{"Targeting": ["This"]}]}})
)]
fn a_spell_cast_trigger_names_what_the_spell_targets(#[case] trigger: &str, #[case] when: Value) {
    let script = creature(trigger);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let want = json!({"Triggered": {"when": when, "effects": [{"Draw": {}}]}});
    assert_eq!(normal(found[0].clone()), normal(want));
}

#[rstest]
#[case::a_player_among_the_targets(
    "Mode$ SpellCast | ValidCard$ Card | ValidActivatingPlayer$ You | TargetsValid$ You,Creature.YouCtrl+inZoneBattlefield"
)]
fn a_target_beyond_cards_stays_a_gap(#[case] trigger: &str) {
    let script = creature(trigger);
    assert_eq!(abilities(&script), Vec::<Value>::new(), "{script}");
    assert!(
        gaps(&script)
            .iter()
            .any(|found| found.starts_with("defined:") || found.starts_with("valid")),
        "{:?}",
        gaps(&script)
    );
}

const TAP: &str = "SVar:ETBTapped:DB$ Tap | ETB$ True | Defined$ ReplacedCard\n";
const EXILE: &str = "SVar:Exile:DB$ ChangeZone | Hidden$ True | Origin$ All | Destination$ Exile | Defined$ ReplacedCard\n";

fn enchantment(line: &str, svar: &str) -> String {
    format!("Name:T\nTypes:Enchantment\nR:{line} | Description$ x\n{svar}")
}

#[rstest]
#[case::opponents_nonbasic_lands_tapped(
    "Event$ Moved | ValidCard$ Land.OppCtrl+nonBasic | Destination$ Battlefield | ReplaceWith$ ETBTapped | ReplacementResult$ Updated | ActiveZones$ Battlefield",
    TAP,
    json!({"Replacement": {"when": {"Enters": {"what": [{"Type": "Land"}, {"ControlledBy": "Opponents"}, {"Not": {"Supertype": "Basic"}}]}}, "by": "Tapped"}})
)]
#[case::every_creature_tapped(
    "Event$ Moved | ValidCard$ Creature | Destination$ Battlefield | ReplaceWith$ ETBTapped | ReplacementResult$ Updated | ActiveZones$ Battlefield",
    TAP,
    json!({"Replacement": {"when": {"Enters": {"what": [{"Type": "Creature"}]}}, "by": "Tapped"}})
)]
#[case::exile_instead_of_dying(
    "Event$ Moved | ActiveZones$ Battlefield | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Creature.OppCtrl | ReplaceWith$ Exile",
    EXILE,
    json!({"Replacement": {"when": {"ZoneChange": {"what": [{"Type": "Creature"}, {"ControlledBy": "Opponents"}], "from": "Battlefield", "to": "Graveyard"}}, "by": {"MoveTo": "Exile"}}})
)]
#[case::exile_instead_from_anywhere(
    "Event$ Moved | ActiveZones$ Battlefield | Destination$ Graveyard | ValidCard$ Card | ReplaceWith$ Exile",
    EXILE,
    json!({"Replacement": {"when": {"ZoneChange": {"what": [], "to": "Graveyard"}}, "by": {"MoveTo": "Exile"}}})
)]
fn a_moved_replacement_of_other_cards_acts_while_its_source_is_on_the_battlefield(
    #[case] line: &str,
    #[case] svar: &str,
    #[case] want: Value,
) {
    let script = enchantment(line, svar);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert_eq!(normal(found[0].clone()), normal(want));
}

#[rstest]
#[case::in_hand(
    "Event$ Moved | ValidCard$ Creature | Destination$ Battlefield | ReplaceWith$ ETBTapped | ReplacementResult$ Updated | ActiveZones$ Hand",
    TAP,
    "key:Replacement.Moved.ActiveZones"
)]
#[case::no_zone(
    "Event$ Moved | ValidCard$ Creature | Destination$ Battlefield | ReplaceWith$ ETBTapped | ReplacementResult$ Updated",
    TAP,
    "key:Replacement.Moved.ActiveZones"
)]
#[case::exile_elsewhere(
    "Event$ Moved | ActiveZones$ Battlefield | Origin$ Hand | Destination$ Graveyard | ValidCard$ Card | ReplaceWith$ Exile",
    EXILE,
    "key:Replacement.Moved.Origin"
)]
#[case::untapped_instead(
    "Event$ Moved | ValidCard$ Land.YouCtrl | Destination$ Battlefield | ReplaceWith$ ETBTapped | ReplacementResult$ Updated | ActiveZones$ Battlefield",
    "SVar:ETBTapped:DB$ Untap | ETB$ True | Defined$ ReplacedCard\n",
    "replacement:Moved-not-an-entering-effect"
)]
fn a_moved_replacement_beyond_the_shape_stays_a_gap(
    #[case] line: &str,
    #[case] svar: &str,
    #[case] gap: &str,
) {
    let script = enchantment(line, svar);
    assert_eq!(abilities(&script), Vec::<Value>::new(), "{script}");
    assert!(
        gaps(&script).iter().any(|found| found == gap),
        "{:?}",
        gaps(&script)
    );
}
