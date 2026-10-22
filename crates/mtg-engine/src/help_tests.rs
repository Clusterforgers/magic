//! Goal: `can_afford_with`, `auto_payment_with`, `check_payment_with` and `complete_payment_with` agree with a brute-force check of every combination of floating mana, taps, convoking creatures (some of them also mana sources) and delved cards, for plain, hybrid and Phyrexian costs.

use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_core::mana::{ManaCost, ManaType};
use proptest::prelude::*;
use rstest::rstest;

use crate::error::PaymentError;
use crate::help::{ConvokeOption, Convoked, Help};
use crate::mana::{
    ManaOption, ManaPool, ManaSource, Payment, auto_payment_with, can_afford_with,
    check_payment_with, complete_payment_with,
};
use crate::spend::Spend;

const LIFE: i32 = 20;

const TYPES: [ManaType; 3] = [ManaType::White, ManaType::Red, ManaType::Colorless];
const COLORS: [Color; 3] = [Color::White, Color::Blue, Color::Red];

fn cost(text: &str) -> ManaCost {
    ManaCost::parse(text).unwrap()
}

fn tap(object: u64) -> ManaSource {
    ManaSource {
        object: ObjectId(object),
        ability: 0,
    }
}

/// One thing a payment may name.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Plain(usize),
    Tap(usize),
}

/// A creature that may convoke: its colours (a mask over `COLORS`) and the mana source it is, if any.
type Creature = (u8, Option<usize>);

/// Floating mana (by type), sources (by type), creatures, cards in the graveyard and a cost.
type Case = (Vec<usize>, Vec<usize>, Vec<Creature>, usize, &'static str);

fn case() -> impl Strategy<Value = Case> {
    (
        prop::collection::vec(0usize..3, 0..3),
        prop::collection::vec(0usize..3, 0..3),
        prop::collection::vec((0u8..8, prop::option::of(0usize..3)), 0..3),
        0usize..3,
        prop::sample::select(vec![
            "{1}",
            "{2}",
            "{3}",
            "{W}",
            "{R}{R}",
            "{W}{1}",
            "{2}{R}",
            "{W}{R}{1}",
            "{2}{C}",
            "{W/U}",
            "{W/U}{R}",
            "{R/P}{1}",
            "{2/W}{R}",
            "{W/R}{W/R}{1}",
            "{C}",
            "{U}{1}",
        ]),
    )
}

struct World {
    pool: ManaPool,
    sources: Vec<ManaOption>,
    creatures: Vec<ConvokeOption>,
    graveyard: Vec<ObjectId>,
    cost: ManaCost,
    parts: Vec<(Part, ManaType)>,
}

fn world((plain, taps, creatures, cards, cost_text): &Case) -> World {
    let mut held = ManaPool::default();
    let mut parts = Vec::new();
    for (at, mana) in plain.iter().enumerate() {
        held.add(TYPES[*mana]);
        parts.push((Part::Plain(at), TYPES[*mana]));
    }
    let mut sources = Vec::new();
    for (at, mana) in taps.iter().enumerate() {
        sources.push(ManaOption::new(tap(at as u64), TYPES[*mana]));
        parts.push((Part::Tap(at), TYPES[*mana]));
    }
    let mut offered: Vec<ConvokeOption> = Vec::new();
    for (at, (mask, dork)) in creatures.iter().enumerate() {
        // A creature that is a mana source is that permanent; a second creature cannot be the same one.
        let id = match dork {
            Some(source) if *source < taps.len() => *source as u64,
            _ => 100 + at as u64,
        };
        if offered.iter().any(|o| o.creature == ObjectId(id)) {
            continue;
        }
        offered.push(ConvokeOption {
            creature: ObjectId(id),
            colors: (0..3)
                .filter(|bit| mask >> bit & 1 == 1)
                .map(|bit| COLORS[bit])
                .collect(),
        });
    }
    World {
        pool: held,
        sources,
        creatures: offered,
        graveyard: (0..*cards as u64).map(|n| ObjectId(200 + n)).collect(),
        cost: cost(cost_text),
        parts,
    }
}

impl World {
    fn help(&self) -> Help<'_> {
        Help {
            convoke: &self.creatures,
            improvise: &[],
            delve: &self.graveyard,
        }
    }
}

/// What a creature may be tapped for: nothing, {1}, or one mana of each colour it is.
/// One way to tap a creature: for {1} or for one mana of a colour.
type Tap = Option<Color>;

fn uses(option: &ConvokeOption) -> Vec<Option<Tap>> {
    let mut uses = vec![None, Some(None)];
    uses.extend(option.colors.iter().map(|c| Some(Some(*c))));
    uses
}

/// Every way the creatures may be tapped: each unused, for {1} or for one mana of a colour it is.
fn tappings(world: &World) -> Vec<Vec<Convoked>> {
    let mut all: Vec<Vec<Convoked>> = vec![Vec::new()];
    for option in &world.creatures {
        let mut next = Vec::new();
        for chosen in &all {
            for way in uses(option) {
                let mut more = chosen.clone();
                if let Some(pays) = way {
                    more.push(Convoked {
                        creature: option.creature,
                        pays,
                    });
                }
                next.push(more);
            }
        }
        all = next;
    }
    all
}

fn payment_of(
    world: &World,
    case: &Case,
    chosen: &[&Part],
    convoke: &[Convoked],
    delved: usize,
    life: u32,
) -> Payment {
    let mut payment = Payment {
        life,
        convoke: convoke.to_vec(),
        delve: world.graveyard[..delved].to_vec(),
        ..Payment::default()
    };
    for part in chosen {
        match part {
            Part::Plain(at) => payment.pool.push(TYPES[case.0[*at]]),
            Part::Tap(at) => payment.taps.push(world.sources[*at].source),
        }
    }
    payment
}

/// Whether the mana made and the helpers pay the cost exactly, with `life` paid for Phyrexian symbols: some way to
/// pay each symbol takes that life, and everything together is the rest of the cost's value and covers each colour it
/// needs. A creature paying {1} and a delved card cover no colour.
fn pays(world: &World, made: &[ManaType], convoke: &[Convoked], delved: usize, life: u32) -> bool {
    world.cost.resolutions().iter().any(|r| {
        r.life == life
            && made.len() + convoke.len() + delved == r.cost.mana_value() as usize
            && ManaType::ALL.into_iter().all(|mana| {
                let helped = convoke
                    .iter()
                    .filter(|c| c.pays.map(ManaType::from) == Some(mana))
                    .count();
                made.iter().filter(|m| **m == mana).count() + helped
                    >= r.cost.required(mana) as usize
            })
    })
}

/// Whether a creature is tapped both for mana and for convoke.
fn twice(world: &World, chosen: &[&Part], convoke: &[Convoked]) -> bool {
    chosen.iter().any(|part| match part {
        Part::Tap(at) => convoke
            .iter()
            .any(|c| c.creature == world.sources[*at].source.object),
        Part::Plain(_) => false,
    })
}

fn subsets(count: usize) -> Vec<Vec<usize>> {
    (0..1usize << count)
        .map(|mask| (0..count).filter(|bit| mask >> bit & 1 == 1).collect())
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(250))]

    #[test]
    fn the_four_functions_agree_with_a_check_of_every_payment(case in case()) {
        let world = world(&case);
        let spend = Spend::other();
        let lives: Vec<u32> = (0..=world.cost.life_symbols()).map(|n| n * 2).collect();
        // Every valid payment: its mana, creatures, cards and life.
        let mut valid: Vec<(Vec<usize>, Vec<Convoked>, usize, u32)> = Vec::new();
        for subset in subsets(world.parts.len()) {
            let made: Vec<ManaType> = subset.iter().map(|at| world.parts[*at].1).collect();
            let chosen: Vec<&Part> = subset.iter().map(|at| &world.parts[*at].0).collect();
            for convoke in tappings(&world) {
                for delved in 0..=world.graveyard.len() {
                    for life in &lives {
                        let payment = payment_of(&world, &case, &chosen, &convoke, delved, *life);
                        let engine = check_payment_with(
                            &world.cost, &world.pool, &world.sources, world.help(), &[], LIFE, &spend, &payment,
                        );
                        let expected = !twice(&world, &chosen, &convoke)
                            && pays(&world, &made, &convoke, delved, *life);
                        prop_assert_eq!(
                            engine.is_ok(), expected,
                            "{:?} convoking {:?} delving {} for life {}: {:?}", chosen, convoke, delved, life, engine
                        );
                        if expected {
                            valid.push((subset.clone(), convoke.clone(), delved, *life));
                        }
                    }
                }
            }
        }
        let affordable = can_afford_with(&world.cost, &world.pool, &world.sources, world.help(), LIFE, &spend);
        prop_assert_eq!(affordable, !valid.is_empty());
        let auto = auto_payment_with(&world.cost, &world.pool, &world.sources, world.help(), LIFE, &spend);
        prop_assert_eq!(auto.is_some(), !valid.is_empty());
        if let Some(payment) = &auto {
            prop_assert!(check_payment_with(
                &world.cost, &world.pool, &world.sources, world.help(), &[], LIFE, &spend, payment
            ).is_ok());
            let least = valid.iter().map(|v| v.3).min().unwrap_or(0);
            prop_assert_eq!(payment.life, least, "mana and helpers before life");
            let fewest = valid
                .iter()
                .filter(|v| v.3 == least)
                .map(|v| v.1.len() + v.2)
                .min()
                .unwrap_or(0);
            prop_assert_eq!(payment.convoke.len() + payment.delve.len(), fewest, "no helper that is not needed");
        }
        // Any part of a valid payment is completed to a valid one that holds it.
        for (subset, convoke, delved, _) in valid.iter().take(8) {
            for keep in 0..=subset.len() {
                let kept: Vec<&Part> = subset[..keep].iter().map(|at| &world.parts[*at].0).collect();
                for helped in [0, convoke.len()] {
                    let cards = if helped == 0 { 0 } else { *delved };
                    let partial = payment_of(&world, &case, &kept, &convoke[..helped], cards, 0);
                    let done = complete_payment_with(
                        &world.cost, &world.pool, &world.sources, world.help(), &[], LIFE, &spend, &partial,
                    );
                    prop_assert!(done.is_ok(), "{:?} with {} helpers of {:?}: {:?}", kept, helped, subset, done);
                    if let Ok(whole) = done {
                        prop_assert!(check_payment_with(
                            &world.cost, &world.pool, &world.sources, world.help(), &[], LIFE, &spend, &whole
                        ).is_ok());
                        for mana in &partial.pool {
                            prop_assert!(whole.pool.contains(mana));
                        }
                        for source in &partial.taps {
                            prop_assert!(whole.taps.contains(source));
                        }
                        for creature in &partial.convoke {
                            prop_assert!(whole.convoke.contains(creature));
                        }
                        for card in &partial.delve {
                            prop_assert!(whole.delve.contains(card));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn without_helpers_the_functions_are_the_plain_ones(case in case()) {
        let world = world(&case);
        let spend = Spend::other();
        let with = auto_payment_with(&world.cost, &world.pool, &world.sources, Help::NONE, LIFE, &spend);
        let plain = crate::mana::auto_payment(&world.cost, &world.pool, &world.sources, LIFE, &spend);
        prop_assert_eq!(with, plain);
    }
}

fn red_creature(id: u64) -> ConvokeOption {
    ConvokeOption {
        creature: ObjectId(id),
        colors: vec![Color::Red],
    }
}

#[rstest]
#[case::a_creature_for_red("{1}{R}", Some(Color::Red), true)]
#[case::a_creature_for_one("{1}{R}", None, true)]
#[case::a_colour_it_is_not("{1}{R}", Some(Color::Blue), false)]
fn a_creature_pays_only_for_a_colour_it_is(
    #[case] cost_text: &str,
    #[case] pays: Option<Color>,
    #[case] allowed: bool,
) {
    let creatures = [red_creature(5)];
    let help = Help {
        convoke: &creatures,
        improvise: &[],
        delve: &[],
    };
    let pool = ManaPool::default();
    let payment = Payment {
        convoke: vec![Convoked {
            creature: ObjectId(5),
            pays,
        }],
        pool: vec![ManaType::Red],
        ..Payment::default()
    };
    let checked = check_payment_with(
        &cost(cost_text),
        &{
            let mut held = pool;
            held.add(ManaType::Red);
            held
        },
        &[],
        help,
        &[],
        LIFE,
        &Spend::other(),
        &payment,
    );
    // A creature for {1} and floating red pay {1}{R}; for red, floating red pays the {1}.
    assert_eq!(checked.is_ok(), allowed, "{checked:?}");
    if !allowed {
        assert!(matches!(checked, Err(PaymentError::WrongColor { .. })));
    }
}
