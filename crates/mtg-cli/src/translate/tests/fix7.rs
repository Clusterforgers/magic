//! Goal: the faults of the round 4 audit stay closed: limits, top creature of a graveyard, target spell, and the words that are gaps.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a card with the given header and lines as compact JSON, and its gaps.
fn card(types: &str, lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:{types}\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

#[rstest]
fn casey_and_raph_is_a_gap_for_its_player_duration_and_its_different_players() {
    let (_, found) = card(
        "Legendary Creature Turtle",
        "PT:4/4\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigCharm\nSVar:TrigCharm:DB$ Charm | MinCharmNum$ 1 | CharmNum$ 2 | Choices$ DBExile,DBToken\nSVar:DBExile:DB$ Dig | ValidTgts$ Player | DigNum$ 1 | ChangeNum$ All | DestinationZone$ Exile | RememberChanged$ True | SubAbility$ DBEffect\nSVar:DBEffect:DB$ Effect | StaticAbilities$ CRPlay | EffectOwner$ RememberedOwner | Duration$ UntilYourNextEndStep | ExileOnMoved$ Exile | RememberObjects$ Remembered | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\nSVar:CRPlay:Mode$ Continuous | Affected$ Card.IsRemembered | MayPlay$ True | MayPlayWithoutManaCost$ True | AffectedZone$ Exile\nSVar:DBToken:DB$ Token | ValidTgts$ Player | TargetUnique$ True | TokenOwner$ ThisTargetedPlayer | TokenAmount$ 2 | TokenScript$ c_a_treasure_sac",
    );
    assert!(
        found
            .iter()
            .any(|gap| gap.contains("EffectOwner-with-a-turn-duration")),
        "{found:?}"
    );
}

#[rstest]
#[case::mind_bomb(
    "DB$ Discard | Defined$ Player.IsRemembered | NumCards$ 2 | Mode$ TgtChoose | RememberDiscarded$ True | SubAbility$ DBDamage\nSVar:DBDamage:DB$ DealDamage | Defined$ Player.IsRemembered | NumDmg$ 3 | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True",
    false
)]
#[case::the_player_alone("DB$ DealDamage | Defined$ Player.IsRemembered | NumDmg$ 3", true)]
fn a_remembered_player_is_not_the_cards_a_later_step_remembered(
    #[case] body: &str,
    #[case] read: bool,
) {
    let (_, found) = card(
        "Sorcery",
        &format!("A:SP$ RepeatEach | RepeatSubAbility$ D | RepeatPlayers$ Player\nSVar:D:{body}"),
    );
    assert_eq!(found.is_empty(), read, "{found:?}");
}

#[rstest]
#[case::mandatory("", "DB$ Draw | Defined$ You", "OncePerTurn")]
#[case::may("| OptionalDecider$ You", "DB$ Draw | Defined$ You", "OncePerTurnDone")]
#[case::pay_a_cost("", "AB$ Draw | Cost$ PayLife<1> | Defined$ You", "OncePerTurnDone")]
fn a_resolved_limit_is_spent_as_the_trigger_triggers_only_where_it_always_does_it(
    #[case] decider: &str,
    #[case] draw: &str,
    #[case] limit: &str,
) {
    let (json, found) = card(
        "Enchantment",
        &format!(
            "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.token+YouCtrl | TriggerZones$ Battlefield | Execute$ D {decider} | ResolvedLimit$ 1\nSVar:D:{draw}"
        ),
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(&format!(r#""limit":"{limit}""#)), "{json}");
}
