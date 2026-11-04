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
    "key:Unless.UnlessCost-X"
)]
#[case::each_player(
    "ChangeZone | DefinedPlayer$ Player | Origin$ Hand | Destination$ Exile | ChangeType$ Card | ChangeNum$ 2 | Mandatory$ True",
    "key:ChangeZone.Choose.DefinedPlayer"
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
#[case::third_from_top(" | LibraryPosition$ 2")]
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
#[case::last_known_information(
    "Event$ Moved | ActiveZones$ Battlefield | Origin$ Battlefield | Destination$ Graveyard | ValidLKI$ Creature.OppCtrl | ReplaceWith$ Exile",
    EXILE,
    "key:Replacement.Moved.ValidLKI"
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
