//! Patterns over game events: what triggers an ability and what a replacement effect replaces.

use serde::{Deserialize, Serialize};

use crate::effect::Effect;
use crate::refs::{Ref, Selector, ZoneKind};

/// A step or main phase of a turn, as triggers name it (the same thirteen as the engine's).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Step {
    /// Untap step.
    Untap,
    /// Upkeep step.
    Upkeep,
    /// Draw step.
    Draw,
    /// Precombat main phase.
    Main1,
    /// Beginning of combat step.
    BeginCombat,
    /// Declare attackers step.
    DeclareAttackers,
    /// Declare blockers step.
    DeclareBlockers,
    /// First-strike combat damage step.
    FirstStrikeDamage,
    /// Combat damage step.
    CombatDamage,
    /// End of combat step.
    EndCombat,
    /// Postcombat main phase.
    Main2,
    /// End step.
    End,
    /// Cleanup step.
    Cleanup,
}

/// The game event a triggered ability waits for (CR 603.2). Each variant is backed by one or more
/// engine events (`docs/cards-dsl.md`, Triggers).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum TriggerPattern {
    /// An object matching the selector moves between zones: enters (`to: Battlefield`), dies
    /// (`from: Battlefield, to: Graveyard`), leaves, is put into a graveyard, and so on.
    ZoneChange {
        /// The object that moves.
        what: Selector,
        /// The zone it leaves, if the pattern names one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<ZoneKind>,
        /// The zone it enters, if the pattern names one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<ZoneKind>,
    },
    /// A creature matching the selector attacks.
    Attacks {
        /// The attacker.
        what: Selector,
    },
    /// An object matching the selector becomes the target of a spell or ability.
    BecomesTarget {
        /// The object targeted.
        what: Selector,
        /// The spell or ability that targets it; anything when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        by: Selector,
    },
    /// A spell matching the selector is cast.
    SpellCast {
        /// The spell; any spell when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        what: Selector,
        /// Who casts it.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        by: Ref,
    },
    /// A permanent matching the selector is sacrificed.
    Sacrificed {
        /// The permanent.
        what: Selector,
        /// Who sacrifices it.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        by: Ref,
    },
    /// A step begins ("at the beginning of ...").
    StepBegins {
        /// The step.
        step: Step,
        /// Whose turn it is.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        whose: Ref,
    },
    /// Any of the patterns ("enters or attacks"); one event triggers the ability once.
    AnyOf(Vec<TriggerPattern>),
}

/// The event a replacement effect replaces (CR 614).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ReplaceWhen {
    /// An effect would create tokens under the player's control.
    TokenCreation {
        /// The player who would control them.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        controller: Ref,
    },
    /// Damage would be dealt to the player or object.
    Damage {
        /// The recipient.
        to: Ref,
        /// Only combat damage (`true`) or only other damage (`false`); both when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        combat: Option<bool>,
    },
    /// An object matching the selector would move between zones.
    ZoneChange {
        /// The object that would move.
        what: Selector,
        /// The zone it would leave, if the pattern names one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<ZoneKind>,
        /// The zone it would enter, if the pattern names one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<ZoneKind>,
    },
}

/// What happens instead.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ReplaceBy {
    /// The number or amount in the event is multiplied.
    Multiply(u32),
    /// The number or amount in the event is increased.
    Add(u32),
    /// The event does not happen (prevention, CR 615).
    Prevent,
    /// These effects happen instead.
    Instead(Vec<Effect>),
}

#[cfg(test)]
mod tests {
    //! Goal: patterns serialize compactly, leave out their defaults and refuse unknown fields.
    use super::*;
    use mtg_core::card::CardType;
    use rstest::rstest;

    use crate::refs::Pred;

    #[rstest]
    #[case::enters(
        TriggerPattern::ZoneChange { what: Selector::this(), from: None, to: Some(ZoneKind::Battlefield) },
        r#"{"ZoneChange":{"what":["This"],"to":"Battlefield"}}"#
    )]
    #[case::upkeep(
        TriggerPattern::StepBegins { step: Step::Upkeep, whose: Ref::You },
        r#"{"StepBegins":{"step":"Upkeep","whose":"You"}}"#
    )]
    #[case::any_upkeep(
        TriggerPattern::StepBegins { step: Step::End, whose: Ref::EachPlayer },
        r#"{"StepBegins":{"step":"End"}}"#
    )]
    #[case::opponent_casts(
        TriggerPattern::SpellCast { what: Selector::default(), by: Ref::Opponents },
        r#"{"SpellCast":{"by":"Opponents"}}"#
    )]
    #[case::either(
        TriggerPattern::AnyOf(vec![
            TriggerPattern::Attacks { what: Selector::this() },
            TriggerPattern::Sacrificed { what: Selector(vec![Pred::Type(CardType::Artifact)]), by: Ref::You },
        ]),
        r#"{"AnyOf":[{"Attacks":{"what":["This"]}},{"Sacrificed":{"what":[{"Type":"Artifact"}],"by":"You"}}]}"#
    )]
    fn patterns_have_stable_json(#[case] value: TriggerPattern, #[case] json: &str) {
        assert_eq!(serde_json::to_string(&value).unwrap(), json);
        assert_eq!(serde_json::from_str::<TriggerPattern>(json).unwrap(), value);
    }

    #[test]
    fn replacement_defaults_to_the_controller() {
        let when: ReplaceWhen = serde_json::from_str(r#"{"TokenCreation":{}}"#).unwrap();
        assert_eq!(
            when,
            ReplaceWhen::TokenCreation {
                controller: Ref::You
            }
        );
        assert_eq!(
            serde_json::to_string(&when).unwrap(),
            r#"{"TokenCreation":{}}"#
        );
    }

    #[test]
    fn an_unknown_field_in_a_struct_variant_is_an_error() {
        let error =
            serde_json::from_str::<TriggerPattern>(r#"{"Attacks":{"what":[],"alone":true}}"#)
                .unwrap_err()
                .to_string();
        assert!(error.contains("unknown field `alone`"), "{error}");
    }
}
