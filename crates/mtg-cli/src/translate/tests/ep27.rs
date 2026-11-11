//! Goal: the pieces of ep27 translate as the card prints them. A card cast as though it had flash under a condition (`CastWithFlash | ValidCard$ Card.Self | EffectZone$ All | Caster$ You`) is a `MayCast` rule of the card from its hand with the line's condition; a check of the spell (X, its targets, teamwork), another caster or zone stays a gap. "You don't lose the game for having 0 or less life" (`GameLoss | ValidLoseReason$ LifeReachedZero | Layer$ CantHappen`) is a `CantLoseGame` rule for the loss by life alone, under the line's condition; another reason, text or player stays a gap. "If you would lose the game, instead ..." (`GameLoss` with `ReplaceWith$`) is a `LoseGame` replacement by the steps of the chain; another player, a layer or a reason stays a gap. Hands played revealed (`MayLookAt$ Player` over `AffectedZone$ Hand`) are a `RevealHand` rule for every player, the opponents or you; a look by one seat or another owner stays a gap. Damage lessened (`ReplaceCount$DamageAmount/Minus.N`), halved rounded down (`HalfDown`) or capped (a fixed N under `DamageAmount$ GE<N+1>`) is a `Subtract`, `Half` or `Cap` replacement; another threshold, a fixed amount alone, nothing subtracted or another rounding stays a gap.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn revelation(line: &str) -> String {
    format!(
        "Name:T\nTypes:Enchantment\nS:Mode$ Continuous | AffectedZone$ Hand | {line} | Description$ x\n"
    )
}

#[rstest]
#[case::players("MayLookAt$ Player", "EachPlayer")]
#[case::opponents("Affected$ Card.OppOwn | MayLookAt$ Player", "Opponents")]
#[case::you("Affected$ Card.YouOwn | MayLookAt$ Player", "You")]
fn ep27_hands_revealed(#[case] line: &str, #[case] who: &str) {
    let script = revelation(line);
    assert_eq!(gaps(&script), Vec::<String>::new(), "{line}");
    assert_eq!(
        abilities(&script),
        vec![json!({"Rule": {"rule": {"RevealHand": who}}})]
    );
}

#[rstest]
#[case::a_look_by_you_alone("Affected$ Card.OppOwn | MayLookAt$ You")]
#[case::another_owner("Affected$ Card.RememberedPlayerOwn | MayLookAt$ Player")]
fn ep27_other_hand_looks_stay_gaps(#[case] line: &str) {
    let found = gaps(&revelation(line));
    assert!(
        found.iter().any(|gap| gap.contains("MayLookAt")),
        "{line}: {found:?}"
    );
}

fn unlife(line: &str) -> String {
    format!("Name:T\nTypes:Enchantment\nR:Event$ GameLoss | ActiveZones$ Battlefield | {line}\n")
}

const SPARED: &str = "ValidPlayer$ You | ValidLoseReason$ LifeReachedZero | Layer$ CantHappen | Description$ You don't lose the game for having 0 or less life.";

#[test]
fn ep27_no_loss_for_life() {
    let script = unlife(SPARED);
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(
        abilities(&script),
        vec![json!({"Rule": {"rule": {"CantLoseGame": {"who": "You", "life_only": true}}}})]
    );
}

#[test]
fn ep27_instead_of_losing() {
    let script = "Name:T\nTypes:Artifact\nR:Event$ GameLoss | ActiveZones$ Battlefield | ValidPlayer$ You | ReplaceWith$ ExileSetLife | Description$ If you would lose the game, instead exile CARDNAME and your life total becomes 1.\nSVar:ExileSetLife:DB$ ChangeZone | Origin$ Battlefield | Destination$ Exile | Defined$ Self | SubAbility$ DBSetLife\nSVar:DBSetLife:DB$ SetLife | Defined$ You | LifeAmount$ 1\n";
    assert_eq!(gaps(script), Vec::<String>::new());
    assert_eq!(
        abilities(script),
        vec![json!({"Replacement": {
            "when": {"LoseGame": {}},
            "by": {"Instead": [{"Exile": {"what": "This"}}, {"SetLife": {"amount": 1}}]},
        }})]
    );
}

#[rstest]
#[case::another_reason(
    "ValidPlayer$ You | ValidLoseReason$ Milled | Layer$ CantHappen | Description$ You don't lose the game for having 0 or less life."
)]
#[case::another_text(
    "ValidPlayer$ You | ValidLoseReason$ LifeReachedZero | Layer$ CantHappen | Description$ You can't lose the game."
)]
#[case::another_player(
    "ValidPlayer$ Opponent | ValidLoseReason$ LifeReachedZero | Layer$ CantHappen | Description$ Your opponents don't lose the game for having 0 or less life."
)]
#[case::instead_for_life_alone(
    "ValidPlayer$ You | ValidLoseReason$ LifeReachedZero | ReplaceWith$ X | Description$ You don't lose the game for having 0 or less life."
)]
#[case::instead_for_an_opponent(
    "ValidPlayer$ Opponent | ReplaceWith$ X | Description$ If an opponent would lose the game, instead x."
)]
#[case::instead_with_a_layer("ValidPlayer$ You | Layer$ Control | ReplaceWith$ X | Description$ x")]
fn ep27_other_game_losses_stay_gaps(#[case] line: &str) {
    let found = gaps(&unlife(line));
    assert!(
        found.iter().any(|gap| gap.contains("GameLoss")),
        "{line}: {found:?}"
    );
}

fn spinners(line: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Faerie\nPT:1/1\nS:Mode$ CastWithFlash | ValidCard$ Card.Self | {line}\n"
    )
}

#[test]
fn ep27_self_flash_under_a_condition() {
    let script = spinners(
        "ValidSA$ Spell | IsPresent$ Creature.YouCtrl | EffectZone$ All | Caster$ You | Description$ You may cast this spell as though it had flash if you control a creature.",
    );
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(
        abilities(&script),
        vec![json!({"Rule": {
            "rule": {"MayCast": {"what": ["This"], "flash": true}},
            "from": "Hand",
            "condition": {"Compare": {"left": {"Count": [{"Type": "Creature"}, {"ControlledBy": "You"}]}, "op": "Ge", "right": 1}},
        }})]
    );
}

#[rstest]
#[case::x_checked("ValidSA$ Spell.XCostLE3 | EffectZone$ All | Caster$ You")]
#[case::targets_checked(
    "ValidSA$ Spell.IsTargeting Valid Card.IsCommander | EffectZone$ All | Caster$ You"
)]
#[case::teamwork("ValidSA$ Spell.Teamwork | EffectZone$ All | Caster$ You")]
#[case::any_caster(
    "ValidSA$ Spell | IsPresent$ Creature.YouCtrl | EffectZone$ All | Caster$ Player"
)]
#[case::from_the_battlefield("ValidSA$ Spell | IsPresent$ Creature.YouCtrl | Caster$ You")]
fn ep27_other_self_flash_stays_a_gap(#[case] line: &str) {
    let found = gaps(&spinners(line));
    assert!(
        found.iter().any(|gap| gap.contains("CastWithFlash")),
        "{line}: {found:?}"
    );
}

fn lessening(line: &str, value: &str) -> String {
    format!(
        "Name:T\nTypes:Enchantment\nR:Event$ DamageDone | ActiveZones$ Battlefield | ValidTarget$ Permanent,Player | {line}ReplaceWith$ Dmg | Description$ x\nSVar:Dmg:DB$ ReplaceEffect | VarName$ DamageAmount | VarValue$ {value}\nSVar:X:ReplaceCount$DamageAmount/{value}\n"
    )
}

#[rstest]
#[case::minus_one("", "X", "Minus.1", json!({"Subtract": 1}))]
#[case::half_down("", "X", "HalfDown", json!("Half"))]
#[case::capped("DamageAmount$ GE4 | ", "3", "", json!({"Cap": 3}))]
fn ep27_lessened_damage(
    #[case] line: &str,
    #[case] value: &str,
    #[case] step: &str,
    #[case] by: serde_json::Value,
) {
    let script = lessening(line, value).replace("/X\n", &format!("/{step}\n"));
    assert_eq!(gaps(&script), Vec::<String>::new(), "{script}");
    assert_eq!(
        abilities(&script),
        vec![json!({"Replacement": {"when": {"Damage": {}}, "by": by}})]
    );
}

#[rstest]
#[case::a_cap_under_another_threshold("DamageAmount$ GE5 | ", "3", "")]
#[case::a_fixed_amount_without_a_threshold("", "3", "")]
#[case::a_threshold_on_a_subtraction("DamageAmount$ GE2 | ", "X", "Minus.1")]
#[case::minus_nothing("", "X", "Minus.0")]
#[case::half_up("", "X", "HalfUp")]
fn ep27_other_damage_amounts_stay_gaps(
    #[case] line: &str,
    #[case] value: &str,
    #[case] step: &str,
) {
    let script = lessening(line, value).replace("/X\n", &format!("/{step}\n"));
    let found = gaps(&script);
    assert!(
        found
            .iter()
            .any(|gap| gap.contains("DamageDone") || gap.contains("DamageAmount")),
        "{script}: {found:?}"
    );
}
