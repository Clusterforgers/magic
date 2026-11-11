//! Goal: the pieces of ep27 translate as the card prints them. A card cast as though it had flash under a condition (`CastWithFlash | ValidCard$ Card.Self | EffectZone$ All | Caster$ You`) is a `MayCast` rule of the card from its hand with the line's condition; a check of the spell (X, its targets, teamwork), another caster or zone stays a gap. "You don't lose the game for having 0 or less life" (`GameLoss | ValidLoseReason$ LifeReachedZero | Layer$ CantHappen`) is a `CantLoseGame` rule for the loss by life alone, under the line's condition; another reason, text or player stays a gap.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

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
#[case::instead(
    "ValidPlayer$ You | ValidLoseReason$ LifeReachedZero | ReplaceWith$ X | Description$ You don't lose the game for having 0 or less life."
)]
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
