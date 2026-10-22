//! The value of X (CR 601.2b, 107.3).
//!
//! A cost with `{X}` asks `ChooseNumber` for X, from 0 to the most the player can pay for with the rest of the total
//! cost; X is kept in `Choices.x`, adds that much generic mana for each `{X}` to the total cost and outlives the
//! announcement: the object on the stack reads it as `Amount::X` while it resolves. The `-X` of a loyalty ability asks
//! the same question, from 0 to the loyalty counters of the planeswalker, and removes that many.

use mtg_cards::ability::{Ability, LoyaltyCost};
use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::{ManaCost, ManaSymbol};

use super::{Ctx, Next, What, Work, cheapest, parts, targets, total};
use crate::choose::{Choices, NumberOptions, enough_modes, modal_of, reads_x, targets_met};
use crate::decision::DecisionKind;
use crate::price::Price;
use crate::resolve::spell_ability;
use crate::state::GameState;
use crate::triggers::{PendingTrigger, x_read};

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
        // A trigger that reads the X of the spell or ability it came from remembers it as it goes on the stack.
        work.choices.x = ctx.triggered.and_then(remembered);
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
    // A value of X that leaves a target specification without a target cannot be announced (CR 601.2b, 601.2c).
    let Some(offer) = Offer::of(ctx, work.choices, max) else {
        return Some(Next::Drop);
    };
    // Nothing to choose between: X is the one value left.
    if offer.min == offer.max {
        work.choices.x = Some(offer.min);
        work.done.x = true;
        return None;
    }
    Some(ctx.ask(DecisionKind::ChooseNumber(NumberOptions {
        source: ctx.source,
        min: offer.min,
        max: Some(offer.max),
        except: offer.except,
        text: "Choose a value for X.".to_owned(),
        can_cancel: ctx.can_cancel(),
    })))
}

/// Whether `pays` accepts the total cost of casting the spell `id` for `cost` (the cost modifiers applied), choosing
/// the least X that leaves each target specification with a legal target (CR 601.2b, 601.2c): more X only costs more.
/// `fixed` is a cast that pays an alternative cost, where X is 0. A spell whose targets do not read X is paid for as it
/// is; for a modal one the X must leave enough modes with their targets (CR 700.2a).
pub(crate) fn payable(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    fixed: bool,
    cost: &ManaCost,
    pays: impl Fn(&ManaCost) -> bool,
) -> bool {
    let rules = spell_ability(state, id)
        .map(|(_, rules)| rules)
        .filter(|rules| targets_read_x(rules));
    let Some(rules) = rules else {
        return pays(&cheapest(state, player, id, cost));
    };
    let symbols = state
        .object(id)
        .map_or(0, |card| u32::from(card.card().x_in_cost));
    let most = if fixed || symbols == 0 { 0 } else { MOST_X };
    for x in 0..=most {
        let total = cost
            .clone()
            .with(ManaSymbol::Generic(x.saturating_mul(symbols)));
        if !pays(&cheapest(state, player, id, &total)) {
            return false;
        }
        if targets_met_at(state, id, player, &rules, x) {
            return true;
        }
    }
    false
}

/// Whether the ability `rules` of `source` can be announced with some X that `affords`: when what it may target reads X
/// (CR 601.2c), that X must leave every target specification a legal target, and a modal ability enough modes with
/// theirs. Abilities whose targets do not read X pass without asking `affords`.
pub(crate) fn some_x_has_targets(
    state: &GameState,
    player: PlayerId,
    source: ObjectId,
    rules: &Ability,
    affords: impl Fn(u32) -> bool,
) -> bool {
    if !targets_read_x(rules) {
        return true;
    }
    (0..=MOST_X)
        .take_while(|x| affords(*x))
        .any(|x| targets_met_at(state, source, player, rules, x))
}

/// Whether what `rules` may target reads X, in its own specifications or in those of any of its modes.
fn targets_read_x(rules: &Ability) -> bool {
    match modal_of(rules) {
        Some(modal) => modal
            .modes
            .iter()
            .any(|mode| mode.targets.iter().any(reads_x)),
        None => rules.targets().iter().any(reads_x),
    }
}

/// Whether `rules` has the targets it needs when X is `x`: every specification a legal target, and for a modal ability
/// enough modes for which that holds.
fn targets_met_at(
    state: &GameState,
    source: ObjectId,
    player: PlayerId,
    rules: &Ability,
    x: u32,
) -> bool {
    let Some(modal) = modal_of(rules) else {
        return targets_met(state, source, player, rules.targets(), None, Some(x));
    };
    let available = modal
        .modes
        .iter()
        .filter(|mode| targets_met(state, source, player, &mode.targets, None, Some(x)))
        .count();
    enough_modes(&modal, u32::try_from(available).unwrap_or(u32::MAX))
}

/// The values of X a player may announce: from `min` to `max`, less the `except` between them.
struct Offer {
    min: u32,
    max: u32,
    except: Vec<u32>,
}

impl Offer {
    /// The values of X from 0 to `most` for which every target specification can be met, or `None` when there are
    /// none. A target specification that does not read X is the same for every value.
    fn of(ctx: &Ctx<'_>, choices: &Choices, most: u32) -> Option<Offer> {
        let specs = targets::specs(ctx, choices);
        if !specs.iter().any(reads_x) {
            return Some(Offer {
                min: 0,
                max: most,
                except: Vec::new(),
            });
        }
        let met: Vec<u32> = (0..=most)
            .filter(|x| {
                targets_met(
                    ctx.state,
                    ctx.source,
                    ctx.player,
                    &specs,
                    ctx.cause(),
                    Some(*x),
                )
            })
            .collect();
        let (min, max) = (*met.first()?, *met.last()?);
        let except = (min..=max)
            .filter(|x| met.binary_search(x).is_err())
            .collect();
        Some(Offer { min, max, except })
    }
}

/// The X a waiting trigger reads (`triggers::x_read`): that of the spell or ability that set it up, of the spell its
/// permanent came from or of the spell cast; 0 for any other trigger.
fn remembered(waiting: &PendingTrigger) -> Option<u32> {
    let trigger = waiting.trigger();
    let Ability::Triggered { when, .. } = waiting.rules() else {
        return trigger.delayed.and(trigger.snapshot.x);
    };
    x_read(
        when,
        trigger.delayed.is_some(),
        &trigger.snapshot,
        &trigger.cause,
    )
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

    /// What the announcement does with X for a spell whose target is a creature with a mana value compared to X.
    #[derive(Debug, PartialEq, Eq)]
    enum Asked {
        /// A number between the bounds, less the numbers excepted.
        Number(u32, u32, Vec<u32>),
        /// X is the one value left and the targets are asked.
        Settled(u32),
        /// No value of X leaves a target: the cast is rewound.
        Rewound,
    }

    fn creature_costing(cost: &str) -> Arc<CardDefinition> {
        let card = CardDefinition::new(format!("Creature {cost}"))
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1);
        Arc::new(if cost.is_empty() {
            card
        } else {
            card.with_mana_cost(ManaCost::parse(cost).unwrap())
        })
    }

    /// The announcement of `{X}` for "destroy target creature with mana value `op` X" against creatures costing
    /// `costs` (an empty cost has mana value 0) when the player has `lands` Forests.
    fn asked(op: &str, costs: &[&str], lands: usize) -> Asked {
        let json = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"Compare":{"stat":"ManaValue","op":"OP","to":"X"}}]}}],"effects":[{"Destroy":{"what":{"Target":0},"regenerate":false}}]}}"#
            .replace("OP", op);
        let destroyer = Arc::new(
            CardDefinition::new("Destroyer")
                .with_type(CardType::Instant)
                .with_x_in_cost(1)
                .with_rules(rules(&json)),
        );
        let game = state(
            vec![vec![destroyer], vec![]],
            vec![
                vec![forest(); lands],
                costs.iter().map(|cost| creature_costing(cost)).collect(),
            ],
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
            }) => Asked::Number(options.min, options.max.unwrap(), options.except),
            Next::Ask(Decision {
                kind: DecisionKind::ChooseTargets(_),
                ..
            }) => Asked::Settled(pending.choices.x.unwrap()),
            Next::Drop => Asked::Rewound,
            _ => panic!("the announcement asked something else"),
        }
    }

    #[rstest]
    #[case::the_values_that_have_a_target("Eq", &["{1}", "{2}", "{4}"], 5, Asked::Number(1, 4, vec![3]))]
    #[case::far_apart("Eq", &["{1}", "{1}", "{6}"], 7, Asked::Number(1, 6, vec![2, 3, 4, 5]))]
    #[case::nothing_among_them("Eq", &["", "{2}"], 3, Asked::Number(0, 2, vec![1]))]
    #[case::one_value_is_no_question("Eq", &["{2}", "{2}"], 4, Asked::Settled(2))]
    #[case::the_most_that_can_be_paid("Eq", &["{5}"], 5, Asked::Settled(5))]
    #[case::a_value_that_cannot_be_paid_is_not_offered("Eq", &["{1}", "{6}"], 4, Asked::Settled(1))]
    #[case::only_nothing("Eq", &[""], 3, Asked::Settled(0))]
    #[case::no_value_has_a_target("Eq", &["{5}"], 4, Asked::Rewound)]
    #[case::no_creature_at_all("Eq", &[], 3, Asked::Rewound)]
    #[case::x_or_less_from_the_least("Le", &["{3}", "{4}"], 5, Asked::Number(3, 5, vec![]))]
    #[case::x_or_less_that_cannot_be_paid("Le", &["{4}"], 3, Asked::Rewound)]
    #[case::x_or_less_with_a_free_creature("Le", &["", "{4}"], 3, Asked::Number(0, 3, vec![]))]
    fn only_the_values_of_x_with_a_legal_target_are_offered(
        #[case] op: &str,
        #[case] creatures: &[&str],
        #[case] lands: usize,
        #[case] expected: Asked,
    ) {
        assert_eq!(asked(op, creatures, lands), expected);
    }
}
