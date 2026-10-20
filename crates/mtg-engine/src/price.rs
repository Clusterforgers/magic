//! What a cost asks of a player: the `{T}` and the mana the engine pays, and the mana sources that may pay it.

use mtg_cards::cost::{Cost, CostItem};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use crate::mana::ManaOption;
use crate::priority::mana_options;
use crate::state::GameState;

/// The parts of a cost the engine pays today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Price {
    /// The source is tapped.
    pub(crate) tap: bool,
    /// Every mana part together; `None` for a cost without mana.
    pub(crate) mana: Option<ManaCost>,
}

/// The name `Caps` gives a cost part, for the parts the engine does not pay.
const fn unpaid(item: &CostItem) -> &'static str {
    match item {
        CostItem::Mana(_) => "Cost.Mana",
        CostItem::Tap => "Cost.Tap",
        CostItem::Untap => "Cost.Untap",
        CostItem::PayLife(_) => "Cost.PayLife",
        CostItem::SacrificeSelf => "Cost.SacrificeSelf",
        CostItem::Sacrifice { .. } => "Cost.Sacrifice",
        CostItem::Discard { .. } => "Cost.Discard",
    }
}

impl Price {
    /// The price of `cost`, or the name of the first part the engine does not pay: another kind of cost, or an
    /// `{X}`, which waits for the choice of X (`Cost.Mana.X`).
    pub(crate) fn of(cost: &Cost) -> Result<Price, &'static str> {
        let mut tap = false;
        let mut symbols = Vec::new();
        let mut mana = false;
        for item in &cost.0 {
            match item {
                CostItem::Tap => tap = true,
                CostItem::Mana(part) if part.has_x() => return Err("Cost.Mana.X"),
                CostItem::Mana(part) => {
                    mana = true;
                    symbols.extend_from_slice(part.rest().symbols());
                }
                other => return Err(unpaid(other)),
            }
        }
        Ok(Price {
            tap,
            mana: mana.then(|| ManaCost::from_symbols(symbols)),
        })
    }

    /// The mana abilities `player` may tap to pay the mana of an ability of `source`: the source itself is not one of
    /// them when its `{T}` is part of the cost, since a permanent is tapped once.
    pub(crate) fn sources(
        &self,
        state: &GameState,
        player: PlayerId,
        source: ObjectId,
    ) -> Vec<ManaOption> {
        let mut sources = mana_options(state, player);
        if self.tap {
            sources.retain(|option| option.source.object != source);
        }
        sources
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a cost is a price when every part of it is a `{T}` or mana (several mana parts add up), and any other part, or an `{X}`, is refused by the name `Caps` gives it; the mana sources of an ability that taps its source never include the source.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::color::Color;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::state;
    use crate::zones::Zone;

    fn cost(json: &str) -> Cost {
        serde_json::from_str(json).unwrap()
    }

    /// The mana of a price written as text, or none for an empty text.
    fn mana(text: &str) -> Option<ManaCost> {
        (!text.is_empty()).then(|| ManaCost::parse(text).unwrap())
    }

    #[rstest]
    #[case::free("[]", false, "")]
    #[case::tapping(r#"["Tap"]"#, true, "")]
    #[case::mana("\"{1}{R}\"", false, "{1}{R}")]
    #[case::mana_and_tap(r#"[{"Mana":"{2}"},"Tap"]"#, true, "{2}")]
    #[case::two_mana_parts_add_up(r#"[{"Mana":"{1}"},{"Mana":"{1}{G}"}]"#, false, "{2}{G}")]
    #[case::hybrid(r#""{G/W}{G/W}""#, false, "{G/W}{G/W}")]
    fn a_cost_of_tapping_and_mana_is_a_price(
        #[case] json: &str,
        #[case] tap: bool,
        #[case] expected: &str,
    ) {
        assert_eq!(
            Price::of(&cost(json)),
            Ok(Price {
                tap,
                mana: mana(expected)
            })
        );
    }

    #[rstest]
    #[case::life(r#"[{"Mana":"{1}"},{"PayLife":2}]"#, "Cost.PayLife")]
    #[case::sacrificing_the_source(r#"[{"Mana":"{2}"},"SacrificeSelf"]"#, "Cost.SacrificeSelf")]
    #[case::sacrificing(r#"[{"Sacrifice":{"filter":[]}}]"#, "Cost.Sacrifice")]
    #[case::discarding(r#"[{"Discard":{}}]"#, "Cost.Discard")]
    #[case::untapping(r#"["Untap"]"#, "Cost.Untap")]
    #[case::x(r#""{X}{R}""#, "Cost.Mana.X")]
    fn a_part_the_engine_does_not_pay_is_named(#[case] json: &str, #[case] name: &str) {
        assert_eq!(Price::of(&cost(json)), Err(name));
    }

    fn forest() -> Arc<CardDefinition> {
        Arc::new(CardDefinition::basic_land(Some(Color::Green)))
    }

    #[test]
    fn the_source_of_an_ability_that_taps_it_is_not_one_of_the_sources_of_its_mana() {
        let game = state(vec![vec![], vec![]], vec![vec![forest(), forest()], vec![]]);
        let rock = game.zone(Zone::Battlefield)[0];
        let sources = |price: &Price| -> Vec<ObjectId> {
            price
                .sources(&game, PlayerId(0), rock)
                .iter()
                .map(|option| option.source.object)
                .collect()
        };
        let tapping = Price::of(&cost(r#"[{"Mana":"{1}"},"Tap"]"#)).unwrap();
        let not_tapping = Price::of(&cost(r#""{1}""#)).unwrap();
        assert_eq!(sources(&not_tapping).len(), 2);
        assert_eq!(sources(&tapping).len(), 1);
        assert!(!sources(&tapping).contains(&rock));
    }
}
