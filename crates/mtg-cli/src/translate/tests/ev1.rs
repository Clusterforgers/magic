//! Goal: the event words and carry words of a line are table rows: a delayed trigger or effect takes the object of the event
//! under a name, `Forget*` keys empty the slot or stay gaps, and a word that needs the object one move on stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn creature(body: &str) -> String {
    format!("Name:T\nTypes:Creature Elf\nPT:1/1\n{body}\n")
}

#[rstest]
#[case::attacker_lki(
    "Mode$ AttackerBlocked | ValidCard$ Creature | ValidBlocker$ Card.Self",
    "Phase | Phase$ EndCombat | ValidPlayer$ Player | RememberObjects$ TriggeredAttackerLKICopy",
    "Triggering"
)]
#[case::attacker(
    "Mode$ Attacks | ValidCard$ Card.Self",
    "Phase | Phase$ EndCombat | RememberObjects$ TriggeredAttacker",
    "This"
)]
fn a_delayed_step_names_the_object_of_the_event(
    #[case] trigger: &str,
    #[case] delayed: &str,
    #[case] named: &str,
) {
    let script = creature(&format!(
        "T:{trigger} | Execute$ Delay\nSVar:Delay:DB$ DelayedTrigger | Mode$ {delayed} | Execute$ Back\nSVar:Back:DB$ ChangeZone | Origin$ Battlefield | Destination$ Hand | Defined$ DelayTriggerRememberedLKI"
    ));
    assert_eq!(gaps(&script), Vec::<String>::new());
    let effects = &abilities(&script)[0]["Triggered"]["effects"];
    assert_eq!(effects[0], json!({"Bind": {"as": "event", "what": named}}));
    assert_eq!(
        effects[1]["DelayedTrigger"]["effects"][0]["Move"]["what"],
        json!({"Bound": "event"})
    );
}

#[rstest]
#[case::card_after_a_move(
    "Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self",
    "TriggeredCard",
    "key:RememberObjects=TriggeredCard-after-a-move"
)]
fn an_object_that_moved_is_not_named_for_a_later_trigger(
    #[case] trigger: &str,
    #[case] word: &str,
    #[case] gap: &str,
) {
    let script = creature(&format!(
        "T:{trigger} | Execute$ Delay\nSVar:Delay:DB$ DelayedTrigger | Mode$ Phase | Phase$ End of Turn | Execute$ Back | RememberObjects$ {word}\nSVar:Back:DB$ ChangeZone | Origin$ Graveyard | Destination$ Battlefield | Defined$ DelayTriggerRememberedLKI"
    ));
    assert_eq!(gaps(&script), vec![gap.to_owned()]);
}

#[rstest]
#[case::controller_of_the_hit_creature(
    "Mode$ DamageDone | ValidSource$ Card.Self | ValidTarget$ Creature",
    "TriggeredTargetController",
    json!({"ControllerOf": "Triggering"})
)]
#[case::controller_of_the_attacker(
    "Mode$ Attacks | ValidCard$ Creature",
    "TriggeredAttackerController",
    json!({"ControllerOf": "Triggering"})
)]
fn a_controller_word_is_the_controller_of_the_event_object(
    #[case] trigger: &str,
    #[case] word: &str,
    #[case] who: Value,
) {
    let script = creature(&format!(
        "T:{trigger} | Execute$ Hit\nSVar:Hit:DB$ DealDamage | Defined$ {word} | NumDmg$ 3"
    ));
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(
        abilities(&script)[0]["Triggered"]["effects"][0]["DealDamage"]["to"],
        who
    );
}

#[rstest]
#[case::other_remembered("ForgetOtherRemembered", Vec::new())]
#[case::other_targets("ForgetOtherTargets", Vec::new())]
#[case::changed_on_a_name("ForgetChanged", Vec::new())]
fn a_forget_key_empties_the_slot_or_stays_a_gap(#[case] key: &str, #[case] want: Vec<String>) {
    let script = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Battlefield | Destination$ Exile | ValidTgts$ Creature | RememberChanged$ True | SubAbility$ Back\nSVar:Back:DB$ ChangeZone | Defined$ Remembered | Origin$ Exile | Destination$ Battlefield | {key}$ True\n"
    );
    assert_eq!(gaps(&script), want);
}
