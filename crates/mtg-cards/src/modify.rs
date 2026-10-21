//! Modifications of objects and rules, and how long an effect lasts.

use std::sync::Arc;

use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;
use serde::{Deserialize, Serialize};

use crate::ability::Ability;
use crate::keyword::Keyword;
use crate::refs::{Ref, Selector};
use crate::trigger::TriggerPattern;
use crate::value::Amount;

/// A change to an object. Static abilities apply them continuously; `Effect::Continuous` applies
/// them for a duration. The layer (CR 613) follows from the kind.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Modification {
    /// Adds to power and toughness (layer 7c).
    ModifyPT {
        /// Power change.
        power: Amount,
        /// Toughness change.
        toughness: Amount,
    },
    /// Sets power and toughness (layer 7b).
    SetPT {
        /// New power.
        power: Amount,
        /// New toughness.
        toughness: Amount,
    },
    /// Switches power and toughness (layer 7d).
    SwitchPT,
    /// Adds a card type (layer 4).
    AddType(CardType),
    /// Removes a card type (layer 4).
    RemoveType(CardType),
    /// Adds a subtype (layer 4).
    AddSubtype(Subtype),
    /// Replaces the colors (layer 5).
    SetColors(Vec<Color>),
    /// Adds a keyword ability (layer 6).
    AddKeyword(Keyword),
    /// Adds protection from a colour the controller chooses as the effect begins (layer 6): "protection from the
    /// color of your choice". Only an effect of a spell or ability has it, never a static ability.
    AddProtectionFromChosenColor,
    /// Removes a keyword ability (layer 6).
    RemoveKeyword(Keyword),
    /// Gives the object an activated or a triggered ability (layer 6); a keyword is `AddKeyword`.
    GrantAbility(Arc<Ability>),
    /// The object loses all its abilities (layer 6); abilities granted after it still count.
    LoseAbilities,
    /// Changes the controller to the player (layer 2).
    SetController(Ref),
    /// The object can't attack (a restriction, no layer).
    CantAttack,
    /// The object can't block (a restriction, no layer).
    CantBlock,
    /// The object can't be blocked (a restriction, no layer).
    CantBeBlocked,
    /// The object can't be blocked by creatures the selector matches (a restriction, no layer).
    CantBeBlockedBy(Selector),
}

impl Modification {
    /// The layer of CR 613 this applies in, or `None` for a restriction that changes no characteristic.
    pub const fn layer(&self) -> Option<&'static str> {
        match self {
            Modification::SetController(_) => Some("2"),
            Modification::AddType(_)
            | Modification::RemoveType(_)
            | Modification::AddSubtype(_) => Some("4"),
            Modification::SetColors(_) => Some("5"),
            Modification::AddKeyword(_)
            | Modification::AddProtectionFromChosenColor
            | Modification::RemoveKeyword(_)
            | Modification::GrantAbility(_)
            | Modification::LoseAbilities => Some("6"),
            Modification::SetPT { .. } => Some("7b"),
            Modification::ModifyPT { .. } => Some("7c"),
            Modification::SwitchPT => Some("7d"),
            Modification::CantAttack
            | Modification::CantBlock
            | Modification::CantBeBlocked
            | Modification::CantBeBlockedBy(_) => None,
        }
    }
}

/// How long a continuous effect, rule or delayed trigger lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Duration {
    /// Until the cleanup step of this turn.
    UntilEndOfTurn,
    /// Until the end of combat.
    UntilEndOfCombat,
    /// Until the controller's next turn begins.
    UntilYourNextTurn,
    /// For as long as the source stays on the battlefield.
    WhileSourceOnBattlefield,
    /// Without end (until the object or the game is gone).
    Forever,
}

/// A change to the rules of the game rather than to an object. `Effect::Rule` makes it for a duration and
/// `Ability::Rule` for as long as its source has the ability; the references name the players or objects it is about.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GameRule {
    /// Damage from the sources can't be prevented, and prevention effects do nothing to it (CR 614.17).
    DamageCantBePrevented {
        /// The sources; any source when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        source: Selector,
        /// Only combat damage (`true`) or only other damage (`false`); both when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        combat: Option<bool>,
    },
    /// The creatures attack each combat if able: a requirement on the declaration (CR 508.1d), not a restriction.
    MustAttack(Ref),
    /// The creatures block each combat if able: a requirement on the declaration (CR 509.1c).
    MustBlock(Ref),
    /// The spells can't be countered.
    CantBeCountered(Ref),
    /// The players can't gain life.
    CantGainLife(Ref),
    /// The players can't draw cards.
    CantDraw(Ref),
    /// The players can't search libraries.
    CantSearch(Ref),
    /// The players play with the top card of their library revealed to every seat.
    RevealTop(Ref),
    /// The players may look at the top card of their own library any time; no one else sees it.
    LookAtTopAnyTime(Ref),
    /// The players have no maximum hand size (CR 402.2).
    NoMaxHandSize(Ref),
    /// The players may play additional lands each of their turns (CR 305.2).
    AdditionalLandPlays {
        /// Who may play them.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How many more lands each turn.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// The card can be a player's commander (CR 903.3), in every zone; the rule is written without `from`.
    CanBeCommander,
    /// A triggered ability of a permanent the rule's controller controls triggers an additional time when the event
    /// `cause` describes makes it trigger ("if a creature dying causes a triggered ability ... to trigger").
    TriggerAgain {
        /// The event that causes the trigger, as a triggered ability describes it.
        cause: TriggerPattern,
    },
}

#[cfg(test)]
mod tests {
    //! Goal: every modification belongs to the layer CR 613 gives it, and the JSON forms are stable.
    use super::*;
    use crate::ability::Timing;
    use crate::cost::Cost;
    use crate::refs::ZoneKind;
    use rstest::rstest;

    fn granted() -> Arc<Ability> {
        Arc::new(Ability::Activated {
            cost: Cost(Vec::new()),
            timing: Timing::Instant,
            condition: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: Vec::new(),
        })
    }

    #[rstest]
    #[case(Modification::SetController(Ref::You), Some("2"))]
    #[case(Modification::AddType(CardType::Artifact), Some("4"))]
    #[case(Modification::RemoveType(CardType::Creature), Some("4"))]
    #[case(Modification::GrantAbility(granted()), Some("6"))]
    #[case(Modification::LoseAbilities, Some("6"))]
    #[case(Modification::AddSubtype(Subtype("Elf".into())), Some("4"))]
    #[case(Modification::SetColors(vec![Color::Blue]), Some("5"))]
    #[case(Modification::AddKeyword(Keyword::Flying), Some("6"))]
    #[case(Modification::RemoveKeyword(Keyword::Flying), Some("6"))]
    #[case(Modification::SetPT { power: Amount::Fixed(1), toughness: Amount::Fixed(1) }, Some("7b"))]
    #[case(Modification::ModifyPT { power: Amount::Fixed(1), toughness: Amount::Fixed(1) }, Some("7c"))]
    #[case(Modification::SwitchPT, Some("7d"))]
    #[case(Modification::CantAttack, None)]
    #[case(Modification::CantBlock, None)]
    #[case(Modification::CantBeBlocked, None)]
    #[case(Modification::CantBeBlockedBy(Selector(vec![crate::refs::Pred::Type(CardType::Creature)])), None)]
    fn layers_follow_the_kind(#[case] modification: Modification, #[case] layer: Option<&str>) {
        assert_eq!(modification.layer(), layer);
        let json = serde_json::to_string(&modification).unwrap();
        assert_eq!(
            serde_json::from_str::<Modification>(&json).unwrap(),
            modification
        );
    }

    #[rstest]
    #[case(
        GameRule::DamageCantBePrevented { source: Selector::default(), combat: None },
        r#"{"DamageCantBePrevented":{}}"#
    )]
    #[case(
        GameRule::DamageCantBePrevented { source: Selector::this(), combat: Some(true) },
        r#"{"DamageCantBePrevented":{"source":["This"],"combat":true}}"#
    )]
    #[case(GameRule::MustAttack(Ref::This), r#"{"MustAttack":"This"}"#)]
    #[case(GameRule::MustBlock(Ref::This), r#"{"MustBlock":"This"}"#)]
    #[case(GameRule::CantBeCountered(Ref::This), r#"{"CantBeCountered":"This"}"#)]
    #[case(
        GameRule::CantGainLife(Ref::EachPlayer),
        r#"{"CantGainLife":"EachPlayer"}"#
    )]
    #[case(GameRule::CantDraw(Ref::Opponents), r#"{"CantDraw":"Opponents"}"#)]
    #[case(GameRule::CantSearch(Ref::You), r#"{"CantSearch":"You"}"#)]
    #[case(GameRule::RevealTop(Ref::You), r#"{"RevealTop":"You"}"#)]
    #[case(GameRule::LookAtTopAnyTime(Ref::You), r#"{"LookAtTopAnyTime":"You"}"#)]
    #[case(
        GameRule::NoMaxHandSize(Ref::EachPlayer),
        r#"{"NoMaxHandSize":"EachPlayer"}"#
    )]
    #[case(
        GameRule::AdditionalLandPlays { who: Ref::You, count: Amount::Fixed(1) },
        r#"{"AdditionalLandPlays":{}}"#
    )]
    #[case(
        GameRule::AdditionalLandPlays { who: Ref::EachPlayer, count: Amount::Fixed(2) },
        r#"{"AdditionalLandPlays":{"who":"EachPlayer","count":2}}"#
    )]
    #[case(GameRule::CanBeCommander, r#""CanBeCommander""#)]
    #[case(
        GameRule::TriggerAgain {
            cause: TriggerPattern::Attacks { what: Selector::this() }
        },
        r#"{"TriggerAgain":{"cause":{"Attacks":{"what":["This"]}}}}"#
    )]
    fn a_rule_is_written_with_the_players_or_objects_it_names(
        #[case] rule: GameRule,
        #[case] json: &str,
    ) {
        assert_eq!(serde_json::to_string(&rule).unwrap(), json);
        assert_eq!(serde_json::from_str::<GameRule>(json).unwrap(), rule);
    }

    #[test]
    fn a_pump_is_written_with_bare_numbers() {
        let pump = Modification::ModifyPT {
            power: Amount::Fixed(3),
            toughness: Amount::Fixed(3),
        };
        assert_eq!(
            serde_json::to_string(&pump).unwrap(),
            r#"{"ModifyPT":{"power":3,"toughness":3}}"#
        );
    }
}
