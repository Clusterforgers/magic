//! Goal: round 16 pack e16b bounds a target only a paid optional cost declares by that payment, reads "if you revealed a
//! Dragon card or controlled a Dragon as you cast this spell" into the reveal paid or a Dragon held as the spell was cast,
//! and leaves a presence the Oracle does not word and an opponent target where the card says "target player" but names
//! an opponent elsewhere a gap (a card that names no opponent widens the target to a player).
#![allow(clippy::unwrap_used)]

use rstest::rstest;
use serde_json::Value;

use super::{abilities, gaps};

/// An instant with the static line `line`, the spell lines `spell` and the Oracle text `oracle`.
fn card(line: &str, spell: &str, oracle: &str) -> String {
    format!("Name:Subject\nManaCost:1 R\nTypes:Instant\nS:{line}\n{spell}\nOracle:{oracle}\n")
}

const OPTIONAL: &str = "Mode$ OptionalCost | EffectZone$ All | ValidCard$ Card.Self | ValidSA$ Spell | Cost$ Sac<1/Artifact>";

#[test]
fn a_target_only_the_paid_spell_declares_is_chosen_only_when_it_was_paid() {
    let spell = "A:SP$ DealDamage | ValidTgts$ Any | NumDmg$ 2 | SubAbility$ DBDestroy\nSVar:DBDestroy:DB$ Destroy | ValidTgts$ Artifact | Condition$ OptionalCost | ConditionOptionalPaid$ True";
    let text = card(OPTIONAL, spell, "");
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    let found = serde_json::to_string(&abilities(&text)).unwrap();
    let bound = r#"{"When":{"condition":"AdditionalCostPaid","otherwise":0,"then":1}}"#;
    assert!(
        found.contains(&format!(r#""bounds":{{"max":{bound},"min":{bound}}}"#)),
        "{found}"
    );
}

#[rstest]
#[case::target_player("Target player discards a card.", false, Some("Player"))]
#[case::target_player_and_opponent(
    "Target player discards a card. Each opponent loses 1 life.",
    true,
    None
)]
#[case::target_opponent("Target opponent discards a card.", false, Some("Opponent"))]
#[case::no_words("", false, Some("Opponent"))]
fn an_opponent_target_where_the_card_says_target_player_is_a_gap(
    #[case] oracle: &str,
    #[case] gapped: bool,
    #[case] what: Option<&str>,
) {
    let spell = "A:SP$ Discard | ValidTgts$ Opponent | NumCards$ 1 | Mode$ TgtChoose";
    let text = card(OPTIONAL, spell, oracle);
    let found = gaps(&text);
    assert_eq!(
        found
            .iter()
            .any(|gap| gap == "targets:Opponent-where-the-card-says-target-player"),
        gapped,
        "{found:?}"
    );
    if let Some(what) = what {
        let read = serde_json::to_string(&abilities(&text)).unwrap();
        assert!(
            read.contains(&format!(r#""targets":[{{"what":"{what}"}}]"#)),
            "{read}"
        );
    }
}

const REVEAL: &str = "Mode$ OptionalCost | EffectZone$ All | ValidCard$ Card.Self | ValidSA$ Spell | Cost$ Reveal<1/Goblin>";
const PRESENCE: &str = r#"{"Any":["AdditionalCostPaid",{"AsCast":{"Compare":{"left":{"Count":[{"Subtype":"Goblin"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}}]}"#;
const GAIN: &str = "A:SP$ DealDamage | ValidTgts$ Any | NumDmg$ 2 | SubAbility$ DBGain\nSVar:DBGain:DB$ GainLife | LifeAmount$ 2 | ConditionCheckSVar$ Count$Presence_Goblin.1.0";

#[test]
fn a_revealed_or_controlled_subtype_is_the_reveal_paid_or_a_permanent_held_as_the_spell_was_cast() {
    let text = card(
        REVEAL,
        GAIN,
        "Subject deals 2 damage to any target. If you revealed a Goblin card or controlled a Goblin as you cast this spell, you gain 2 life.",
    );
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    let found = serde_json::to_string(&abilities(&text)).unwrap();
    let presence: Value = serde_json::from_str(PRESENCE).unwrap();
    assert!(
        found.contains(&format!(r#""condition":{presence}"#)),
        "{found}"
    );
}

#[test]
fn a_presence_the_oracle_does_not_word_is_a_gap() {
    let text = card(
        REVEAL,
        GAIN,
        "Subject deals 2 damage to any target. You gain 2 life.",
    );
    assert!(
        gaps(&text)
            .iter()
            .any(|gap| gap.contains("Presence_Goblin")),
        "{:?}",
        gaps(&text)
    );
}
