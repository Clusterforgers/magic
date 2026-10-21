//! Goal: the announcement of a spell walks the steps of CR 601.2b to 601.2h in their order whatever the spell asks for (modes, optional costs, X, targets, a division, the payment), passes over the steps it has no question for, adds the optional costs and X to the cost it asks the player to pay, keeps every answer in the choices of the cast, and rewinds the whole announcement, with nothing paid or logged, when a step cannot be met.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CastWay;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;
use proptest::prelude::*;
use rstest::rstest;

use super::{Answered, Needs, Next, Optional, Pending, Phase, What, XCost, announce};
use crate::choose::{ChoiceOption, Target};
use crate::decision::{Decision, DecisionKind, PayOptions};
use crate::fixtures::{creature, state};
use crate::mana::can_afford;
use crate::object::ObjectRef;
use crate::state::GameState;
use crate::ways::Way;
use crate::zones::Zone;

const SEAT: PlayerId = PlayerId(0);

/// What a spell asks of its caster besides the payment.
#[derive(Debug, Clone, Copy)]
enum Shape {
    /// No target.
    Plain,
    /// One target of any kind.
    Targeted,
    /// One to three targets of any kind that share 3.
    Divided,
    /// One to two targets that share 1, so two targets cannot each have one.
    Stingy,
    /// A target creature.
    Creature,
    /// Choose one: draw, or the divided spell.
    Modal,
}

fn spell(shape: Shape) -> Arc<CardDefinition> {
    let divided = r#"{"what":"AnyTarget","bounds":{"min":1,"max":3},"divide":3}"#;
    let ability = match shape {
        Shape::Plain => r#"{"Spell":{"effects":[{"Draw":{}}]}}"#.to_owned(),
        Shape::Targeted => {
            r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"Draw":{}}]}}"#.to_owned()
        }
        Shape::Divided => {
            format!(r#"{{"Spell":{{"targets":[{divided}],"effects":[{{"Draw":{{}}}}]}}}}"#)
        }
        Shape::Stingy => r#"{"Spell":{"targets":[{"what":"AnyTarget","bounds":{"min":1,"max":2},"divide":1}],"effects":[{"Draw":{}}]}}"#.to_owned(),
        Shape::Creature => r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Draw":{}}]}}"#.to_owned(),
        Shape::Modal => format!(
            r#"{{"Spell":{{"effects":[{{"Modal":{{"choose":{{"min":1,"max":1}},"repeat":false,"modes":[
                {{"label":"Cards","effects":[{{"Draw":{{}}}}]}},
                {{"label":"Split","targets":[{divided}],"effects":[{{"Draw":{{}}}}]}}]}}}}]}}}}"#
        ),
    };
    Arc::new(
        CardDefinition::new("Spell")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse("{1}").unwrap())
            .with_rules(serde_json::from_str(&ability).unwrap()),
    )
}

fn forests(count: usize) -> Vec<Arc<CardDefinition>> {
    (0..count)
        .map(|_| {
            Arc::new(CardDefinition::basic_land(Some(
                mtg_core::color::Color::Green,
            )))
        })
        .collect()
}

/// Seat 0 holds the spell and controls `lands` Forests; seat 1 controls a creature.
fn table(shape: Shape, lands: usize) -> GameState {
    state(
        vec![vec![spell(shape)], vec![]],
        vec![forests(lands), vec![creature("Bear", 2, 2)]],
    )
}

/// [`table`] with no creature anywhere.
fn bare_table(shape: Shape, lands: usize) -> GameState {
    state(
        vec![vec![spell(shape)], vec![]],
        vec![forests(lands), vec![]],
    )
}

/// A kicker of `{1}`.
fn kicker() -> Optional {
    Optional {
        label: "Kicker {1}".to_owned(),
        mana: ManaCost::parse("{1}").unwrap(),
        repeatable: false,
    }
}

fn needs(kicked: bool, x: u32) -> Needs {
    Needs {
        ways: Vec::new(),
        optional: if kicked { vec![kicker()] } else { Vec::new() },
        x: (x > 0).then_some(XCost::Mana { symbols: x }),
    }
}

fn pending(game: &GameState, needs: Needs) -> Pending {
    let card = game.zone(Zone::Hand(SEAT))[0];
    Pending::needing(SEAT, What::Spell(card), needs)
}

/// How a walk through the announcement ended.
#[derive(Debug)]
enum End {
    /// The payment is asked.
    Paying(PayOptions),
    /// The announcement was rewound.
    Rewound,
    /// The caster backed out.
    Cancelled,
}

/// Answers the announcement's questions with `pick` until it asks for the payment or ends; the names of the
/// decisions asked come back with the end.
fn walk(
    game: &GameState,
    pending: &mut Pending,
    mut pick: impl FnMut(&DecisionKind) -> Option<Answered>,
) -> (Vec<&'static str>, End) {
    let mut asked = Vec::new();
    for _ in 0..16 {
        match announce(game, pending) {
            Next::Ask(Decision {
                kind: DecisionKind::PayMana(options),
                ..
            }) => {
                asked.push("pay-mana");
                return (asked, End::Paying(options));
            }
            Next::Ask(Decision { kind, .. }) => {
                asked.push(kind.name());
                match pick(&kind) {
                    Some(answer) => pending.accept(answer).unwrap(),
                    None => return (asked, End::Cancelled),
                }
            }
            Next::Commit(events) => panic!("a spell is paid for first: {events:?}"),
            Next::Drop => return (asked, End::Rewound),
        }
    }
    panic!("the announcement asked more than sixteen questions: {asked:?}");
}

/// The answers a player who wants everything would give: the last mode, every optional cost, the most X, as many
/// targets as allowed and one point to each target but the first.
#[allow(clippy::unnecessary_wraps)] // the shape `walk` takes: a caster who backs out answers with nothing
fn greedy(kind: &DecisionKind) -> Option<Answered> {
    Some(match kind {
        DecisionKind::ChooseMode(o) => Answered::Modes(vec![o.modes.last().unwrap().index]),
        DecisionKind::ChooseCosts(o) => {
            Answered::Costs(o.optional.iter().map(|cost| cost.index).collect())
        }
        DecisionKind::ChooseNumber(o) => Answered::Number(o.max.unwrap_or(o.min)),
        DecisionKind::ChooseTargets(o) => Answered::Targets(
            o.legal
                .iter()
                .copied()
                .take(o.max as usize)
                .collect::<Vec<Target>>(),
        ),
        DecisionKind::DivideAmount(o) => {
            let rest = u32::try_from(o.among.len() - 1).unwrap();
            Answered::Division(
                o.among
                    .iter()
                    .enumerate()
                    .map(|(at, target)| (*target, if at == 0 { o.total - rest } else { 1 }))
                    .collect(),
            )
        }
        other => panic!("{} was not expected", other.name()),
    })
}

#[test]
fn the_steps_are_listed_in_the_order_of_the_rules() {
    let rules: Vec<char> = Phase::ORDER.iter().map(|phase| phase.rule()).collect();
    assert_eq!(rules, ['b', 'b', 'b', 'b', 'c', 'd', 'e', 'f', 'g']);
    assert_eq!(Phase::ORDER[0], Phase::Modes);
    assert_eq!(Phase::ORDER[8], Phase::Payment);
    let mut seen = Phase::ORDER.to_vec();
    seen.dedup();
    assert_eq!(seen.len(), Phase::ORDER.len(), "each step once");
}

#[rstest]
#[case::everything(Shape::Modal, true, 1, &["choose-mode", "choose-costs", "choose-number", "choose-targets", "divide-amount", "pay-mana"])]
#[case::no_modes(Shape::Divided, true, 1, &["choose-costs", "choose-number", "choose-targets", "divide-amount", "pay-mana"])]
#[case::no_optional_cost(Shape::Divided, false, 1, &["choose-number", "choose-targets", "divide-amount", "pay-mana"])]
#[case::no_x(Shape::Divided, true, 0, &["choose-costs", "choose-targets", "divide-amount", "pay-mana"])]
#[case::a_modal_spell_alone(Shape::Modal, false, 0, &["choose-mode", "choose-targets", "divide-amount", "pay-mana"])]
#[case::targets_without_a_division(Shape::Targeted, true, 0, &["choose-costs", "choose-targets", "pay-mana"])]
#[case::only_x(Shape::Plain, false, 1, &["choose-number", "pay-mana"])]
#[case::only_an_optional_cost(Shape::Plain, true, 0, &["choose-costs", "pay-mana"])]
#[case::nothing_but_the_payment(Shape::Plain, false, 0, &["pay-mana"])]
fn the_questions_come_in_the_order_of_the_rules_and_a_step_with_none_is_passed_over(
    #[case] shape: Shape,
    #[case] kicked: bool,
    #[case] x: u32,
    #[case] expected: &[&str],
) {
    let game = table(shape, 6);
    let mut pending = pending(&game, needs(kicked, x));
    let (asked, end) = walk(&game, &mut pending, greedy);
    assert_eq!(asked, expected);
    assert!(matches!(end, End::Paying(_)), "{end:?}");
}

/// Four Forests pay for `{1}`, the kicker `{1}` and X once for each `{X}` of the cost, so X is offered up to what is left.
#[rstest]
#[case::the_printed_cost_and_x_of_nothing(false, 1, 0, 1, 3)]
#[case::an_optional_cost_adds_its_mana_and_leaves_less_for_x(true, 1, 0, 2, 2)]
#[case::x_adds_generic_mana(false, 1, 2, 3, 3)]
#[case::both(true, 1, 2, 4, 2)]
#[case::each_x_costs_x(false, 2, 1, 3, 1)]
#[case::each_x_and_an_optional_cost(true, 2, 1, 4, 1)]
fn the_total_cost_is_the_cost_with_the_optional_costs_and_x_added(
    #[case] kicked: bool,
    #[case] symbols: u32,
    #[case] x: u32,
    #[case] total: u32,
    #[case] most_x: u32,
) {
    let game = table(Shape::Plain, 4);
    let mut pending = pending(&game, needs(kicked, symbols));
    let mut offered = None;
    let (_, end) = walk(&game, &mut pending, |kind| {
        if let DecisionKind::ChooseNumber(o) = kind {
            offered = Some((o.min, o.max, o.can_cancel));
            return Some(Answered::Number(x));
        }
        greedy(kind)
    });
    let End::Paying(options) = end else {
        panic!("the payment was expected, got {end:?}");
    };
    assert_eq!(options.cost.generic(), total);
    assert_eq!(offered, Some((0, Some(most_x), true)));
    assert_eq!(pending.choices.x, Some(x), "X outlives the spell");
    assert_eq!(
        pending.choices.optional_costs,
        if kicked { vec![0] } else { Vec::new() }
    );
}

/// A way to cast the spell: its printed cost of `{1}`, or an alternative cost.
fn way(alternative: Option<CastWay>, cost: Option<&str>) -> Way {
    Way {
        alternative,
        label: cost.map_or("Cast for {1}".to_owned(), |cost| {
            format!("Alternative {cost}")
        }),
        mana: cost.map(|cost| ManaCost::parse(cost).unwrap()),
        instant: true,
    }
}

#[rstest]
#[case::an_alternative_cost_replaces_the_printed_one(&[(Some(CastWay::Evoke), Some("{2}"))], &["pay-mana"], 2, Some(CastWay::Evoke))]
#[case::a_free_one_leaves_nothing_to_pay(&[(Some(CastWay::Free), Some("{0}"))], &["pay-mana"], 0, Some(CastWay::Free))]
#[case::the_printed_way_alone_is_not_asked(&[(None, None)], &["pay-mana"], 1, None)]
#[case::two_ways_are_asked_and_the_first_is_taken(&[(None, None), (Some(CastWay::Evoke), Some("{2}"))], &["choose-option", "pay-mana"], 1, None)]
#[case::an_alternative_that_cannot_be_paid_is_not_offered(&[(None, None), (Some(CastWay::Overload), Some("{9}"))], &["pay-mana"], 1, None)]
#[case::an_alternative_that_cannot_be_paid_leaves_the_other(&[(Some(CastWay::Flashback), Some("{9}")), (Some(CastWay::Free), Some("{0}"))], &["pay-mana"], 0, Some(CastWay::Free))]
fn the_way_the_spell_is_cast_decides_the_cost_it_pays(
    #[case] ways: &[(Option<CastWay>, Option<&str>)],
    #[case] asked_for: &[&str],
    #[case] paid: u32,
    #[case] chosen: Option<CastWay>,
) {
    let game = table(Shape::Plain, 4);
    let mut pending = pending(
        &game,
        Needs {
            ways: ways
                .iter()
                .map(|(way_of, cost)| way(*way_of, *cost))
                .collect(),
            ..Needs::default()
        },
    );
    let (asked, end) = walk(&game, &mut pending, |kind| match kind {
        DecisionKind::ChooseOption(_) => Some(Answered::Way(0)),
        other => greedy(other),
    });
    assert_eq!(asked, asked_for);
    let End::Paying(options) = end else {
        panic!("{end:?}");
    };
    assert_eq!(options.cost.generic(), paid);
    assert_eq!(pending.choices.way, chosen, "the way is remembered");
}

#[test]
fn the_second_way_is_taken_when_the_player_picks_it() {
    let game = table(Shape::Plain, 4);
    let mut pending = pending(
        &game,
        Needs {
            ways: vec![way(None, None), way(Some(CastWay::Evoke), Some("{2}"))],
            ..Needs::default()
        },
    );
    let (asked, end) = walk(&game, &mut pending, |kind| match kind {
        DecisionKind::ChooseOption(options) => {
            assert_eq!(
                options.options,
                [
                    ChoiceOption::Label("Cast for {1}".into()),
                    ChoiceOption::Label("Alternative {2}".into())
                ]
            );
            Some(Answered::Way(1))
        }
        other => greedy(other),
    });
    assert_eq!(asked, ["choose-option", "pay-mana"]);
    let End::Paying(options) = end else {
        panic!("{end:?}");
    };
    assert_eq!(options.cost.generic(), 2);
    assert_eq!(pending.choices.way, Some(CastWay::Evoke));
}

#[test]
fn no_way_that_can_be_used_rewinds_the_announcement_and_an_unasked_way_is_refused() {
    let game = table(Shape::Plain, 1);
    let mut pending = pending(
        &game,
        Needs {
            ways: vec![way(Some(CastWay::Overload), Some("{9}"))],
            ..Needs::default()
        },
    );
    let (asked, end) = walk(&game, &mut pending, greedy);
    assert!(asked.is_empty() && matches!(end, End::Rewound), "{end:?}");
    assert!(pending.accept(Answered::Way(0)).is_err());
}

#[test]
fn an_overloaded_spell_asks_for_no_targets() {
    let game = table(Shape::Creature, 4);
    let mut pending = pending(
        &game,
        Needs {
            ways: vec![way(Some(CastWay::Overload), Some("{2}"))],
            ..Needs::default()
        },
    );
    let (asked, end) = walk(&game, &mut pending, greedy);
    assert_eq!(asked, ["pay-mana"]);
    assert!(matches!(end, End::Paying(_)));
    assert_eq!(pending.choices.targets, Vec::<Vec<Target>>::new());
}

#[rstest]
#[case::a_target_that_cannot_be_met(Shape::Creature, 0, needs(false, 0), Vec::new())]
#[case::x_that_cannot_be_paid_even_as_nothing(Shape::Plain, 0, needs(false, 1), Vec::new())]
#[case::a_cost_that_cannot_be_paid(Shape::Plain, 0, needs(false, 0), Vec::new())]
#[case::a_division_that_cannot_give_each_target_one(Shape::Stingy, 3, needs(false, 0), Vec::new())]
fn a_step_that_cannot_be_met_rewinds_the_announcement(
    #[case] shape: Shape,
    #[case] lands: usize,
    #[case] needs: Needs,
    #[case] kicked: Vec<usize>,
) {
    let game = bare_table(shape, lands);
    let mut pending = pending(&game, needs);
    let (_, end) = walk(&game, &mut pending, |kind| match kind {
        // Two targets for a division of 1: the answer is legal for the specification, not for the division.
        DecisionKind::ChooseTargets(o) if matches!(shape, Shape::Stingy) => {
            Some(Answered::Targets(o.legal.iter().copied().take(2).collect()))
        }
        DecisionKind::ChooseCosts(_) => Some(Answered::Costs(kicked.clone())),
        other => greedy(other),
    });
    assert!(matches!(end, End::Rewound), "{end:?}");
    // Nothing was paid: the announcement only ever looked at the game.
    assert_eq!(game.zone(Zone::Hand(SEAT)).len(), 1);
}

#[test]
fn a_division_that_gives_each_target_one_is_asked_only_when_the_amount_allows_it() {
    let game = table(Shape::Stingy, 3);
    let mut pending = pending(&game, Needs::default());
    // The specification allows two targets, but there is only 1 to share: one target is all that is offered.
    let (asked, end) = walk(&game, &mut pending, greedy);
    assert_eq!(asked, ["choose-targets", "pay-mana"]);
    assert!(matches!(end, End::Paying(_)));
    assert_eq!(pending.choices.targets[0].len(), 1);
    assert_eq!(pending.choices.division.len(), 1);
    assert_eq!(pending.choices.division[0].1, 1);
}

#[test]
fn a_target_that_is_no_longer_legal_rewinds_the_announcement_at_the_check() {
    let game = table(Shape::Targeted, 3);
    let mut pending = pending(&game, Needs::default());
    let gone = Target::Object(ObjectRef {
        id: ObjectId(9_999),
        counter: 0,
    });
    pending.choices.targets.push(vec![gone]);
    assert!(matches!(announce(&game, &mut pending), Next::Drop));
}

#[test]
fn an_answer_is_taken_only_for_the_step_that_asked() {
    let game = table(Shape::Plain, 3);
    let mut pending = pending(&game, needs(true, 1));
    assert!(
        pending.accept(Answered::Number(1)).is_err(),
        "nothing was asked yet"
    );
    let Next::Ask(Decision {
        kind: DecisionKind::ChooseCosts(_),
        ..
    }) = announce(&game, &mut pending)
    else {
        panic!("the optional costs come first");
    };
    assert!(
        pending.accept(Answered::Number(1)).is_err(),
        "X waits for the costs"
    );
    pending.accept(Answered::Costs(vec![0])).unwrap();
    assert_eq!(pending.choices.optional_costs, [0]);
    assert!(
        pending.accept(Answered::Costs(vec![0])).is_err(),
        "answered once"
    );
    let Next::Ask(Decision {
        kind: DecisionKind::ChooseNumber(_),
        ..
    }) = announce(&game, &mut pending)
    else {
        panic!("X comes after the costs");
    };
}

#[rstest]
#[case::at_the_modes("choose-mode", false, false, false, false)]
#[case::at_the_optional_costs("choose-costs", true, false, false, false)]
#[case::at_x("choose-number", true, true, false, false)]
#[case::at_the_targets("choose-targets", true, true, true, false)]
#[case::at_the_division("divide-amount", true, true, true, true)]
fn backing_out_at_a_question_leaves_nothing_chosen_for_the_steps_after_it(
    #[case] stop: &str,
    #[case] has_modes: bool,
    #[case] has_costs: bool,
    #[case] has_x: bool,
    #[case] has_targets: bool,
) {
    let game = table(Shape::Modal, 6);
    let mut pending = pending(&game, needs(true, 1));
    let (asked, end) = walk(&game, &mut pending, |kind| {
        (kind.name() != stop).then(|| greedy(kind)).flatten()
    });
    assert!(matches!(end, End::Cancelled), "{stop}: {end:?}");
    assert_eq!(asked.last(), Some(&stop));
    let choices = &pending.choices;
    assert_eq!(!choices.modes.is_empty(), has_modes);
    assert_eq!(!choices.optional_costs.is_empty(), has_costs);
    assert_eq!(choices.x.is_some(), has_x);
    assert_eq!(!choices.targets.is_empty(), has_targets);
    assert!(choices.division.is_empty(), "the division is asked last");
}

/// A source of choices from bytes.
struct Picks<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Picks<'_> {
    fn below(&mut self, n: usize) -> usize {
        let byte = self.bytes[self.at % self.bytes.len()];
        self.at += 1;
        usize::from(byte) % n.max(1)
    }

    /// Every answer is legal; one time in eight the caster backs out of a cast that can be backed out of.
    fn answer(&mut self, kind: &DecisionKind) -> Option<Answered> {
        let cancels = |picks: &mut Picks<'_>, can_cancel: bool| can_cancel && picks.below(8) == 0;
        match kind {
            DecisionKind::ChooseMode(o) => {
                if cancels(self, o.can_cancel) {
                    return None;
                }
                let count = o.min as usize + self.below((o.max - o.min) as usize + 1);
                let mut chosen: Vec<usize> = Vec::new();
                for _ in 0..count {
                    let rest: Vec<usize> = o
                        .modes
                        .iter()
                        .map(|mode| mode.index)
                        .filter(|index| !chosen.contains(index))
                        .collect();
                    if rest.is_empty() {
                        break;
                    }
                    chosen.push(rest[self.below(rest.len())]);
                }
                chosen.sort_unstable();
                Some(Answered::Modes(chosen))
            }
            DecisionKind::ChooseCosts(o) => {
                if cancels(self, o.can_cancel) {
                    return None;
                }
                let costs = o
                    .optional
                    .iter()
                    .filter(|_| self.below(2) == 0)
                    .map(|cost| cost.index)
                    .collect();
                Some(Answered::Costs(costs))
            }
            DecisionKind::ChooseNumber(o) => {
                if cancels(self, o.can_cancel) {
                    return None;
                }
                let top = o.max.unwrap_or(o.min + 3);
                let span = usize::try_from(top - o.min).unwrap() + 1;
                Some(Answered::Number(
                    o.min + u32::try_from(self.below(span)).unwrap(),
                ))
            }
            DecisionKind::ChooseTargets(o) => {
                if cancels(self, o.can_cancel) {
                    return None;
                }
                let count = o.min as usize + self.below((o.max - o.min) as usize + 1);
                let mut left = o.legal.clone();
                let mut chosen = Vec::new();
                for _ in 0..count.min(left.len()) {
                    chosen.push(left.remove(self.below(left.len())));
                }
                Some(Answered::Targets(chosen))
            }
            DecisionKind::DivideAmount(o) => {
                if cancels(self, o.can_cancel) {
                    return None;
                }
                let mut shares: Vec<(Target, u32)> = o.among.iter().map(|t| (*t, 1)).collect();
                for _ in 0..o.total as usize - shares.len() {
                    let at = self.below(shares.len());
                    shares[at].1 += 1;
                }
                Some(Answered::Division(shares))
            }
            other => panic!("{} was not expected", other.name()),
        }
    }
}

/// Where each question stands among the steps.
fn rank(name: &str) -> usize {
    match name {
        "choose-mode" => 0,
        "choose-costs" => 2,
        "choose-number" => 3,
        "choose-targets" => 4,
        "divide-amount" => 5,
        "pay-mana" => 8,
        other => panic!("{other} is not a question of the announcement"),
    }
}

proptest! {
    #[test]
    fn any_cast_asks_in_order_keeps_its_answers_and_pays_the_total_or_is_rewound(
        shape in prop_oneof![
            Just(Shape::Plain),
            Just(Shape::Targeted),
            Just(Shape::Divided),
            Just(Shape::Stingy),
            Just(Shape::Creature),
            Just(Shape::Modal),
        ],
        kicked in any::<bool>(),
        symbols in 0u32..3,
        lands in 0usize..7,
        bytes in proptest::collection::vec(any::<u8>(), 1..24),
    ) {
        let game = table(shape, lands);
        let mut pending = pending(&game, needs(kicked, symbols));
        let mut picks = Picks { bytes: &bytes, at: 0 };
        let (asked, end) = walk(&game, &mut pending, |kind| picks.answer(kind));
        // The questions never go back and never repeat a step.
        let ranks: Vec<usize> = asked.iter().map(|name| rank(name)).collect();
        prop_assert!(ranks.windows(2).all(|pair| pair[0] < pair[1]), "{asked:?}");
        let choices = &pending.choices;
        let kicks = u32::try_from(choices.optional_costs.len()).unwrap();
        let paid = 1 + kicks + choices.x.unwrap_or(0) * symbols;
        match end {
            End::Paying(options) => {
                prop_assert_eq!(options.cost.generic(), paid);
                prop_assert!(can_afford(&options.cost, &options.pool, &options.sources, options.life));
                prop_assert!(options.can_cancel);
                // A division gives every target at least one and the whole amount.
                if !choices.division.is_empty() {
                    prop_assert!(choices.division.iter().all(|(_, share)| *share >= 1));
                    let total: u32 = choices.division.iter().map(|(_, share)| share).sum();
                    prop_assert!(total == 3 || total == 1, "{:?}", choices.division);
                }
            }
            // The table has a creature and two players, so a spell is rewound only when its cost cannot be paid.
            End::Rewound => prop_assert!(paid as usize > lands, "{asked:?}"),
            End::Cancelled => {}
        }
    }
}
