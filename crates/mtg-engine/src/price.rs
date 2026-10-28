//! What a cost asks of a player: the `{T}`, the sacrifice, exile or discard of the source, the mana and the other parts
//! of an ability's cost (`parts`) the engine pays, and the mana sources that may pay it.

use mtg_cards::cost::{Cost, CostItem};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use crate::mana::{ManaOption, reserve_energy};
use crate::parts::{Part, Payer};
use crate::priority::mana_options;
use crate::state::GameState;

/// What paying a cost does to its source besides tapping it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Leaves {
    /// The source stays where it is.
    #[default]
    Stays,
    /// It is sacrificed from the battlefield.
    Sacrificed,
    /// It is exiled from the zone it is in.
    Exiled,
    /// It is discarded from the hand.
    Discarded,
}

/// The parts of a cost the engine pays today.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Price {
    /// The source is tapped.
    pub(crate) tap: bool,
    /// What happens to the source.
    pub(crate) leaves: Leaves,
    /// Every mana part together, without its `{X}`; `None` for a cost without mana.
    pub(crate) mana: Option<ManaCost>,
    /// How many `{X}` the mana parts hold; each costs X generic mana once X is announced.
    pub(crate) x: u32,
    /// The other parts, in the order the cost prints them: life, counters removed, the objects the player picks.
    pub(crate) parts: Vec<Part>,
    /// The parts are paid for a spell being cast: the card itself is never picked for them.
    pub(crate) casting: bool,
    /// The changes the ability prints of its own cost (`CostItem::Changed`), applied to the mana as it is activated.
    pub(crate) changes: Vec<CostItem>,
}

/// The name `Caps` gives a cost part, for the parts the engine does not pay.
const fn unpaid(item: &CostItem) -> &'static str {
    match item {
        CostItem::Mana(_) => "Cost.Mana",
        CostItem::Tap => "Cost.Tap",
        CostItem::Untap => "Cost.Untap",
        CostItem::PayLife(_) => "Cost.PayLife",
        CostItem::SacrificeSelf => "Cost.SacrificeSelf",
        CostItem::ExileSelf => "Cost.ExileSelf",
        CostItem::DiscardSelf => "Cost.DiscardSelf",
        CostItem::Sacrifice { .. } => "Cost.Sacrifice",
        CostItem::Discard { .. } => "Cost.Discard",
        CostItem::PayEnergy(_) => "Cost.PayEnergy",
        CostItem::Mill(_) => "Cost.Mill",
        CostItem::RemoveCounters { .. } => "Cost.RemoveCounters",
        CostItem::TapPermanents { .. } => "Cost.TapPermanents",
        CostItem::TapPower { .. } => "Cost.TapPower",
        CostItem::ExileFromGraveyard { .. } => "Cost.ExileFromGraveyard",
        CostItem::ReturnToHand { .. } => "Cost.ReturnToHand",
        CostItem::ExileFromHand { .. } => "Cost.ExileFromHand",
        CostItem::RevealFromHand { .. } => "Cost.RevealFromHand",
        CostItem::OneOf(_) => "Cost.OneOf",
        CostItem::Changed { .. } => "Cost.Changed",
    }
}

impl Price {
    /// The price of a cost paid in the middle of a resolution, or the name of the first part the engine does not pay:
    /// another kind of cost, a part that needs the announcement of an ability to pick what it takes, an `{X}`
    /// (`Cost.Mana.X`; no announcement chooses it there) or a second way to lose the source.
    pub(crate) fn of(cost: &Cost) -> Result<Price, &'static str> {
        let price = Price::announced(cost)?;
        if let Some(part) = price.parts.first() {
            return Err(part.name());
        }
        if price.x > 0 {
            return Err("Cost.Mana.X");
        }
        if !price.changes.is_empty() {
            return Err("Cost.Changed");
        }
        Ok(price)
    }

    /// The price of the cost of an ability as it is announced, with the `{X}` it holds counted for the announcement
    /// to ask; the name of the first part the engine does not pay otherwise.
    pub(crate) fn announced(cost: &Cost) -> Result<Price, &'static str> {
        let mut tap = false;
        let mut leaves = Leaves::Stays;
        let mut symbols = Vec::new();
        let mut mana = false;
        let mut x = 0u32;
        let mut parts = Vec::new();
        let mut changes = Vec::new();
        for item in &cost.0 {
            let moves = match item {
                CostItem::SacrificeSelf => Some(Leaves::Sacrificed),
                CostItem::ExileSelf => Some(Leaves::Exiled),
                CostItem::DiscardSelf => Some(Leaves::Discarded),
                _ => None,
            };
            if let Some(moves) = moves {
                if leaves != Leaves::Stays && leaves != moves {
                    return Err(unpaid(item));
                }
                leaves = moves;
                continue;
            }
            match item {
                CostItem::Tap => tap = true,
                CostItem::Changed { .. } => changes.push(item.clone()),
                CostItem::Mana(part) => {
                    mana = true;
                    x = x.saturating_add(u32::from(part.x()));
                    symbols.extend_from_slice(part.rest().symbols());
                }
                other => match Part::read(other)? {
                    Some(part) => parts.push(part),
                    None => return Err(unpaid(other)),
                },
            }
        }
        Ok(Price {
            tap,
            leaves,
            mana: mana.then(|| ManaCost::from_symbols(symbols)),
            x,
            parts,
            casting: false,
            changes,
        })
    }

    /// The parts a spell pays besides its mana as it is cast: those of the way it is cast and of its optional costs.
    pub(crate) fn casting(parts: Vec<Part>) -> Price {
        Price {
            parts,
            casting: true,
            ..Price::default()
        }
    }

    /// The price with every counted part taking the X announced; X is 0 for an ability that announces none.
    pub(crate) fn at(&self, x: u32) -> Price {
        Price {
            parts: self.parts.iter().map(|part| part.at(x)).collect(),
            ..self.clone()
        }
    }

    /// Whether some part takes the X announced with the ability.
    pub(crate) fn counted(&self) -> bool {
        self.parts.iter().any(Part::is_counted)
    }

    /// Whether paying moves the source out of the zone it is in.
    pub(crate) fn moves_source(&self) -> bool {
        self.leaves != Leaves::Stays
    }

    /// The objects the pick of `part` cannot take: the source when the price already loses it, or taps it and the part
    /// taps permanents, and what `picked` holds, the picks made for the parts before.
    pub(crate) fn barred(
        &self,
        part: &Part,
        source: ObjectId,
        picked: &[Vec<ObjectId>],
    ) -> Vec<ObjectId> {
        let mut barred: Vec<ObjectId> = picked.iter().flatten().copied().collect();
        if self.casting || self.moves_source() || (self.tap && part.taps()) {
            barred.push(source);
        }
        barred
    }

    /// The objects the player would give for every part of the price beyond mana (CR 118.3), or `None` when some part
    /// cannot be paid now: each part takes what is left of the objects the parts before it would take, and leaves the
    /// permanents that make mana for the mana as far as it can.
    pub(crate) fn claimed(
        &self,
        state: &GameState,
        player: PlayerId,
        source: ObjectId,
    ) -> Option<Vec<ObjectId>> {
        self.claimed_leaving(state, player, source, &[])
    }

    /// [`Self::claimed`] that leaves the objects in `leave` (the creatures, artifacts and cards that help pay) as far
    /// as it can, as it does the permanents that make mana.
    pub(crate) fn claimed_leaving(
        &self,
        state: &GameState,
        player: PlayerId,
        source: ObjectId,
        leave: &[ObjectId],
    ) -> Option<Vec<ObjectId>> {
        let mut spare: Vec<ObjectId> = mana_options(state, player)
            .iter()
            .map(|option| option.source.object)
            .collect();
        spare.extend_from_slice(leave);
        let mut taken: Vec<Vec<ObjectId>> = Vec::new();
        for part in &self.parts {
            let barred = self.barred(part, source, &taken);
            let owner = Payer {
                state,
                player,
                source,
                barred: &barred,
            };
            taken.push(part.payable(&owner, &spare)?);
        }
        Some(taken.into_iter().flatten().collect())
    }

    /// The energy the parts of the price pay.
    pub(crate) fn energy(&self) -> u32 {
        self.parts
            .iter()
            .map(|part| match part {
                Part::Energy(amount) => *amount,
                _ => 0,
            })
            .sum()
    }

    /// The life the parts of the price pay.
    pub(crate) fn life(&self) -> u32 {
        self.parts
            .iter()
            .map(|part| match part {
                Part::Life(amount) => *amount,
                _ => 0,
            })
            .sum()
    }

    /// The mana abilities `player` may tap to pay the mana of an ability of `source`: the source itself is not one of
    /// them when its `{T}` is part of the cost, since a permanent is tapped once, or when it is sacrificed, exiled or
    /// discarded for it, since it is gone by the time the mana is paid.
    pub(crate) fn sources(
        &self,
        state: &GameState,
        player: PlayerId,
        source: ObjectId,
    ) -> Vec<ManaOption> {
        let mut sources = mana_options(state, player);
        if self.tap || self.moves_source() {
            crate::mana_takes::leave(&mut sources, &[source]);
        }
        // The energy the other parts of the price pay is not there for the mana abilities.
        reserve_energy(&mut sources, self.energy());
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
    #[case::free("[]", false, false, "")]
    #[case::tapping(r#"["Tap"]"#, true, false, "")]
    #[case::mana("\"{1}{R}\"", false, false, "{1}{R}")]
    #[case::mana_and_tap(r#"[{"Mana":"{2}"},"Tap"]"#, true, false, "{2}")]
    #[case::two_mana_parts_add_up(r#"[{"Mana":"{1}"},{"Mana":"{1}{G}"}]"#, false, false, "{2}{G}")]
    #[case::hybrid(r#""{G/W}{G/W}""#, false, false, "{G/W}{G/W}")]
    #[case::a_clue_pays_mana_and_sacrifices_itself(
        r#"[{"Mana":"{2}"},"SacrificeSelf"]"#,
        false,
        true,
        "{2}"
    )]
    #[case::food_pays_mana_taps_and_sacrifices_itself(
        r#"[{"Mana":"{2}"},"Tap","SacrificeSelf"]"#,
        true,
        true,
        "{2}"
    )]
    #[case::a_treasure_sacrifices_itself_for_nothing_else(
        r#"["Tap","SacrificeSelf"]"#,
        true,
        true,
        ""
    )]
    fn a_cost_of_tapping_sacrificing_the_source_and_mana_is_a_price(
        #[case] json: &str,
        #[case] tap: bool,
        #[case] sacrifice: bool,
        #[case] expected: &str,
    ) {
        assert_eq!(
            Price::of(&cost(json)),
            Ok(Price {
                tap,
                leaves: if sacrifice {
                    Leaves::Sacrificed
                } else {
                    Leaves::Stays
                },
                mana: mana(expected),
                x: 0,
                parts: Vec::new(),
                casting: false,
                changes: Vec::new(),
            })
        );
    }

    #[rstest]
    #[case::exiling_the_source(
        r#"[{"Mana":"{3}{G}{G}"},"ExileSelf"]"#,
        Leaves::Exiled,
        "{3}{G}{G}"
    )]
    #[case::exiling_the_source_for_free(r#"["ExileSelf"]"#, Leaves::Exiled, "")]
    #[case::discarding_the_source(
        r#"[{"Mana":"{U/R}{U/R}"},"DiscardSelf"]"#,
        Leaves::Discarded,
        "{U/R}{U/R}"
    )]
    fn a_cost_that_exiles_or_discards_the_source_is_a_price(
        #[case] json: &str,
        #[case] leaves: Leaves,
        #[case] expected: &str,
    ) {
        let price = Price::of(&cost(json)).unwrap();
        assert!(price.moves_source() && !price.tap);
        assert_eq!((price.leaves, price.mana), (leaves, mana(expected)));
    }

    #[test]
    fn a_source_cannot_be_lost_in_two_ways() {
        assert_eq!(
            Price::of(&cost(r#"["SacrificeSelf","ExileSelf"]"#)),
            Err("Cost.ExileSelf")
        );
        assert_eq!(
            Price::of(&cost(r#"["ExileSelf","ExileSelf"]"#)).map(|price| price.leaves),
            Ok(Leaves::Exiled)
        );
    }

    #[rstest]
    #[case::life(r#"[{"Mana":"{1}"},{"PayLife":2}]"#, "Cost.PayLife")]
    #[case::sacrificing_another_permanent(
        r#"[{"Mana":"{2}"},{"Sacrifice":{"filter":[]}}]"#,
        "Cost.Sacrifice"
    )]
    #[case::sacrificing(r#"[{"Sacrifice":{"filter":[]}}]"#, "Cost.Sacrifice")]
    #[case::discarding(r#"[{"Discard":{}}]"#, "Cost.Discard")]
    #[case::untapping(r#"["Untap"]"#, "Cost.Untap")]
    #[case::energy(r#"[{"PayEnergy":1}]"#, "Cost.PayEnergy")]
    #[case::removing_counters(
        r#"["Tap",{"RemoveCounters":{"counter":"charge"}}]"#,
        "Cost.RemoveCounters"
    )]
    #[case::tapping_other_permanents(
        r#"[{"TapPermanents":{"filter":[{"Type":"Creature"}]}}]"#,
        "Cost.TapPermanents"
    )]
    #[case::exiling_from_a_graveyard(
        r#"[{"ExileFromGraveyard":{"filter":[],"count":2}}]"#,
        "Cost.ExileFromGraveyard"
    )]
    #[case::returning_to_a_hand(
        r#"[{"ReturnToHand":{"filter":[{"Type":"Land"}]}}]"#,
        "Cost.ReturnToHand"
    )]
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

    #[rstest]
    #[case::sacrificed(r#"[{"Mana":"{1}"},"SacrificeSelf"]"#)]
    #[case::exiled(r#"[{"Mana":"{1}"},"ExileSelf"]"#)]
    #[case::discarded(r#"[{"Mana":"{1}"},"DiscardSelf"]"#)]
    fn the_source_of_an_ability_that_moves_it_is_not_one_of_the_sources_of_its_mana_either(
        #[case] json: &str,
    ) {
        let game = state(vec![vec![], vec![]], vec![vec![forest(), forest()], vec![]]);
        let rock = game.zone(Zone::Battlefield)[0];
        let price = Price::of(&cost(json)).unwrap();
        let sources: Vec<ObjectId> = price
            .sources(&game, PlayerId(0), rock)
            .iter()
            .map(|option| option.source.object)
            .collect();
        assert_eq!(sources.len(), 1);
        assert!(!sources.contains(&rock));
    }
}
