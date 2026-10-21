//! Abilities: the rules kinds a card face is made of.

use serde::{Deserialize, Serialize};

use crate::card::Span;
use crate::cost::Cost;
use crate::effect::Effect;
use crate::keyword::Keyword;
use crate::mana::ManaCostX;
use crate::modify::{GameRule, Modification};
use crate::refs::{Ref, Selector, TargetSpec, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen, TriggerPattern};
use crate::value::{Amount, Condition};

/// When an activated ability may be activated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Timing {
    /// Whenever the player has priority.
    #[default]
    Instant,
    /// Only when the player could cast a sorcery.
    Sorcery,
}

impl Timing {
    /// Whether this is the default timing, so serialization can leave it out.
    pub const fn is_instant(&self) -> bool {
        matches!(self, Timing::Instant)
    }
}

/// A limit on how often an ability can be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Limit {
    /// Once each turn.
    OncePerTurn,
}

/// The loyalty cost of a planeswalker ability (CR 606.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum LoyaltyCost {
    /// Put this many loyalty counters on the planeswalker.
    Plus(u32),
    /// Remove this many.
    Minus(u32),
    /// Neither.
    Zero,
    /// Remove X loyalty counters.
    MinusX,
}

impl LoyaltyCost {
    /// The change the cost makes to the loyalty counters of its planeswalker: positive when it puts counters on, negative
    /// when it removes them. `None` for `-X`, whose X is chosen when the ability is announced.
    pub fn change(self) -> Option<i32> {
        match self {
            LoyaltyCost::Plus(n) => i32::try_from(n).ok(),
            LoyaltyCost::Minus(n) => i32::try_from(n).ok().map(i32::wrapping_neg),
            LoyaltyCost::Zero => Some(0),
            LoyaltyCost::MinusX => None,
        }
    }
}

/// Whether a cost modifier makes spells cost more or less.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CostChange {
    /// Costs more.
    Increase,
    /// Costs less (generic mana only, CR 601.2f).
    Reduce,
}

/// Whether a flag is off, so serialization can leave it out (serde calls it with a reference).
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false(flag: &bool) -> bool {
    !*flag
}

/// One ability of a card face.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Ability {
    /// What an instant or sorcery does as it resolves.
    Spell {
        /// The targets, chosen as the spell is cast.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        targets: Vec<TargetSpec>,
        /// What it does.
        effects: Vec<Effect>,
    },
    /// "Cost: effect" (CR 602). A mana ability is one with no targets that only adds mana.
    Activated {
        /// The cost.
        cost: Cost,
        /// When it may be activated.
        #[serde(default, skip_serializing_if = "Timing::is_instant")]
        timing: Timing,
        /// "Activate only if ..." (CR 602.5b): what must hold as it is activated.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        condition: Option<Condition>,
        /// How often.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<Limit>,
        /// The zone it can be activated from.
        #[serde(default, skip_serializing_if = "ZoneKind::is_battlefield")]
        from: ZoneKind,
        /// The targets, chosen as it is activated.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        targets: Vec<TargetSpec>,
        /// What it does.
        effects: Vec<Effect>,
    },
    /// A planeswalker's loyalty ability (CR 606).
    Loyalty {
        /// The change in loyalty.
        cost: LoyaltyCost,
        /// The targets.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        targets: Vec<TargetSpec>,
        /// What it does.
        effects: Vec<Effect>,
    },
    /// "When / whenever / at ..." (CR 603).
    Triggered {
        /// The event it waits for.
        when: TriggerPattern,
        /// The intervening "if" (CR 603.4), checked on trigger and on resolution.
        #[serde(rename = "if", default, skip_serializing_if = "Option::is_none")]
        intervening_if: Option<Condition>,
        /// How often it can trigger.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<Limit>,
        /// The zone the source must be in for it to trigger.
        #[serde(default, skip_serializing_if = "ZoneKind::is_battlefield")]
        from: ZoneKind,
        /// The targets, chosen as it is put on the stack.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        targets: Vec<TargetSpec>,
        /// What it does.
        effects: Vec<Effect>,
    },
    /// A static ability that changes objects continuously (CR 604, 613).
    Static {
        /// The objects it affects.
        affected: Ref,
        /// The changes.
        mods: Vec<Modification>,
        /// It applies only while this holds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        condition: Option<Condition>,
        /// Whether this is a characteristic-defining ability (CR 604.3): it defines the power and toughness, colours,
        /// types or subtypes of its own object, works in every zone and applies first in its layer.
        #[serde(default, skip_serializing_if = "is_false")]
        defining: bool,
        /// The zone the source must be in.
        #[serde(default, skip_serializing_if = "ZoneKind::is_battlefield")]
        from: ZoneKind,
    },
    /// A static ability that changes a rule of the game (CR 604): "players can't gain life", "this creature attacks
    /// each combat if able".
    Rule {
        /// The rule.
        rule: GameRule,
        /// It applies only while this holds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        condition: Option<Condition>,
        /// The zone the source must be in: the battlefield, or the stack for a spell that can't be countered.
        #[serde(default, skip_serializing_if = "ZoneKind::is_battlefield")]
        from: ZoneKind,
    },
    /// A replacement or prevention effect (CR 614, 615).
    Replacement {
        /// The event replaced.
        when: ReplaceWhen,
        /// What happens instead.
        by: ReplaceBy,
        /// It applies only while this holds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        condition: Option<Condition>,
        /// The zone the source must be in.
        #[serde(default, skip_serializing_if = "ZoneKind::is_battlefield")]
        from: ZoneKind,
    },
    /// Spells cost more or less to cast (CR 601.2f).
    CostModifier {
        /// The spells affected; `This` for the spell with this ability.
        applies_to: Selector,
        /// More or less.
        change: CostChange,
        /// The mana added or removed each time.
        mana: ManaCostX,
        /// How many times.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        per: Amount,
        /// It applies only while this holds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        condition: Option<Condition>,
    },
    /// A keyword ability; the engine knows what it does.
    Keyword(Keyword),
    /// A clause of the oracle text the data does not model; the table resolves it by hand.
    Manual {
        /// Why, in plain words.
        reason: String,
        /// Where the clause is in the face's oracle text.
        span: Span,
    },
}

impl Ability {
    /// Whether this is a clause left to the table.
    pub const fn is_manual(&self) -> bool {
        matches!(self, Ability::Manual { .. })
    }

    /// The targets the ability asks for, if its kind has any.
    pub fn targets(&self) -> &[TargetSpec] {
        match self {
            Ability::Spell { targets, .. }
            | Ability::Activated { targets, .. }
            | Ability::Loyalty { targets, .. }
            | Ability::Triggered { targets, .. } => targets,
            _ => &[],
        }
    }

    /// The effects it runs, if its kind has any.
    pub fn effects(&self) -> &[Effect] {
        match self {
            Ability::Spell { effects, .. }
            | Ability::Activated { effects, .. }
            | Ability::Loyalty { effects, .. }
            | Ability::Triggered { effects, .. } => effects,
            _ => &[],
        }
    }

    /// Whether this is an activated ability with no targets that only adds mana (CR 605.1a).
    pub fn is_mana_ability(&self) -> bool {
        match self {
            Ability::Activated {
                targets, effects, ..
            } => {
                targets.is_empty()
                    && !effects.is_empty()
                    && effects.iter().all(|e| matches!(e, Effect::AddMana { .. }))
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: abilities keep the compact JSON forms of the card files, mana abilities are recognised from data, and unknown fields are refused.
    use super::*;
    use rstest::rstest;

    use crate::effect::Produce;
    use mtg_core::mana::ManaType;

    fn add(mana: &[ManaType]) -> Effect {
        Effect::AddMana {
            who: Ref::You,
            mana: Produce::Fixed(mana.to_vec()),
            times: Amount::one(),
        }
    }

    #[test]
    fn a_tap_for_mana_ability_is_a_mana_ability() {
        let sol_ring = Ability::Activated {
            cost: Cost(vec![crate::cost::CostItem::Tap]),
            timing: Timing::Instant,
            condition: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: vec![],
            effects: vec![add(&[ManaType::Colorless, ManaType::Colorless])],
        };
        assert!(sol_ring.is_mana_ability());
        assert_eq!(
            serde_json::to_string(&sol_ring).unwrap(),
            r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless","Colorless"]}}}]}}"#
        );
    }

    #[rstest]
    #[case::with_a_target(r#"{"Activated":{"cost":["Tap"],"targets":[{"what":"Player"}],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#)]
    #[case::other_effect(r#"{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}]}}"#)]
    #[case::spell(r#"{"Spell":{"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#)]
    fn other_abilities_are_not_mana_abilities(#[case] json: &str) {
        assert!(
            !serde_json::from_str::<Ability>(json)
                .unwrap()
                .is_mana_ability()
        );
    }

    #[test]
    fn the_intervening_if_is_called_if_in_json() {
        let json = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":40}},"effects":[{"WinGame":{}}]}}"#;
        let ability: Ability = serde_json::from_str(json).unwrap();
        assert!(matches!(
            ability,
            Ability::Triggered {
                intervening_if: Some(_),
                ..
            }
        ));
        assert_eq!(serde_json::to_string(&ability).unwrap(), json);
    }

    #[test]
    fn loyalty_costs_read_as_plain_words() {
        let json = r#"{"Loyalty":{"cost":{"Plus":2},"effects":[{"Draw":{}}]}}"#;
        let ability: Ability = serde_json::from_str(json).unwrap();
        assert!(matches!(
            ability,
            Ability::Loyalty {
                cost: LoyaltyCost::Plus(2),
                ..
            }
        ));
        assert_eq!(
            serde_json::to_string(&LoyaltyCost::Zero).unwrap(),
            r#""Zero""#
        );
    }

    #[rstest]
    #[case::plus(LoyaltyCost::Plus(2), Some(2))]
    #[case::zero(LoyaltyCost::Zero, Some(0))]
    #[case::minus(LoyaltyCost::Minus(3), Some(-3))]
    #[case::minus_twelve(LoyaltyCost::Minus(12), Some(-12))]
    #[case::minus_x(LoyaltyCost::MinusX, None)]
    fn a_loyalty_cost_changes_the_counters_by_its_sign(
        #[case] cost: LoyaltyCost,
        #[case] change: Option<i32>,
    ) {
        assert_eq!(cost.change(), change);
    }

    #[test]
    fn unknown_fields_in_abilities_are_errors() {
        let error = serde_json::from_str::<Ability>(r#"{"Spell":{"effects":[],"cost":"{R}"}}"#)
            .unwrap_err()
            .to_string();
        assert!(error.contains("unknown field `cost`"), "{error}");
    }
}
