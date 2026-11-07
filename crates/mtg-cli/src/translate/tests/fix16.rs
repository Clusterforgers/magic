//! Goal: `UntapUpTo$` lets the player untap fewer only when Oracle says "up to" (Rising Waters untaps exactly one), and
//! the Crowns' "enchanted creature and other creatures that share a creature type with it" reads as the host and the
//! creatures sharing a subtype with it; a near-miss of the Crown phrase stays a gap; a script that says less than Oracle
//! (or its own printed text) follows Oracle or stays a gap, and where the two agree the old reading stays.
use rstest::rstest;
use serde_json::Value;

use super::{abilities, gaps};

fn read(text: &str) -> String {
    assert!(gaps(text).is_empty(), "{:?}", gaps(text));
    Value::Array(abilities(text)).to_string()
}

const UNTAP: &str =
    "SVar:TrigUntap:DB$ Untap | UntapUpTo$ True | UntapType$ Land.YouCtrl | Amount$ 1";

#[rstest]
#[case::a_land("you untap a land you control.", false)]
#[case::up_to("Untap up to one land you control.", true)]
#[case::player_up_to("that player untaps up to one land.", true)]
fn fix16_untap_up_to_follows_oracle(#[case] oracle: &str, #[case] fewer: bool) {
    let text = format!(
        "Name:T\nManaCost:U\nTypes:Enchantment\nT:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ TrigUntap | TriggerDescription$ x\n{UNTAP}\nOracle:{oracle}\n"
    );
    assert_eq!(read(&text).contains(r#""min":0"#), fewer, "{}", read(&text));
}

const CROWN: &str =
    "AttachedBy Sacrificed & Valid Creature.sharesCreatureTypeWith AttachedBy Sacrificed";

#[rstest]
#[case::crown(CROWN, true)]
#[case::other_word(
    "AttachedBy Sacrificed & Valid Creature.sharesColorWith AttachedBy Sacrificed",
    false
)]
fn fix16_a_crown_names_its_host_and_its_kin(#[case] defined: &str, #[case] read_whole: bool) {
    let text = format!(
        "Name:T\nManaCost:W\nTypes:Enchantment Aura\nK:Enchant:Creature\nA:AB$ Pump | Cost$ Sac<1/CARDNAME> | Defined$ {defined} | KW$ Flying | SpellDescription$ x\nOracle:Enchant creature\nx\n"
    );
    assert_eq!(gaps(&text).is_empty(), read_whole, "{:?}", gaps(&text));
    if read_whole {
        let expected = r#"{"Each":[{"Type":"Creature"},{"AnyOf":[{"Is":"Attached"},{"SharesSubtype":"Attached"}]}]}"#;
        assert!(read(&text).contains(expected), "{}", read(&text));
    }
}

const RADIANT: &str = "Name:T\nManaCost:G\nTypes:Creature Spirit\nPT:2/3\nS:Mode$ Continuous | AffectedDefined$ Self | AddPower$ 1 | AddToughness$ 2 | IsPresent$ Creature.OppCtrl+Black | Description$ CARDNAME gets +1/+2 as long as an opponent controls a black {kind}.\nOracle:x\n";
const DEEPWAY: &str = "Name:T\nManaCost:W\nTypes:Creature Merfolk\nPT:2/2\nS:Mode$ Continuous | Affected$ Creature.Elf+YouCtrl | AddPower$ 1 | CheckSVar$ RaidTest | SVarCompare$ GE3 | Description$ As long as you attacked with three or more {kind} this turn, Merfolk you control get +1/+0.\nSVar:RaidTest:Count$AttackersDeclared\nOracle:x\n";
const STIR: &str = "Name:T\nManaCost:X B\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Graveyard | Destination$ Battlefield | ValidTgts$ {valid} | SpellDescription$ Return target creature card with mana value X or less from your graveyard to the battlefield.\nSVar:X:Count$xPaid\nOracle:Return target creature card with mana value X or less from your graveyard to the battlefield.\n";
const HANA: &str = "Name:T\nManaCost:G\nTypes:Creature Spirit\nPT:1/1\nA:AB$ ChangeZone | Cost$ 1 G Sac<1/CARDNAME> | ValidTgts$ Creature | Origin$ Graveyard | Destination$ Hand | SpellDescription$ Return target creature card from {whose} to your hand.\nOracle:{1}{G}, Sacrifice ~: Return target creature card from {whose} to your hand.\n";
const RIDER: &str = "Name:T\nManaCost:5 B\nTypes:Creature Spirit\nPT:3/7\nA:AB$ LoseLife | Cost$ 1 B T | ValidTgts$ Player | LifeAmount$ 3 | SpellDescription$ Target player loses 3 life.\nOracle:{1}{B}, {T}: Target {who} loses 3 life.\n";
const SLIVER: &str = "Name:T\nManaCost:2 B\nTypes:Creature Sliver\nPT:2/2\nA:AB$ Discard | Cost$ 2 Sac<1/CARDNAME> | ValidTgts$ Player | NumCards$ 1 | Mode$ Random | SpellDescription$ Target player discards a card at random.{rest}\nOracle:x\n";
const PIRATES: &str = "Name:T\nManaCost:2 U\nTypes:Creature Human Pirate\nPT:2/2\nT:Mode$ AttackerUnblocked | ValidCard$ Card.Self | Execute$ TrigGainControl |{decider} TriggerDescription$ Whenever CARDNAME attacks and isn't blocked, you may gain control of target artifact defending player controls.\nSVar:TrigGainControl:DB$ GainControl | ValidTgts$ Artifact.ControlledBy TriggeredDefendingPlayer\nOracle:x\n";

/// The rules of the card when it reads whole, else its gaps.
fn outcome(text: &str) -> Result<String, String> {
    match gaps(text).as_slice() {
        [] => Ok(Value::Array(abilities(text)).to_string()),
        found => Err(format!("{found:?}")),
    }
}

#[rstest]
#[case::black_permanent(RADIANT.replace("{kind}", "permanent"), Ok(r#""Count":[{"ControlledBy":"Opponents"},{"Color":"Black"}]"#))]
#[case::black_creature(RADIANT.replace("{kind}", "creature"), Ok(r#""Count":[{"Type":"Creature"}"#))]
#[case::attacked_with_merfolk(DEEPWAY.replace("{kind}", "Merfolk"), Err("condition:attackers-of-a-type"))]
#[case::attacked_with_creatures(DEEPWAY.replace("{kind}", "creatures"), Ok(r#""what":"Attackers""#))]
#[case::unchecked_mana_value(STIR.replace("{valid}", "Creature.YouCtrl"), Err("target:script-lacks-mana-value"))]
#[case::checked_mana_value(STIR.replace("{valid}", "Creature.YouCtrl+cmcLEX"), Ok(r#""ManaValue""#))]
#[case::your_graveyard(HANA.replace("{whose}", "your graveyard"), Ok(r#"{"In":"Graveyard"},{"OwnedBy":"You"}"#))]
#[case::a_graveyard(HANA.replace("{whose}", "a graveyard"), Ok(r#""what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"}]}"#))]
#[case::oracle_opponent(RIDER.replace("{who}", "opponent"), Ok(r#""targets":[{"what":"Opponent"}]"#))]
#[case::oracle_player(RIDER.replace("{who}", "player"), Ok(r#""targets":[{"what":"Player"}]"#))]
#[case::printed_sorcery(SLIVER.replace("{rest}", " Activate only as a sorcery."), Ok(r#""timing":"Sorcery""#))]
#[case::no_sorcery(SLIVER.replace("{rest}", ""), Err("no timing"))]
#[case::may_without_decider(PIRATES.replace("{decider}", ""), Err("trigger:may-target-without-decider"))]
#[case::may_with_decider(PIRATES.replace("{decider}", " OptionalDecider$ You |"), Ok(r#""May""#))]
fn fix16_a_script_that_says_less_follows_oracle(
    #[case] text: String,
    #[case] expected: Result<&str, &str>,
) {
    let got = outcome(&text);
    match (expected, &got) {
        (Ok(part), Ok(rules)) => assert!(rules.contains(part), "{rules}"),
        (Err("no timing"), Ok(rules)) => assert!(!rules.contains("timing"), "{rules}"),
        (Err(gap), Err(found)) => assert!(found.contains(gap), "{found}"),
        _ => panic!("{expected:?}: {got:?}"),
    }
}
