//! Goal: Forge parameters that name a pair, a group or a remembered number translate to the rules the card prints, and a near miss of each stays a gap.
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;

use mtg_cli::forge::script::Script;
use mtg_cli::translate::card::translate_script;
use mtg_cli::translate::ctx::World;
use rstest::rstest;

/// The abilities of the first face as text, and the gaps it left.
fn read(text: &str) -> (String, Vec<String>) {
    let out = translate_script(&World::new(&[], BTreeMap::new()), &Script::parse(text));
    let face = &out.faces[0];
    let gaps = face.gaps.iter().map(|gap| gap.0.clone()).collect();
    (serde_json::to_string(&face.abilities).unwrap(), gaps)
}

const FELL: &str = "Name:Fell\nManaCost:4 W\nTypes:Sorcery\nA:SP$ DestroyAll | ValidCards$ Creature.powerGTX | ValidTgts$ Creature | SpellDescription$ Destroy all creatures with power greater than target creature's power.\nSVar:X:Targeted$CardPower\nOracle:Destroy all creatures with power greater than target creature's power.\n";
const WILD: &str = "Name:Wild\nManaCost:3 R\nTypes:Sorcery\nA:SP$ Pump | ValidTgts$ Permanent.nonEnchantment | TargetMin$ 3 | TargetMax$ 3 | SubAbility$ DBR | SpellDescription$ Choose three target nonenchantment permanents. Destroy one of them at random.\nSVar:DBR:DB$ ChooseCard | Defined$ You | Amount$ 1 | AtRandom$ True | DefinedCards$ Targeted | SubAbility$ DBD\nSVar:DBD:DB$ Destroy | Defined$ ChosenCard\nOracle:Choose three target nonenchantment permanents. Destroy one of them at random.\n";
const DRAIN: &str = "Name:Drain\nManaCost:U U\nTypes:Instant\nA:SP$ Counter | TargetType$ Spell | RememberCounteredCMC$ True | ValidTgts$ Card | SubAbility$ DBDel | SpellDescription$ Counter target spell.\nSVar:DBDel:DB$ DelayedTrigger | Mode$ Phase | Phase$ Main1,Main2 | ValidPlayer$ You | Execute$ AddMana | RememberNumber$ True | SubAbility$ DBC\nSVar:DBC:DB$ Cleanup | ClearRemembered$ True\nSVar:AddMana:DB$ Mana | Produced$ C | Amount$ X\nSVar:X:Count$TriggerRememberAmount\nOracle:Counter target spell. At the beginning of your next main phase, add an amount of {C} equal to that spell's mana value.\n";
const TRUCE: &str = "Name:Truce\nManaCost:1 W\nTypes:Instant\nA:SP$ Pump | Defined$ You & Valid Permanent.YouCtrl | KW$ Hexproof | SpellDescription$ You and permanents you control gain hexproof until end of turn.\nOracle:You and permanents you control gain hexproof until end of turn.\n";
const BARK: &str = "Name:Bark\nManaCost:2 B\nTypes:Creature Treefolk\nPT:0/4\nT:Mode$ Phase | Phase$ End of Turn | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ TrigDraw | TriggerDescription$ At the beginning of your end step, draw a card and blight 1.\nSVar:TrigDraw:DB$ Draw | NumCards$ 1 | Defined$ You | SubAbility$ DBBlight\nSVar:DBBlight:DB$ Blight | Num$ 1 | Defined$ You\nOracle:At the beginning of your end step, draw a card and blight 1.\n";
const RAVENER: &str = "Name:Rav\nManaCost:3 U\nTypes:Creature Tyranid\nPT:0/0\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigTarget | TriggerDescription$ When Rav enters, target creature attacks target opponent this turn if able.\nSVar:TrigTarget:DB$ Pump | ValidTgts$ Creature | SubAbility$ DBEffect\nSVar:DBEffect:DB$ Effect | ValidTgts$ Opponent | RememberObjects$ ParentTarget & Targeted | StaticAbilities$ MustAttack | ExileOnMoved$ Battlefield\nSVar:MustAttack:Mode$ MustAttack | ValidCreature$ Creature.IsRemembered | MustAttack$ Player.IsRemembered | Description$ This creature attacks this player this turn if able.\nOracle:When Rav enters, target creature attacks target opponent this turn if able.\n";
const ARCH: &str = "Name:Arch\nManaCost:3 B B\nTypes:Creature Demon\nPT:5/4\nT:Mode$ Phase | Phase$ End of Turn | ValidPlayer$ Opponent | TriggerZones$ Battlefield | Execute$ TrigChoose | TriggerDescription$ At the beginning of each opponent's end step, that player chooses up to two creatures they control, then sacrifices the rest.\nSVar:TrigChoose:DB$ ChooseCard | Defined$ TriggeredPlayer | MinAmount$ 0 | Amount$ 2 | TargetControls$ True | Choices$ Creature | Reveal$ True | SubAbility$ DBS\nSVar:DBS:DB$ SacrificeAll | ValidCards$ Creature.nonChosenCard | Controller$ TriggeredPlayer | SubAbility$ DBC\nSVar:DBC:DB$ Cleanup | ClearChosenCard$ True\nOracle:At the beginning of each opponent's end step, that player chooses up to two creatures they control, then sacrifices the rest.\n";

#[rstest]
#[case::power_greater_than_the_target(FELL, &[r#""Compare":{"op":"Gt","stat":"Power","to":{"Stat":{"of":{"Target":0},"stat":"Power"}}}"#])]
#[case::one_of_three_targets_at_random(WILD, &[r#""ChooseAtRandom":{"as":"card","from":{"Target":0}}"#, r#""max":3,"min":3"#])]
#[case::the_mana_value_kept_for_the_next_main_phase(DRAIN, &[r#""Let":{"amount""#, r#""times":{"Bound":"number"}"#, r#""step":"Main2""#])]
#[case::the_player_and_the_permanents_alike(TRUCE, &[r#""PlayersHave":{"keyword":"Hexproof","who":"You"}"#, r#""ControlledBy":"You""#])]
#[case::blight_puts_a_minus_counter_on_a_creature_picked(BARK, &[r#""counter":"-1/-1""#, r#""Choose":{"as":"blighted""#])]
#[case::a_creature_and_a_player_as_a_pair(RAVENER, &[r#""MustAttackPlayers":{"players":{"Target":1},"who":{"Target":0}}"#])]
#[case::the_player_chosen_sacrifices_their_rest(ARCH, &[r#""ControlledBy":"Triggering""#, r#""who":"Triggering""#])]
fn the_parameter_reads_as_the_card_prints_it(#[case] script: &str, #[case] parts: &[&str]) {
    let (abilities, gaps) = read(script);
    assert!(gaps.is_empty(), "{gaps:?}");
    for part in parts {
        assert!(abilities.contains(part), "{part} in {abilities}");
    }
}

#[rstest]
#[case::a_number_that_is_not_the_target(
    FELL,
    "X:Targeted$CardPower",
    "X:Count$Valid Creature.YouCtrl",
    "ValidTgts-unread"
)]
#[case::a_pool_that_is_not_the_targets(
    WILD,
    "DefinedCards$ Targeted",
    "DefinedCards$ Remembered",
    "ChooseCard.DefinedCards"
)]
#[case::no_number_remembered(DRAIN, "RememberCounteredCMC$ True | ", "", "RememberNumber")]
#[case::another_creature_for_the_player(
    RAVENER,
    "Player.IsRemembered",
    "Player.Opponent",
    "ParentTarget & Targeted"
)]
#[case::another_player_sacrifices(
    ARCH,
    "Controller$ TriggeredPlayer",
    "Controller$ You",
    "SacrificeAll.Controller"
)]
fn a_near_miss_stays_a_gap(
    #[case] script: &str,
    #[case] from: &str,
    #[case] to: &str,
    #[case] gap: &str,
) {
    assert!(script.contains(from), "{from}");
    let (_, gaps) = read(&script.replace(from, to));
    assert!(
        gaps.iter().any(|found| found.contains(gap)),
        "{gap} in {gaps:?}"
    );
}
