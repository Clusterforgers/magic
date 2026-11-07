//! Goal: round 15 pack a reads speed (CR 702.179): the keyword start your engines!, the condition max speed on a granted
//! ability, trigger, static or graveyard ability, "your speed" as an amount, max speed as a player property and its
//! negation, and a script word of speed it does not know stays a gap.
#![allow(clippy::unwrap_used, clippy::needless_raw_string_hashes)]

use rstest::rstest;

use super::{abilities, gaps};

#[rstest]
#[case::the_keyword(
    "Artifact",
    "K:Start your engines\nOracle:Start your engines!",
    r#"{"Keyword":"StartYourEngines"}"#
)]
#[case::a_granted_static(
    "Creature Vampire",
    r#"S:Mode$ Continuous | AffectedDefined$ Self | Condition$ MaxSpeed | AddStaticAbility$ MaxSpeedStatic | Description$ Max speed — This creature has double strike.
SVar:MaxSpeedStatic:Mode$ Continuous | Secondary$ True | AffectedDefined$ Self | AddKeyword$ Double Strike | Description$ Max speed — This creature has double strike.
Oracle:Max speed — This creature has double strike."#,
    r#"{"Static":{"affected":"This","condition":{"MaxSpeed":"You"},"mods":[{"GrantAbility":{"Static":{"affected":"This","mods":[{"AddKeyword":"DoubleStrike"}]}}}]}}"#
)]
#[case::a_granted_trigger(
    "Artifact",
    r#"S:Mode$ Continuous | AffectedDefined$ Self | Condition$ MaxSpeed | AddTrigger$ DrawTrig | Description$ Max speed — Whenever you draw a card, each opponent mills two cards.
SVar:DrawTrig:Mode$ Drawn | ValidCard$ Card.YouCtrl | TriggerZones$ Battlefield | Execute$ TrigMill | Secondary$ True | TriggerDescription$ Max speed — Whenever you draw a card, each opponent mills two cards.
SVar:TrigMill:DB$ Mill | Defined$ Opponent | NumCards$ 2
Oracle:Max speed — Whenever you draw a card, each opponent mills two cards."#,
    r#""condition":{"MaxSpeed":"You"},"mods":[{"GrantAbility":{"Triggered""#
)]
#[case::from_the_graveyard(
    "Creature Cat",
    r#"S:Mode$ Continuous | EffectZone$ Graveyard | AffectedZone$ Graveyard | Affected$ Card.Self | Condition$ MaxSpeed | AddAbility$ ABDraw | Description$ Max speed — {3}, Exile this card from your graveyard: Draw a card.
SVar:ABDraw:AB$ Draw | Cost$ 3 ExileFromGrave<1/CARDNAME/this card> | ActivationZone$ Graveyard | Secondary$ True | SpellDescription$ Draw a card.
Oracle:Max speed — {3}, Exile this card from your graveyard: Draw a card."#,
    r#""condition":{"MaxSpeed":"You"},"from":"Graveyard""#
)]
#[case::your_speed_as_a_count(
    "Enchantment",
    r#"A:AB$ GainLife | Cost$ 2 Sac<1/CARDNAME/this enchantment> | LifeAmount$ Count$YourSpeed | SpellDescription$ You gain life equal to your speed.
Oracle:{2}, Sacrifice this enchantment: You gain life equal to your speed."#,
    r#"{"GainLife":{"amount":{"Speed":"You"}}}"#
)]
#[case::each_player_without_max_speed(
    "Enchantment",
    r#"A:AB$ DealDamage | Cost$ 2 Sac<1/CARDNAME/this enchantment> | Defined$ Player.!MaxSpeed | NumDmg$ 2 | SpellDescription$ It deals 2 damage to each player who doesn't have max speed.
Oracle:{2}, Sacrifice this enchantment: It deals 2 damage to each player who doesn't have max speed."#,
    r#"{"PlayersWhere":{"test":{"Not":{"MaxSpeed":"You"}},"who":"EachPlayer"}}"#
)]
#[case::max_speed_as_a_number(
    "Creature God",
    r#"S:Mode$ CantAttack,CantBlock | ValidCard$ Card.Self | CheckSVar$ PlayerCountPropertyYou$HasPropertyMaxSpeed | SVarCompare$ NE1 | Description$ CARDNAME can't attack or block unless you have max speed.
Oracle:Hazoret can't attack or block unless you have max speed."#,
    r#"{"Players":{"PlayersWhere":{"test":{"MaxSpeed":"You"},"who":"You"}}}"#
)]
#[case::two_amounts_by_max_speed(
    "Instant",
    r#"A:SP$ Draw | NumCards$ X | SpellDescription$ Draw two cards if you have max speed, else one.
SVar:X:Count$MaxSpeed.2.1
Oracle:Draw a card. Max speed — Draw two cards instead."#,
    r#"{"MaxSpeed":"You"}"#
)]
fn a_speed_card_reads_without_a_gap_and_holds_its_fragment(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] fragment: &str,
) {
    let text = format!("Name:Subject\nTypes:{types}\n{lines}\n");
    let found = gaps(&text);
    assert!(found.is_empty(), "{found:?}");
    let json = serde_json::to_string(&abilities(&text)).unwrap();
    assert!(json.contains(fragment), "{fragment} missing in {json}");
}

/// A speed fact the engine does not hold, the highest speed among players, stays a gap.
#[test]
fn the_highest_speed_stays_a_gap() {
    let text = "Name:Subject\nTypes:Sorcery\nA:SP$ Draw | NumCards$ X\nSVar:X:PlayerCountPlayers$HighestSpeed\nOracle:Draw cards.\n";
    assert!(!gaps(&text.to_owned()).is_empty());
}
