//! The value of X (CR 601.2b, 107.3).
//!
//! A cost with `{X}` asks `ChooseNumber` for X, from 0 to the most the player can pay for with the rest of the total
//! cost; X is kept in `Choices.x`, adds that much generic mana for each `{X}` to the total cost and outlives the
//! announcement: the object on the stack reads it as `Amount::X` while it resolves. The `-X` of a loyalty ability asks
//! the same question, from 0 to the loyalty counters of the planeswalker, and removes that many.

use mtg_cards::ability::{Ability, LoyaltyCost};
use mtg_cards::value::CounterKind;
use mtg_core::mana::{ManaCost, ManaSymbol};

use super::{Ctx, Next, What, Work, parts, total};
use crate::choose::{Choices, NumberOptions};
use crate::decision::DecisionKind;
use crate::price::Price;
use crate::state::GameState;

/// The most the search for the largest X goes on: a payment this large is more mana than any game has.
const MOST_X: u32 = 1_000;

/// The X a spell or ability asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum XCost {
    /// An `{X}` in a mana cost; each costs X generic mana.
    Mana {
        /// How many `{X}` the cost holds.
        symbols: u32,
    },
    /// The `-X` of a loyalty ability: X loyalty counters are removed.
    Loyalty,
    /// The amount of a part of an ability's cost (life, counters, energy, objects picked): X of them are paid.
    Parts,
}

impl XCost {
    /// The X that `what` asks for, read from the cost printed on the card or in its rules: `rules` are the rules of
    /// the ability being announced.
    pub(crate) fn of(state: &GameState, what: &What, rules: Option<&Ability>) -> Option<XCost> {
        let symbols = match what {
            What::Spell(spell) => u32::from(state.object(*spell)?.card().x_in_cost),
            What::Ability(_) => match rules? {
                Ability::Activated { cost, .. } => {
                    let price = Price::announced(cost).ok()?;
                    if price.x == 0 && price.counted() {
                        return Some(XCost::Parts);
                    }
                    price.x
                }
                Ability::Loyalty {
                    cost: LoyaltyCost::MinusX,
                    ..
                } => return Some(XCost::Loyalty),
                _ => 0,
            },
            What::Trigger(_) => 0,
        };
        (symbols > 0).then_some(XCost::Mana { symbols })
    }
}

pub(super) fn run(ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
    if work.done.x {
        return None;
    }
    if ctx.needs.x.is_none() {
        work.done.x = true;
        return None;
    }
    // An alternative cost has no `{X}`: the one of the mana cost is 0 (CR 107.3b).
    if work.choices.way.is_some() {
        work.choices.x = Some(0);
        work.done.x = true;
        return None;
    }
    // Without a value of X that can be paid for there is nothing to announce.
    let Some(max) = largest(ctx, work.choices) else {
        return Some(Next::Drop);
    };
    // Nothing to choose between: X is 0.
    if max == 0 {
        work.choices.x = Some(0);
        work.done.x = true;
        return None;
    }
    Some(ctx.ask(DecisionKind::ChooseNumber(NumberOptions {
        source: ctx.source,
        min: 0,
        max: Some(max),
        text: "Choose a value for X.".to_owned(),
        can_cancel: ctx.can_cancel(),
    })))
}

/// `mana` with X paid: X generic mana for each `{X}` of the cost.
pub(super) fn add(ctx: &Ctx<'_>, choices: &Choices, mana: Option<ManaCost>) -> Option<ManaCost> {
    let Some(XCost::Mana { symbols }) = &ctx.needs.x else {
        return mana;
    };
    let generic = choices.x.unwrap_or(0).saturating_mul(*symbols);
    Some(
        mana.unwrap_or(ManaCost::ZERO)
            .with(ManaSymbol::Generic(generic)),
    )
}

/// The largest X the player can announce, or `None` when not even X = 0 can be paid.
fn largest(ctx: &Ctx<'_>, choices: &Choices) -> Option<u32> {
    match ctx.needs.x.as_ref()? {
        XCost::Loyalty => Some(
            ctx.state
                .object(ctx.source)?
                .permanent()?
                .counters_of(&CounterKind::Loyalty),
        ),
        XCost::Mana { .. } => {
            let payable = largest_payable(ctx, choices)?;
            Some(parts::most(ctx, choices).map_or(payable, |most| most.min(payable)))
        }
        XCost::Parts => parts::most(ctx, choices),
    }
}

/// The largest X whose total cost the player can pay. More X is more generic mana, so what can be paid for is a run of
/// values from 0, and a bisection finds where it ends.
fn largest_payable(ctx: &Ctx<'_>, choices: &Choices) -> Option<u32> {
    let mut trial = choices.clone();
    let mut payable = |x: u32| {
        trial.x = Some(x);
        total::of(ctx, &trial).is_some_and(|total| total::affordable(ctx, &total))
    };
    if !payable(0) {
        return None;
    }
    let (mut low, mut high) = (0, MOST_X);
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if payable(middle) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    Some(low)
}

#[cfg(test)]
mod tests {
    //! Goal: X is read from the card as the announcement begins (the `{X}` symbols of a spell, of the mana of an activated ability and the `-X` of a loyalty ability, nothing from a cost without one), and the most X offered is the most the total cost allows, for every number of `{X}`, of lands and of other symbols, and nothing is announced when not even X = 0 can be paid.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use mtg_core::color::Color;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::abilities::{AbilityRef, AbilitySource};
    use crate::announce::{Needs, Pending, announce};
    use crate::decision::Decision;
    use crate::fixtures::state;
    use crate::zones::Zone;

    const SEAT: PlayerId = PlayerId(0);

    fn forest() -> Arc<CardDefinition> {
        Arc::new(CardDefinition::basic_land(Some(Color::Green)))
    }

    fn spell(cost: &str, x: u8) -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Spell")
                .with_type(CardType::Instant)
                .with_mana_cost(ManaCost::parse(cost).unwrap())
                .with_x_in_cost(x),
        )
    }

    fn rules(json: &str) -> Ability {
        serde_json::from_str(json).unwrap()
    }

    #[rstest]
    #[case::no_x(0, None)]
    #[case::one(1, Some(XCost::Mana { symbols: 1 }))]
    #[case::two(2, Some(XCost::Mana { symbols: 2 }))]
    fn a_spell_asks_for_the_x_symbols_of_its_cost(#[case] x: u8, #[case] expected: Option<XCost>) {
        let game = state(vec![vec![spell("{G}", x)], vec![]], vec![vec![], vec![]]);
        let card = game.zone(Zone::Hand(SEAT))[0];
        assert_eq!(XCost::of(&game, &What::Spell(card), None), expected);
    }

    #[rstest]
    #[case::mana_without_x(r#"{"Activated":{"cost":"{1}{G}","effects":[{"Draw":{}}]}}"#, None)]
    #[case::a_tap(r#"{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}]}}"#, None)]
    #[case::x(
        r#"{"Activated":{"cost":"{X}","effects":[{"Draw":{}}]}}"#,
        Some(XCost::Mana { symbols: 1 })
    )]
    #[case::x_and_a_tap(
        r#"{"Activated":{"cost":[{"Mana":"{X}{G}"},"Tap"],"effects":[{"Draw":{}}]}}"#,
        Some(XCost::Mana { symbols: 1 })
    )]
    #[case::two_x(
        r#"{"Activated":{"cost":"{X}{X}","effects":[{"Draw":{}}]}}"#,
        Some(XCost::Mana { symbols: 2 })
    )]
    #[case::x_in_two_mana_parts(
        r#"{"Activated":{"cost":[{"Mana":"{X}"},{"Mana":"{X}{1}"}],"effects":[{"Draw":{}}]}}"#,
        Some(XCost::Mana { symbols: 2 })
    )]
    #[case::minus_x(
        r#"{"Loyalty":{"cost":"MinusX","effects":[{"Draw":{}}]}}"#,
        Some(XCost::Loyalty)
    )]
    #[case::minus_three(r#"{"Loyalty":{"cost":{"Minus":3},"effects":[{"Draw":{}}]}}"#, None)]
    #[case::a_spell_ability(r#"{"Spell":{"effects":[{"Draw":{}}]}}"#, None)]
    fn an_ability_asks_for_the_x_of_its_cost(#[case] json: &str, #[case] expected: Option<XCost>) {
        let game = state(vec![vec![], vec![]], vec![vec![forest()], vec![]]);
        let object = game.zone(Zone::Battlefield)[0];
        let what = What::Ability(AbilityRef {
            object,
            ability: 0,
            source: AbilitySource::Printed,
        });
        assert_eq!(XCost::of(&game, &what, Some(&rules(json))), expected);
    }

    /// What the cast of `cost` with `x` symbols of `{X}` and `lands` Forests asks first: the most X offered, or `None`
    /// when no number is asked or the cast is rewound.
    fn most(cost: &str, x: u8, lands: usize) -> Option<u32> {
        let game = state(
            vec![vec![spell(cost, x)], vec![]],
            vec![vec![forest(); lands], vec![]],
        );
        let card = game.zone(Zone::Hand(SEAT))[0];
        let needs = Needs {
            ways: Vec::new(),
            optional: Vec::new(),
            x: XCost::of(&game, &What::Spell(card), None),
            price: None,
        };
        let mut pending = Pending::needing(SEAT, What::Spell(card), needs);
        match announce(&game, &mut pending) {
            Next::Ask(Decision {
                kind: DecisionKind::ChooseNumber(options),
                ..
            }) => options.max,
            _ => None,
        }
    }

    #[rstest]
    #[case::one_x_and_a_colour("{G}", 1, 5, Some(4))]
    #[case::one_x_alone("", 1, 3, Some(3))]
    #[case::a_hybrid_beside_x("{G/W}", 1, 4, Some(3))]
    #[case::two_x_and_a_colour("{G}", 2, 5, Some(2))]
    #[case::two_x_and_generic("{2}{G}", 2, 7, Some(2))]
    #[case::three_x("", 3, 8, Some(2))]
    #[case::only_the_rest_can_be_paid("{G}", 1, 1, None)]
    #[case::two_x_and_only_the_rest("{G}", 2, 2, None)]
    #[case::too_little_for_the_rest("{2}{G}", 1, 2, None)]
    fn the_most_x_is_what_is_left_after_the_rest_of_the_cost(
        #[case] cost: &str,
        #[case] x: u8,
        #[case] lands: usize,
        #[case] expected: Option<u32>,
    ) {
        assert_eq!(most(cost, x, lands), expected);
    }

    #[test]
    fn a_cast_whose_rest_cannot_be_paid_is_rewound() {
        // {X}{2}{G} with two Forests: not even X = 0 is payable, so there is nothing to announce.
        let game = state(
            vec![vec![spell("{2}{G}", 1)], vec![]],
            vec![vec![forest(); 2], vec![]],
        );
        let card = game.zone(Zone::Hand(SEAT))[0];
        let needs = Needs {
            ways: Vec::new(),
            optional: Vec::new(),
            x: Some(XCost::Mana { symbols: 1 }),
            price: None,
        };
        let mut pending = Pending::needing(SEAT, What::Spell(card), needs);
        assert!(matches!(announce(&game, &mut pending), Next::Drop));
    }
}
