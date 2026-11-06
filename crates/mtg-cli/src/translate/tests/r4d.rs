//! Goal: the filter words and own-target reads of round 4 pack d read the way the cards say them: an alternative that names
//! no zone is the table, a chosen player owns the control word, "another spell" leaves the source out, a line reads its own
//! target before it is declared, and a word that names a target the line never declares stays a gap.
use rstest::rstest;

use super::{abilities, gaps};

fn card(types: &str, lines: &str) -> String {
    format!("Name:T\nTypes:{types}\n{lines}\n")
}

#[rstest]
#[case::table_beside_stack(
    "Instant",
    "A:SP$ Animate | Colors$ Red | OverwriteColors$ True | ValidTgts$ Card | TgtZone$ Stack,Battlefield | Duration$ Permanent",
    r#"{"AnyOf":[{"In":"Stack"},{"In":"Battlefield"}]}"#
)]
#[case::table_beside_graveyard(
    "Planeswalker",
    "A:AB$ Destroy | Cost$ SubCounter<1/LOYALTY> | Planeswalker$ True | ValidTgts$ Artifact | TgtZone$ Battlefield,Graveyard",
    r#"{"AnyOf":[{"In":"Battlefield"},{"In":"Graveyard"}]}"#
)]
#[case::land_inside_permanent(
    "Creature",
    "S:Mode$ Continuous | Affected$ Land.YouOwn+ThisTurnEnteredFrom_Library,Permanent.YouOwn+ThisTurnEnteredFrom_Library | MayPlay$ True | EffectZone$ Battlefield | AffectedZone$ Graveyard",
    r#""AnyOf":[{"Type":"Artifact"}"#
)]
#[case::forecast_reveals_itself(
    "Sorcery",
    "A:AB$ Draw | Cost$ 2 U Reveal<1/CARDNAME> | NumCards$ 1 | Forecast$ True",
    r#""RevealFromHand":{"filter":["This"]}"#
)]
#[case::chosen_player_controls(
    "Creature",
    "K:ETBReplacement:Other:ChooseP\nSVar:ChooseP:DB$ ChoosePlayer | Defined$ You | Choices$ Player.Opponent\nS:Mode$ Continuous | Affected$ Card.Self | SetPower$ X | SetToughness$ X | CharacteristicDefining$ True\nSVar:X:Count$Valid Land.ChosenCtrl",
    r#"{"ControlledBy":"Chosen"}"#
)]
#[case::chosen_hand_is_owned(
    "Artifact",
    "K:ETBReplacement:Other:ChooseP\nSVar:ChooseP:DB$ ChoosePlayer | Defined$ You | Choices$ Player.Opponent\nT:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ Player.Chosen | TriggerZones$ Battlefield | Execute$ Hit\nSVar:Hit:DB$ DealDamage | Defined$ ChosenPlayer | NumDmg$ X\nSVar:X:Count$ValidHand Card.ChosenCtrl",
    r#"{"OwnedBy":"Chosen"}"#
)]
#[case::player_or_its_controller(
    "Planeswalker",
    "A:AB$ DealDamage | Cost$ SubCounter<1/LOYALTY> | Planeswalker$ True | ValidTgts$ Player,Planeswalker | NumDmg$ 1 | SubAbility$ All\nSVar:All:DB$ DamageAll | NumDmg$ 1 | ValidCards$ Creature.ControlledBy TargetedOrController",
    r#"{"AnyOf":[{"ControlledBy":{"Target":0}},{"ControlledBy":{"ControllerOf":{"Target":0}}}]}"#
)]
#[case::other_spell_cast(
    "Creature",
    "K:etbCounter:P1P1:X\nSVar:X:Count$ThisTurnCast_Card.!CastSaSource",
    r#""filter":["Other"]"#
)]
#[case::target_creates_token(
    "Creature",
    "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ Tok\nSVar:Tok:DB$ Token | TokenScript$ w_1_1_soldier | ValidTgts$ Opponent | TokenOwner$ Targeted",
    r#""who":{"Target":0}"#
)]
#[case::top_creature_of_the_graveyard(
    "Creature",
    "A:AB$ Pump | Cost$ ExileFromGrave<1/Card.TopGraveyardCreature> | NumAtt$ +2 | NumDef$ +2",
    r#""TopOfGraveyard""#
)]
#[case::targeted_by_the_spell(
    "Instant",
    "A:SP$ Pump | ValidTgts$ Creature.YouCtrl | NumAtt$ +1 | SubAbility$ Reg\nSVar:Reg:DB$ Regenerate | Defined$ Valid Creature.YouCtrl+!targetedBy",
    r#"{"Not":{"Is":{"Target":0}}}"#
)]
#[case::destroy_remembers_the_target(
    "Sorcery",
    "A:SP$ Destroy | ValidTgts$ Creature | AlwaysRemember$ True | RememberLKI$ True | SubAbility$ Dis\nSVar:Dis:DB$ Discard | Defined$ RememberedController | NumCards$ 1 | Mode$ TgtChoose",
    r#""who":{"ControllerOf":{"Target":0}}"#
)]
#[case::condition_reads_its_own_target(
    "Instant",
    "A:SP$ Pump | ValidTgts$ Creature.YouCtrl | NumAtt$ +2 | ConditionDefined$ ThisTargetedCard | ConditionPresent$ Creature.powerEQ2",
    r#""subject":{"Target":0}"#
)]
#[case::amount_reads_its_own_player_target(
    "Instant",
    "A:SP$ Draw | ValidTgts$ Opponent | NumCards$ X\nSVar:X:Count$ValidHand Card.TargetedPlayerOwn",
    r#"{"OwnedBy":{"Target":0}}"#
)]
fn a_word_of_the_pack_reads_as_the_card_says(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] needle: &str,
) {
    let script = card(types, lines);
    assert!(gaps(&script).is_empty(), "{:?}", gaps(&script));
    let text: String = abilities(&script).iter().map(ToString::to_string).collect();
    assert!(text.contains(needle), "{needle} not in {text}");
}

#[rstest]
#[case::chosen_without_a_choice(
    "Creature",
    "S:Mode$ Continuous | Affected$ Card.Self | SetPower$ X | SetToughness$ X | CharacteristicDefining$ True\nSVar:X:Count$Valid Land.ChosenCtrl"
)]
#[case::other_spell_of_a_trigger(
    "Creature",
    "T:Mode$ SpellCast | ValidActivatingPlayer$ You | Execute$ Go | TriggerZones$ Battlefield\nSVar:Go:DB$ Draw | NumCards$ X\nSVar:X:Count$ThisTurnCast_Card.!CastSaSource"
)]
#[case::target_the_line_never_declares(
    "Instant",
    "A:SP$ Pump | ValidTgts$ Creature.YouCtrl | NumAtt$ +1 | SubAbility$ Drop\nSVar:Drop:DB$ AnimateAll | ValidCards$ Creature.TargetedPlayerCtrl | ValidTgts$ Opponent | RemoveKeywords$ First Strike"
)]
fn a_word_without_what_it_reads_stays_a_gap(#[case] types: &str, #[case] lines: &str) {
    assert!(!gaps(&card(types, lines)).is_empty());
}
