//! Goal: round 6 trigger faults stay closed: a printed "you may" over a single step without a decider in the script asks, a printed "target creature you control" the script leaves open is a gap, and a printed "it deals" of another object the script deals from the card is a gap.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a creature with the given lines as compact JSON, and its gaps.
fn card(lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:Creature Ooze\nPT:1/1\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

const UPKEEP: &str =
    "T:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ Trig";

#[rstest]
#[case::printed_may("you may put a +1/+1 counter on CARDNAME.", "", true)]
#[case::decider_kept(
    "you may put a +1/+1 counter on CARDNAME.",
    " | OptionalDecider$ You",
    true
)]
#[case::printed_forced("put a +1/+1 counter on CARDNAME.", "", false)]
// A may that is not the verb of the step is not this shape.
#[case::other_verb("you may draw a card.", "", false)]
// A tap or untap chooses its state anyway (fix pass 8), and a may after an earlier sentence covers only that sentence.
#[case::tap_or_untap("you may tap or untap CARDNAME.", "", false)]
#[case::later_sentence(
    "scry 1. If you control a land, you may put a +1/+1 counter on CARDNAME.",
    "",
    false
)]
fn r6a_a_printed_may_over_one_step_asks(
    #[case] printed: &str,
    #[case] decider: &str,
    #[case] asks: bool,
) {
    let (json, found) = card(&format!(
        "{UPKEEP}{decider} | TriggerDescription$ At the beginning of your upkeep, {printed}\nSVar:Trig:DB$ PutCounter | Defined$ Self | CounterType$ P1P1 | CounterNum$ 1"
    ));
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(json.contains("\"May\""), asks, "{json}");
}

const ENTERS: &str = "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ Trig";

#[rstest]
#[case::any_creature("Creature", true)]
#[case::own_creature("Creature.YouCtrl", false)]
fn r6a_a_printed_own_target_left_open_is_a_gap(#[case] valid: &str, #[case] gap: bool) {
    let (_, found) = card(&format!(
        "{ENTERS} | TriggerDescription$ When CARDNAME enters, target creature you control gets +2/+2 until end of turn.\nSVar:Trig:DB$ Pump | ValidTgts$ {valid} | NumAtt$ +2 | NumDef$ +2"
    ));
    assert_eq!(
        found
            .iter()
            .any(|g| g == "trigger:printed-own-target-unfiltered"),
        gap,
        "{found:?}"
    );
}

const DRAGON: &str = "T:Mode$ Attacks | ValidCard$ Creature.Other+YouCtrl | TriggerZones$ Battlefield | Execute$ Trig";

#[rstest]
#[case::from_the_card("", true)]
#[case::from_the_attacker(" | DamageSource$ TriggeredAttacker", false)]
// The card itself: "Whenever CARDNAME attacks, it deals" is the card dealing.
#[case::the_card_attacks(" | DamageSource$ Self", false)]
fn r6a_a_printed_it_deals_from_the_card_is_a_gap(#[case] source: &str, #[case] gap: bool) {
    let subject = if source.contains("Self") {
        "CARDNAME"
    } else {
        "another creature you control"
    };
    let source = source.replace(" | DamageSource$ Self", "");
    let (_, found) = card(&format!(
        "{DRAGON} | TriggerDescription$ Whenever {subject} attacks, it deals 2 damage to any target.\nSVar:Trig:DB$ DealDamage | ValidTgts$ Any | NumDmg$ 2{source}"
    ));
    assert_eq!(
        found
            .iter()
            .any(|g| g == "trigger:printed-it-deals-from-host"),
        gap,
        "{found:?}"
    );
}
