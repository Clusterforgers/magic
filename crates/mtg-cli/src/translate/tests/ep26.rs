//! Goal: the pieces of ep26 translate as the card prints them: a target chosen at random (`TargetsAtRandom$ True`) on any verb that reads targets, over players, opponents and objects; a random number of random targets stays a gap. Exiling permanents you control as a cost (`Exile<N/Type>`); a count "or more" or a total stays a gap. Mana of the colours of the imprinted card or of the cards exiled with the source; an imprint without its exile or another memory stays a gap. A mana ability activated only as an instant (`InstantSpeed$ True`); another word or both speeds stay gaps. The next spell that can't be countered this turn (a static `SpellCast` delayed trigger with a `CantHappen` counter replacement); another turn, memory, spell ability or event stays a gap. A card cast from its graveyard or from exile (two permissions); other cards, another effect zone or three zones stay gaps.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn activated(line: &str) -> String {
    format!("Name:T\nTypes:Artifact Creature Goblin\nPT:1/1\nA:AB$ {line}\n")
}

fn targets(script: &str) -> Value {
    abilities(script)
        .first()
        .and_then(|found| found.get("Activated"))
        .and_then(|body| body.get("targets"))
        .cloned()
        .unwrap_or_default()
}

#[rstest]
#[case::any_target(
    "DealDamage | Cost$ T | NumDmg$ 2 | ValidTgts$ Any | TargetsAtRandom$ True",
    json!("AnyTarget")
)]
#[case::an_opponent(
    "DealDamage | Cost$ T | NumDmg$ 2 | ValidTgts$ Opponent | TargetsAtRandom$ True",
    json!("Opponent")
)]
#[case::a_creature_an_opponent_controls(
    "Destroy | Cost$ T | ValidTgts$ Creature.OppCtrl | TargetsAtRandom$ True",
    json!({"Object": [{"Type": "Creature"}, {"ControlledBy": "Opponents"}]})
)]
fn ep26_a_target_chosen_at_random(#[case] line: &str, #[case] what: Value) {
    let script = activated(line);
    assert_eq!(gaps(&script), Vec::<String>::new(), "{line}");
    assert_eq!(targets(&script), json!([{"what": {"AtRandom": what}}]));
}

#[rstest]
#[case::a_random_number_of_targets(
    "Tap | Cost$ T | ValidTgts$ Creature | TargetMin$ 0 | TargetMax$ 3 | TargetsAtRandom$ True"
)]
#[case::another_word("DealDamage | Cost$ T | NumDmg$ 2 | ValidTgts$ Any | TargetsAtRandom$ Some")]
fn ep26_other_random_targets_stay_gaps(#[case] line: &str) {
    let found = gaps(&activated(line));
    assert!(
        found.iter().any(|gap| gap.contains("TargetsAtRandom")),
        "{line}: {found:?}"
    );
}

fn cost(script: &str) -> Value {
    abilities(script)
        .first()
        .and_then(|found| found.get("Activated"))
        .and_then(|body| body.get("cost"))
        .cloned()
        .unwrap_or_default()
}

#[rstest]
#[case::a_creature("Exile<1/Creature>", json!({"filter": [{"Type": "Creature"}]}))]
#[case::two_artifacts("Exile<2/Artifact>", json!({"filter": [{"Type": "Artifact"}], "count": 2}))]
fn ep26_exile_a_permanent_you_control_as_a_cost(#[case] part: &str, #[case] body: Value) {
    let script = activated(&format!("GainLife | Cost$ {part} | LifeAmount$ 3"));
    assert_eq!(gaps(&script), Vec::<String>::new(), "{part}");
    assert_eq!(cost(&script), json!([{"ExilePermanents": body}]));
}

#[rstest]
#[case::one_or_more("Exile<1+/Artifact.Other>")]
#[case::a_total("Exile<1/Artifact.Other+withTotalCMCEQX>")]
fn ep26_other_exile_costs_stay_gaps(#[case] part: &str) {
    let script = activated(&format!("GainLife | Cost$ {part} | LifeAmount$ 3"));
    assert!(!gaps(&script).is_empty(), "{part}");
}

const IMPRINT: &str = "T:Mode$ ChangesZone | ValidCard$ Card.Self | Origin$ Any | Destination$ Battlefield | OptionalDecider$ You | Execute$ TrigExile | TriggerDescription$ x\nSVar:TrigExile:DB$ ChangeZone | Imprint$ True | Origin$ Hand | Destination$ Exile | ChangeType$ Card.nonArtifact+nonLand | ChangeNum$ 1\n";

fn mox(valid: &str, imprint: bool) -> String {
    format!(
        "Name:T\nTypes:Artifact\n{}A:AB$ ManaReflected | Cost$ T | Valid$ {valid} | ColorOrType$ Color | ReflectProperty$ Is | SpellDescription$ x\n",
        if imprint { IMPRINT } else { "" }
    )
}

fn mana_among(script: &str) -> Value {
    abilities(script)
        .iter()
        .find_map(|found| found.get("Activated"))
        .and_then(|body| body.pointer("/effects/0/AddMana/mana/AnyColorAmong"))
        .cloned()
        .unwrap_or_default()
}

#[rstest]
#[case::the_imprinted_card(
    "Defined.Imprinted",
    true,
    json!([{"Is": {"Remembered": {"on": "This", "slot": "imprinted"}}}, {"In": "Exile"}])
)]
#[case::the_cards_exiled_with_it("Defined.ExiledWith", false, json!([{"ExiledWith": "This"}, {"In": "Exile"}]))]
fn ep26_mana_of_the_exiled_cards_colors(
    #[case] valid: &str,
    #[case] imprint: bool,
    #[case] among: Value,
) {
    let script = mox(valid, imprint);
    assert_eq!(gaps(&script), Vec::<String>::new(), "{valid}");
    assert_eq!(mana_among(&script), among);
}

#[rstest]
#[case::imprinted_without_an_imprint("Defined.Imprinted")]
#[case::remembered("Defined.Remembered")]
fn ep26_other_exiled_colors_stay_gaps(#[case] valid: &str) {
    assert!(!gaps(&mox(valid, false)).is_empty(), "{valid}");
}

fn diamond(speed: &str) -> String {
    format!(
        "Name:T\nTypes:Artifact\nA:AB$ Mana | Cost$ Sac<1/CARDNAME> Discard<0/Hand> | Produced$ Any | Amount$ 3{speed} | SpellDescription$ x\n"
    )
}

fn timing(script: &str) -> Value {
    abilities(script)
        .first()
        .and_then(|found| found.pointer("/Activated/timing"))
        .cloned()
        .unwrap_or_default()
}

#[rstest]
#[case::as_an_instant(" | InstantSpeed$ True", json!("AsInstant"))]
#[case::at_any_time("", Value::Null)]
#[case::as_a_sorcery(" | SorcerySpeed$ True", json!("Sorcery"))]
fn ep26_a_mana_ability_activated_only_as_an_instant(#[case] speed: &str, #[case] read: Value) {
    let script = diamond(speed);
    assert_eq!(gaps(&script), Vec::<String>::new(), "{speed}");
    assert_eq!(timing(&script), read);
}

#[rstest]
#[case::another_word(" | InstantSpeed$ Some")]
#[case::both(" | InstantSpeed$ True | SorcerySpeed$ True")]
fn ep26_other_speeds_stay_gaps(#[case] speed: &str) {
    assert!(!gaps(&diamond(speed)).is_empty(), "{speed}");
}

const ANTI_MAGIC: &str = "Event$ Counter | ValidCard$ Card.IsRemembered | ValidSA$ Spell | Layer$ CantHappen | Description$ x";

fn insist(valid: &str, trigger: &str, effect: &str, replacement: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ DelayedTrigger | Mode$ SpellCast{valid} | ValidActivatingPlayer$ You | Execute$ DBEffect{trigger} | Static$ True | SpellDescription$ x\nSVar:DBEffect:DB$ Effect | ReplacementEffects$ AntiMagic | RememberObjects$ TriggeredCard{effect}\nSVar:AntiMagic:{replacement}\n"
    )
}

fn spell_effects(script: &str) -> Value {
    abilities(script)
        .first()
        .and_then(|found| found.pointer("/Spell/effects"))
        .cloned()
        .unwrap_or_default()
}

const STACK: &str = " | ForgetOnMoved$ Stack";
const TURN: &str = " | ThisTurn$ True";

#[rstest]
#[case::any_spell("", json!({"benefit": "Uncounterable"}))]
#[case::a_creature_spell(" | ValidCard$ Creature", json!({"benefit": "Uncounterable", "what": [{"Type": "Creature"}]}))]
fn ep26_the_next_spell_cannot_be_countered(#[case] valid: &str, #[case] rule: Value) {
    let script = insist(valid, TURN, STACK, ANTI_MAGIC);
    assert_eq!(gaps(&script), Vec::<String>::new(), "{valid}");
    assert_eq!(
        spell_effects(&script),
        json!([{"Rule": {"rule": {"NextSpell": rule}, "duration": "UntilEndOfTurn"}}])
    );
}

#[rstest]
#[case::any_turn(insist("", "", STACK, ANTI_MAGIC))]
#[case::kept_after_the_stack(insist("", TURN, "", ANTI_MAGIC))]
#[case::countered_by_abilities(insist(
    "",
    TURN,
    STACK,
    "Event$ Counter | ValidCard$ Card.IsRemembered | Layer$ CantHappen"
))]
#[case::another_event(insist(
    "",
    TURN,
    STACK,
    "Event$ Destroy | ValidCard$ Card.IsRemembered | ValidSA$ Spell | Layer$ CantHappen"
))]
fn ep26_other_next_spell_shields_stay_gaps(#[case] script: String) {
    assert!(!gaps(&script).is_empty(), "{script}");
}

fn squee(affected: &str, zones: &str, effect_zones: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Goblin\nPT:2/1\nS:Mode$ Continuous | Affected$ {affected} | MayPlay$ True | AffectedZone$ {zones} | EffectZone$ {effect_zones} | Description$ x\n"
    )
}

#[rstest]
#[case::exile_first("Exile,Graveyard")]
#[case::graveyard_first("Graveyard,Exile")]
fn ep26_a_card_cast_from_two_zones(#[case] zones: &str) {
    let script = squee("Card.Self", zones, zones);
    assert_eq!(gaps(&script), Vec::<String>::new(), "{zones}");
    let one = |zone: &str| json!({"Rule": {"from": zone, "rule": {"MayCast": {"from": zone, "what": ["This"]}}}});
    assert_eq!(abilities(&script), vec![one("Graveyard"), one("Exile")]);
}

#[rstest]
#[case::other_cards(squee("Creature.YouOwn", "Exile,Graveyard", "Exile,Graveyard"))]
#[case::from_the_battlefield(squee("Card.Self", "Exile,Graveyard", "Battlefield"))]
#[case::three_zones(squee("Card.Self", "Exile,Graveyard,Hand", "Exile,Graveyard,Hand"))]
fn ep26_other_two_zone_permissions_stay_gaps(#[case] script: String) {
    assert!(!gaps(&script).is_empty(), "{script}");
}
