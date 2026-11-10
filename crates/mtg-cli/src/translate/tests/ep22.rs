//! Goal: the pieces of ep22 translate as the card prints them: a leave trigger printed "if it had counters on it" reads its counters as the intervening condition on the object of the event and keeps the rest of the filter, while one printed "with a counter on it" keeps the filter; other filters, a move into the battlefield and a list of alternatives stay as they were; a flip with no call reads heads as a win and tails as a loss, the caster flips for their spell, and the coin doubler of your flips is a rule; heads with a call, tails of several flips and other flippers stay gaps.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn spell(line: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ {line}\nSVar:Turn:DB$ AddTurn | Defined$ You | NumTurns$ X\nSVar:Draw:DB$ Draw\n"
    )
}

fn effects(script: &str) -> Value {
    abilities(script)
        .first()
        .and_then(|found| found.get("Spell"))
        .and_then(|body| body.get("effects"))
        .cloned()
        .unwrap_or_default()
}

fn heads() -> Value {
    json!({"FlipCoin": {"win": [{"ExtraTurn": {}}]}})
}

#[rstest]
#[case::heads_three_times(
    "FlipCoin | Amount$ 3 | NoCall$ True | HeadsSubAbility$ Turn",
    json!([heads(), heads(), heads()])
)]
#[case::heads_and_tails_once(
    "FlipCoin | NoCall$ True | HeadsSubAbility$ Turn | TailsSubAbility$ Draw",
    json!([{"FlipCoin": {"win": [{"ExtraTurn": {}}], "lose": [{"Draw": {}}]}}])
)]
fn ep22_flips_with_no_call_translate(#[case] line: &str, #[case] expected: Value) {
    let script = spell(line);
    assert_eq!(gaps(&script), Vec::<String>::new(), "{line}");
    assert_eq!(effects(&script), expected, "{line}");
}

#[rstest]
#[case::heads_with_a_call("FlipCoin | HeadsSubAbility$ Turn", "key:FlipCoin.HeadsSubAbility")]
#[case::tails_of_several_flips(
    "FlipCoin | Amount$ 2 | NoCall$ True | HeadsSubAbility$ Turn | TailsSubAbility$ Draw",
    "key:FlipCoin.Amount-with-more"
)]
#[case::another_flipper(
    "FlipCoin | Flipper$ Player | WinSubAbility$ Draw",
    "key:FlipCoin.Flipper=Player"
)]
fn ep22_other_flips_stay_gaps(#[case] line: &str, #[case] gap: &str) {
    let found = gaps(&spell(line));
    assert!(found.iter().any(|found| found == gap), "{found:?}");
}

#[test]
fn ep22_the_caster_flips_for_their_spell() {
    let script = "Name:T\nTypes:Enchantment\nT:Mode$ SpellCast | ValidCard$ Card | TriggerZones$ Battlefield | Execute$ Flip | TriggerDescription$ Whenever a player casts a spell, that player flips a coin. If they lose the flip, counter that spell.\nSVar:Flip:DB$ FlipCoin | LoseSubAbility$ Counter | Flipper$ TriggeredActivator\nSVar:Counter:DB$ Counter | Defined$ TriggeredSpellAbility\n";
    let found = triggered(script);
    assert_eq!(
        found["effects"][0]["FlipCoin"]["who"],
        json!("Triggering"),
        "{found}"
    );
}

#[rstest]
#[case::yours("Mode$ FlipCoinDoubler | ValidPlayer$ You", None)]
#[case::an_opponent_s(
    "Mode$ FlipCoinDoubler | ValidPlayer$ Opponent",
    Some("key:Static.FlipCoinDoubler.ValidPlayer")
)]
fn ep22_the_coin_doubler_translates(#[case] line: &str, #[case] gap: Option<&str>) {
    let script = format!(
        "Name:T\nTypes:Artifact\nS:{line} | Description$ If you would flip a coin, instead flip two coins and ignore one.\n"
    );
    match gap {
        None => assert_eq!(
            abilities(&script),
            vec![json!({"Rule": {"rule": {"FlipTwoCoins": "You"}}})]
        ),
        Some(gap) => assert!(gaps(&script).iter().any(|found| found == gap)),
    }
}

fn trigger(line: &str, printed: &str) -> String {
    format!(
        "Name:T\nTypes:Artifact\nT:{line} | TriggerZones$ Battlefield | Execute$ Draw | TriggerDescription$ {printed}\nSVar:Draw:DB$ Draw\n"
    )
}

fn triggered(script: &str) -> Value {
    abilities(script)
        .into_iter()
        .next()
        .and_then(|found| found.get("Triggered").cloned())
        .unwrap_or_default()
}

const LEAVES: &str = "Mode$ ChangesZone | Origin$ Battlefield | Destination$ Any | ValidCard$ Creature.YouCtrl+HasCounters";
const IF_HAD: &str = "Whenever a creature you control leaves the battlefield, if it had counters on it, draw a card.";
const WITH: &str =
    "Whenever a creature you control with a counter on it leaves the battlefield, draw a card.";

#[rstest]
#[case::printed_if(LEAVES, IF_HAD, false, true)]
#[case::printed_with(LEAVES, WITH, true, false)]
#[case::into_the_battlefield(
    "Mode$ ChangesZone | Destination$ Battlefield | ValidCard$ Creature.YouCtrl+HasCounters",
    IF_HAD,
    true,
    false
)]
#[case::alternatives(
    "Mode$ ChangesZone | Origin$ Battlefield | Destination$ Any | ValidCard$ Creature.YouCtrl,Artifact.YouCtrl+HasCounters",
    IF_HAD,
    true,
    false
)]
fn ep22_had_counters_is_the_intervening_condition(
    #[case] line: &str,
    #[case] printed: &str,
    #[case] in_filter: bool,
    #[case] as_if: bool,
) {
    let found = triggered(&trigger(line, printed));
    let when = found.get("when").map(Value::to_string).unwrap_or_default();
    assert_eq!(when.contains("HasAnyCounter"), in_filter, "{found}");
    let had = json!({"Matches": {"subject": "Triggering", "selector": ["HasAnyCounter"]}});
    assert_eq!(found.get("if") == Some(&had), as_if, "{found}");
}

fn static_line(line: &str) -> String {
    format!("Name:T\nTypes:Enchantment\nS:{line}\n")
}

const SAME: &str = "Description$ Creatures you control are artifacts in addition to their other types. The same is true for creature spells you control and creature cards you own that aren't on the battlefield.";

#[test]
fn ep22_every_zone_lists_the_zones() {
    let script = static_line(&format!(
        "Mode$ Continuous | Affected$ Creature.YouCtrl | AffectedZone$ All | AddType$ Artifact | {SAME}"
    ));
    assert_eq!(gaps(&script), Vec::<String>::new());
    let found = abilities(&script);
    let zones = json!({"AnyOf": [{"In": "Battlefield"}, {"In": "Stack"}, {"In": "Hand"}, {"In": "Library"}, {"In": "Graveyard"}, {"In": "Exile"}, {"In": "Command"}]});
    assert_eq!(
        found[0]["Static"]["affected"]["Each"],
        json!([{"Type": "Creature"}, {"ControlledBy": "You"}, zones])
    );
}

#[rstest]
#[case::another_wording(
    "Mode$ Continuous | Affected$ Land.YouCtrl | AffectedZone$ All | AddType$ Desert | Description$ Lands you control and land cards you own that aren't on the battlefield are Deserts in addition to their other types."
)]
#[case::not_yours(&format!(
    "Mode$ Continuous | Affected$ Creature | AffectedZone$ All | AddType$ Artifact | {SAME}"
))]
fn ep22_other_zone_lists_stay_gaps(#[case] line: &str) {
    let found = gaps(&static_line(line));
    assert!(
        found
            .iter()
            .any(|gap| gap == "key:Static.Continuous.AffectedZone=All"),
        "{found:?}"
    );
}

#[rstest]
#[case::colored_only("Color$ W U B R G | IgnoreGeneric$ True", None)]
#[case::of_generic("IgnoreGeneric$ True", Some("key:Static.CostModifier.IgnoreGeneric"))]
fn ep22_a_colored_reduction_translates(#[case] keys: &str, #[case] gap: Option<&str>) {
    let script = static_line(&format!(
        "Mode$ ReduceCost | ValidCard$ Creature | Type$ Spell | Activator$ You | Amount$ 1 | {keys}"
    ));
    match gap {
        None => assert_eq!(
            abilities(&script)[0]["CostModifier"]["mana"],
            json!("{W}{U}{B}{R}{G}")
        ),
        Some(gap) => assert!(gaps(&script).iter().any(|found| found == gap)),
    }
}
