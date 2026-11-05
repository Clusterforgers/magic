//! Goal: "you may sacrifice X; if you do, ... that creature's value ..." is written as a sacrifice that names what it took
//! and effects that read the name, whatever the effect does with the value, and a pass over players that asks whose
//! cards a step moved stays a gap.
use rstest::rstest;

use super::{abilities, gaps};

fn attack_trigger(effect: &str, amount: &str) -> String {
    format!(
        "Name:T\nManaCost:2\nTypes:Creature Wolf\nPT:3/2\nT:Mode$ Attacks | ValidCard$ Card.Self | Execute$ Go | TriggerDescription$ x\nSVar:Go:AB$ {effect} | Cost$ Sac<1/Creature.Other/another creature>\nSVar:X:Sacrificed${amount}\n"
    )
}

#[rstest]
#[case::counters(
    "PutCounter | Defined$ Self | CounterType$ P1P1 | CounterNum$ X",
    "CardPower"
)]
#[case::cards("Draw | Defined$ You | NumCards$ X", "CardManaCost")]
#[case::life("GainLife | Defined$ You | LifeAmount$ X", "CardToughness")]
fn a_sacrifice_paid_in_a_sub_ability_names_what_it_took(
    #[case] effect: &str,
    #[case] amount: &str,
) {
    let text = attack_trigger(effect, amount);
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = abilities(&text)[0].to_string();
    assert!(
        found.contains(r#""Sacrifice":{"as":"sacrificed""#),
        "{found}"
    );
    assert!(found.contains(r#"{"Bound":"sacrificed"}"#), "{found}");
    assert!(found.contains(r#""Bound":"sacrificed~moved""#), "{found}");
    assert!(
        !found.contains("Paid") && !found.contains("Pay\""),
        "{found}"
    );
}

#[test]
fn a_cost_that_sacrifices_the_source_reads_the_source_as_it_left() {
    let text = "Name:T\nManaCost:2\nTypes:Creature Imp\nPT:3/1\nA:AB$ DealDamage | Cost$ R Sac<1/CARDNAME> | ValidTgts$ Creature | NumDmg$ X | SpellDescription$ x\nSVar:X:Sacrificed$CardPower\n";
    assert_eq!(gaps(text), Vec::<String>::new());
    let found = abilities(text)[0].to_string();
    assert!(
        found.contains(r#""amount":{"Stat":{"of":"This","stat":"Power"}}"#),
        "{found}"
    );
    assert!(!found.contains("Paid"), "{found}");
}

#[rstest]
#[case::mana_value("Discarded$CardManaCost", "ManaValue")]
#[case::power("Discarded$CardPower", "Power")]
fn a_discard_cost_names_the_card_it_took(#[case] amount: &str, #[case] stat: &str) {
    let text = format!(
        "Name:T\nManaCost:2\nTypes:Creature Imp\nPT:1/1\nA:AB$ Pump | Cost$ 1 B Discard<1/Creature> | NumAtt$ +X | NumDef$ +X | SpellDescription$ x\nSVar:X:{amount}\n"
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = abilities(&text)[0].to_string();
    assert!(
        found.contains(&format!(r#""Stat":{{"of":"Paid","stat":"{stat}"}}"#)),
        "{found}"
    );
}

#[test]
fn the_player_who_sacrifices_is_the_one_asked() {
    let text = "Name:T\nManaCost:2\nTypes:Enchantment\nT:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | Execute$ Sac | TriggerDescription$ x\nSVar:Sac:DB$ Sacrifice | Defined$ Player.Opponent | SacValid$ Creature | Optional$ True\n";
    assert_eq!(gaps(text), Vec::<String>::new());
    let found = &abilities(text)[0]["Triggered"]["effects"][0]["May"];
    assert_eq!(found["who"], "Opponents", "{found}");
    assert!(found["effects"][0].get("Sacrifice").is_some(), "{found}");
}

#[test]
fn a_pass_over_players_never_asks_whose_cards_a_step_moved() {
    let text = "Name:T\nManaCost:2\nTypes:Enchantment\nT:Mode$ Phase | Phase$ End of Turn | ValidPlayer$ You | Execute$ Sac | TriggerDescription$ x\nSVar:Sac:DB$ Sacrifice | Optional$ True | SacValid$ Creature | RememberSacrificed$ True | SubAbility$ Each\nSVar:Each:DB$ RepeatEach | RepeatPlayers$ Player.Opponent | RepeatSubAbility$ Pay\nSVar:Pay:DB$ Sacrifice | Defined$ Player.IsRemembered | SacValid$ Permanent | RememberSacrificed$ True | Optional$ True | SubAbility$ Lose\nSVar:Lose:DB$ LoseLife | Defined$ Player.IsRemembered | LifeAmount$ 2 | ConditionCheckSVar$ Y | ConditionSVarCompare$ EQ0\nSVar:Y:Remembered$Valid Card.RememberedPlayerCtrl\n";
    assert!(
        gaps(text)
            .iter()
            .any(|gap| gap == "valid:RememberedPlayerCtrl-of-moved-cards"),
        "{:?}",
        gaps(text)
    );
}
