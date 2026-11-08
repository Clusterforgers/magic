//! Goal: the object and event words of the g16s tail read as the text says: a card chosen from several may be played
//! (`Card.ChosenCard`, typed alternatives), "you and target opponent each create", the permanent tapped for mana and the
//! source of a damage trigger in a filter or a cost, the player a damage trigger hurt in the target of its reflexive
//! trigger, the basic land types of the player whose upkeep it is, the cards a scry looked at, and the permanent tapped
//! as a cost; a word the engine does not hold stays a gap.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

/// The script, its gaps, and the rules as text.
fn read(script: &str) -> (Vec<String>, String) {
    (gaps(script), json!(abilities(script)).to_string())
}

#[rstest]
#[case::chosen_card(
    "Card.ChosenCard",
    "Exile the top two cards of your library. Choose one of them. Until the end of your next turn, you may play that card.",
    false
)]
#[case::typed_alternatives(
    "Instant.IsRemembered,Sorcery.IsRemembered",
    "Exile the top two cards of your library. Until the end of your next turn, you may cast instant or sorcery spells from among them.",
    true
)]
fn a_remembered_card_may_be_played(
    #[case] affected: &str,
    #[case] oracle: &str,
    #[case] filtered: bool,
) {
    let script = format!(
        "Name:Test Seer\nTypes:Sorcery\nA:SP$ Dig | Defined$ You | DigNum$ 2 | ChangeNum$ All | DestinationZone$ Exile | RememberChanged$ True | SubAbility$ DBEffect | SpellDescription$ {oracle}\nSVar:DBEffect:DB$ Effect | RememberObjects$ RememberedCard | StaticAbilities$ Play | Duration$ UntilTheEndOfYourNextTurn | ForgetOnMoved$ Exile | SubAbility$ DBCleanup\nSVar:Play:Mode$ Continuous | MayPlay$ True | Affected$ {affected} | AffectedZone$ Exile\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\nOracle:{oracle}\n"
    );
    let (gaps, found) = read(&script);
    let unread = gaps.iter().any(|gap| gap.contains("MayPlay-Affected"));
    assert!(!unread, "{gaps:?} {found}");
    if filtered {
        assert!(found.contains("\"MayCast\""), "{found}");
        assert!(found.contains("\"what\":[{\"AnyOf\""), "{found}");
    }
}

#[rstest]
#[case::unknown_head("Creature.Foo+IsRemembered", false)]
#[case::no_remembered("Instant.YouOwn", true)]
fn a_card_word_without_the_remembered_cards_is_not_read(
    #[case] affected: &str,
    #[case] other: bool,
) {
    let script = format!(
        "Name:Test Seer\nTypes:Sorcery\nA:SP$ Dig | Defined$ You | DigNum$ 2 | ChangeNum$ All | DestinationZone$ Exile | RememberChanged$ True | SubAbility$ DBEffect\nSVar:DBEffect:DB$ Effect | RememberObjects$ RememberedCard | StaticAbilities$ Play | SubAbility$ DBCleanup\nSVar:Play:Mode$ Continuous | MayPlay$ True | Affected$ {affected} | AffectedZone$ Exile\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\nOracle:Exile the top two cards of your library. You may play them.\n"
    );
    let (gaps, found) = read(&script);
    if other {
        assert!(
            gaps.iter().any(|gap| gap.contains("MayPlay-Affected")),
            "{gaps:?} {found}"
        );
    } else {
        assert!(!gaps.is_empty() || !found.contains("\"only\""), "{found}");
    }
}

#[rstest]
#[case::treasure("c_a_treasure_sac", "a Treasure token")]
fn you_and_a_target_opponent_each_create_a_token(#[case] script_name: &str, #[case] noun: &str) {
    let text = format!("When this creature enters, you and target opponent each create {noun}.");
    let script = format!(
        "Name:Test Banker\nTypes:Creature Human\nPT:1/1\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigToken | TriggerDescription$ {text}\nSVar:TrigToken:DB$ Token | ValidTgts$ Opponent | TokenOwner$ TargetedAndYou | TokenScript$ {script_name}\nOracle:{text}\n"
    );
    let (gaps, found) = read(&script);
    assert!(gaps.is_empty(), "{gaps:?}");
    assert_eq!(found.matches("\"CreateToken\"").count(), 2, "{found}");
    assert!(found.contains("{\"CreateToken\":{\"token\""), "{found}");
    assert!(found.contains("\"who\":{\"Target\":0}"), "{found}");
}

#[test]
fn a_token_owner_with_a_remembered_token_is_a_gap() {
    let script = "Name:Test Banker\nTypes:Sorcery\nA:SP$ Token | ValidTgts$ Opponent | TokenOwner$ TargetedAndYou | TokenScript$ c_a_treasure_sac | RememberTokens$ True\nOracle:You and target opponent each create a Treasure token.\n";
    assert!(!gaps(script).is_empty());
}

#[test]
fn the_land_tapped_for_mana_is_the_triggering_object() {
    let script = "Name:Test Cauldron\nTypes:Artifact\nT:Mode$ TapsForMana | ValidCard$ Land | Execute$ TrigBounce | TriggerZones$ Battlefield | TriggerDescription$ Whenever a land is tapped for mana, return it to its owner's hand.\nSVar:TrigBounce:DB$ ChangeZone | Origin$ Battlefield | Destination$ Hand | Defined$ TriggeredCardLKICopy\nOracle:Whenever a land is tapped for mana, return it to its owner's hand.\n";
    let (gaps, found) = read(script);
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(
        found.contains("\"Move\":{\"to\":\"Hand\",\"what\":\"Triggering\"}"),
        "{found}"
    );
}

#[rstest]
#[case::source_in_a_cost(
    "Card.TriggeredSource",
    "\"Sacrifice\":{\"filter\":[{\"Is\":\"Dealer\"}]}"
)]
fn the_source_of_a_damage_trigger_is_a_cost_object(#[case] valid: &str, #[case] cost: &str) {
    let script = format!(
        "Name:Test Helm\nTypes:Artifact Equipment\nT:Mode$ DamageDone | ValidSource$ Creature.EquippedBy | ValidTarget$ Player | CombatDamage$ True | Execute$ TrigDraw | TriggerZones$ Battlefield | TriggerDescription$ Whenever equipped creature deals combat damage to a player, you may sacrifice it. If you do, draw three cards.\nSVar:TrigDraw:AB$ Draw | Cost$ Sac<1/{valid}> | NumCards$ 3\nOracle:Whenever equipped creature deals combat damage to a player, you may sacrifice it. If you do, draw three cards.\n"
    );
    let (gaps, found) = read(&script);
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(found.contains(cost), "{found}");
}

#[test]
fn a_reflexive_target_is_controlled_by_the_player_the_damage_hurt() {
    let script = "Name:Test Ninja\nTypes:Creature Human Ninja\nPT:2/2\nT:Mode$ DamageDone | ValidSource$ Card.Self | ValidTarget$ Player | Execute$ TrigImmediate | CombatDamage$ True | OptionalDecider$ You | TriggerDescription$ Whenever this creature deals combat damage to a player, you may discard a creature card. When you do, destroy target creature that player controls.\nSVar:TrigImmediate:AB$ ImmediateTrigger | Cost$ Discard<1/Creature> | Execute$ TrigDestroy | TriggerDescription$ When you do, destroy target creature that player controls.\nSVar:TrigDestroy:DB$ Destroy | ValidTgts$ Creature.ControlledBy Spawner>TriggeredTarget\nOracle:Whenever this creature deals combat damage to a player, you may discard a creature card. When you do, destroy target creature that player controls.\n";
    let (gaps, found) = read(script);
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(
        found.contains("\"Bind\":{\"as\":\"when") && found.contains("{\"ControlledBy\":{\"Bound\""),
        "{found}"
    );
}

#[test]
fn the_domain_of_the_active_player_counts_their_lands() {
    let script = "Name:Test Borders\nTypes:Enchantment\nT:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ Player | Execute$ TrigGain | TriggerZones$ Battlefield | TriggerDescription$ Domain \u{2014} At the beginning of each player's upkeep, that player gains 1 life for each basic land type among lands they control.\nSVar:TrigGain:DB$ GainLife | Defined$ TriggeredPlayer | LifeAmount$ X\nSVar:X:Count$DomainActivePlayer\nOracle:Domain \u{2014} At the beginning of each player's upkeep, that player gains 1 life for each basic land type among lands they control.\n";
    let (gaps, found) = read(script);
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(
        found.contains("{\"ControlledBy\":\"ActivePlayer\"}"),
        "{found}"
    );
}

#[test]
fn the_cards_a_scry_looked_at_are_the_amount_of_the_scry_trigger() {
    let script = "Name:Test Seer\nTypes:Creature Elf\nPT:1/1\nT:Mode$ Scry | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ TrigPump | TriggerDescription$ Whenever you scry, this creature gets +1/+1 until end of turn for each card you looked at while scrying this way.\nSVar:TrigPump:DB$ Pump | Defined$ Self | NumAtt$ +X | NumDef$ +X\nSVar:X:TriggerCount$ScryNum\nOracle:Whenever you scry, this creature gets +1/+1 until end of turn for each card you looked at while scrying this way.\n";
    let (gaps, found) = read(script);
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(found.contains("\"power\":\"Triggering\""), "{found}");
}

#[test]
fn the_creature_tapped_for_a_cost_is_the_paid_object() {
    let script = "Name:Test Sling\nTypes:Artifact\nA:AB$ DealDamage | Cost$ 3 T tapXType<1/Creature> | ValidTgts$ Creature | NumDmg$ X | SpellDescription$ Test Sling deals damage equal to the tapped creature's power to target creature.\nSVar:X:Tapped$CardPower\nOracle:{3}, {T}, Tap an untapped creature you control: Test Sling deals damage equal to the tapped creature's power to target creature.\n";
    let (gaps, found) = read(script);
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(
        found.contains("{\"Stat\":{\"of\":\"Paid\",\"stat\":\"Power\"}}"),
        "{found}"
    );
}

#[test]
fn a_tapped_creature_beside_another_paid_cost_is_a_gap() {
    let script = "Name:Test Sling\nTypes:Artifact\nA:AB$ DealDamage | Cost$ Sac<1/Creature> tapXType<1/Creature> | ValidTgts$ Creature | NumDmg$ X\nSVar:X:Tapped$CardPower\nOracle:Sacrifice a creature, Tap an untapped creature you control: Test Sling deals damage equal to the tapped creature's power to target creature.\n";
    assert!(!gaps(script).is_empty());
}
