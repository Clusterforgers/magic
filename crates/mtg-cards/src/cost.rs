//! Costs: what a player pays to activate an ability, cast a spell a certain way, or avoid an effect.

use std::fmt;
use std::str::FromStr;

use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::mana::ManaCostX;
use crate::refs::Selector;
use crate::value::{Amount, CounterKind};

/// One part of a cost.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum CostItem {
    /// Mana, written as printed (`"{1}{R}"`); X is chosen by the payer.
    Mana(ManaCostX),
    /// Tap the source (`{T}`).
    Tap,
    /// Untap the source (`{Q}`).
    Untap,
    /// Pay life.
    PayLife(Amount),
    /// Sacrifice the source.
    SacrificeSelf,
    /// Exile the source from the zone it is in ("Exile this card from your graveyard").
    ExileSelf,
    /// Discard the source from the hand ("Discard this card").
    DiscardSelf,
    /// Sacrifice permanents the payer controls that match.
    Sacrifice {
        /// Which permanents may be chosen.
        filter: Selector,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// Discard cards from hand.
    Discard {
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
        /// Which cards may be chosen; any card when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filter: Option<Selector>,
    },
    /// Pay energy (`{E}`, CR 107.14): remove that many energy counters from the payer.
    PayEnergy(Amount),
    /// Remove counters of a kind from the source ("Remove a charge counter from this land").
    RemoveCounters {
        /// The kind of counter.
        counter: CounterKind,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// Tap untapped permanents the payer controls that match ("Tap an untapped creature you control").
    TapPermanents {
        /// Which permanents may be chosen.
        filter: Selector,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// Exile cards from the payer's graveyard that match.
    ExileFromGraveyard {
        /// Which cards may be chosen.
        filter: Selector,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// Return permanents the payer controls that match to their owner's hand.
    ReturnToHand {
        /// Which permanents may be chosen.
        filter: Selector,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// Exile cards from the payer's hand that match ("Exile a red card from your hand").
    ExileFromHand {
        /// Which cards may be chosen.
        filter: Selector,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
}

/// The parts of a cost, paid together; a cost of one mana part is written as its mana text.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Cost(pub Vec<CostItem>);

impl Cost {
    /// A cost of mana only.
    pub fn mana(cost: ManaCostX) -> Cost {
        Cost(vec![CostItem::Mana(cost)])
    }

    /// Whether any mana part holds an `{X}`, or any part counts the X announced ("Pay X life", "Sacrifice X creatures").
    pub fn has_x(&self) -> bool {
        self.0.iter().any(|item| match item {
            CostItem::Mana(mana) => mana.has_x(),
            CostItem::PayLife(amount) | CostItem::PayEnergy(amount) => *amount == Amount::X,
            CostItem::RemoveCounters { count, .. }
            | CostItem::Sacrifice { count, .. }
            | CostItem::Discard { count, .. }
            | CostItem::TapPermanents { count, .. }
            | CostItem::ExileFromGraveyard { count, .. }
            | CostItem::ExileFromHand { count, .. }
            | CostItem::ReturnToHand { count, .. } => *count == Amount::X,
            _ => false,
        })
    }
}

impl Serialize for Cost {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0.as_slice() {
            [CostItem::Mana(mana)] => serializer.serialize_str(&mana.to_string()),
            items => items.serialize(serializer),
        }
    }
}

struct CostVisitor;

impl<'de> Visitor<'de> for CostVisitor {
    type Value = Cost;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a mana cost such as \"{1}{R}\" or a list of cost parts")
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Cost, E> {
        ManaCostX::from_str(text)
            .map(Cost::mana)
            .map_err(|error| E::custom(error.to_string()))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Cost, A::Error> {
        Vec::<CostItem>::deserialize(de::value::SeqAccessDeserializer::new(seq)).map(Cost)
    }
}

impl<'de> Deserialize<'de> for Cost {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Cost, D::Error> {
        deserializer.deserialize_any(CostVisitor)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a cost of mana alone is its text, any other cost is a list of parts, and both read back identically.
    use super::*;
    use mtg_core::card::CardType;
    use rstest::rstest;

    use crate::refs::Pred;

    fn mana(text: &str) -> CostItem {
        CostItem::Mana(text.parse().unwrap())
    }

    #[rstest]
    #[case::mana_only(Cost(vec![mana("{2}{R}")]), r#""{2}{R}""#)]
    #[case::with_x(Cost(vec![mana("{X}{G}")]), r#""{X}{G}""#)]
    #[case::empty(Cost(vec![]), "[]")]
    #[case::tap_only(Cost(vec![CostItem::Tap]), r#"["Tap"]"#)]
    #[case::fetchland(
        Cost(vec![CostItem::Tap, CostItem::PayLife(Amount::Fixed(1)), CostItem::SacrificeSelf]),
        r#"["Tap",{"PayLife":1},"SacrificeSelf"]"#
    )]
    #[case::mana_and_tap(Cost(vec![mana("{2}"), CostItem::Tap]), r#"[{"Mana":"{2}"},"Tap"]"#)]
    #[case::exiling_the_source(
        Cost(vec![mana("{3}{G}{G}"), CostItem::ExileSelf]),
        r#"[{"Mana":"{3}{G}{G}"},"ExileSelf"]"#
    )]
    #[case::discarding_the_source(
        Cost(vec![mana("{U/R}{U/R}"), CostItem::DiscardSelf]),
        r#"[{"Mana":"{U/R}{U/R}"},"DiscardSelf"]"#
    )]
    #[case::sacrifice(
        Cost(vec![CostItem::Sacrifice { filter: Selector(vec![Pred::Type(CardType::Creature)]), count: Amount::Fixed(2) }]),
        r#"[{"Sacrifice":{"filter":[{"Type":"Creature"}],"count":2}}]"#
    )]
    fn costs_have_stable_json(#[case] value: Cost, #[case] json: &str) {
        assert_eq!(serde_json::to_string(&value).unwrap(), json);
        assert_eq!(serde_json::from_str::<Cost>(json).unwrap(), value);
    }

    #[rstest]
    #[case::number("3", "invalid type")]
    #[case::bad_symbol(r#""{S}""#, "unsupported mana symbol")]
    #[case::bad_part(r#"["Teleport"]"#, "unknown variant `Teleport`")]
    fn bad_costs_say_why(#[case] json: &str, #[case] expected: &str) {
        let error = serde_json::from_str::<Cost>(json).unwrap_err().to_string();
        assert!(error.contains(expected), "{error}");
    }

    #[test]
    fn x_is_found_in_mana_and_life_parts() {
        assert!(Cost(vec![mana("{X}{R}")]).has_x());
        assert!(Cost(vec![CostItem::PayLife(Amount::X)]).has_x());
        assert!(!Cost(vec![mana("{2}"), CostItem::Tap]).has_x());
    }
}
