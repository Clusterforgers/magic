//! Goal: whether a cost can be paid, the game's payment and the plain payment stay quick and agree on crowded boards of every kind of mana source (lands, duals, Treasures, Sol Ring, Signets, filter lands, Cabal Coffers, creature-only and snow mana), for costs the lands pay, costs that need the rocks and costs too large to pay.

use std::time::{Duration, Instant};

use rstest::rstest;

use mtg_cards::spend::{SpendFilter, SpendOn, SpendTraits};
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_core::mana::{ManaCost, ManaType};

use crate::help::Help;
use crate::mana::{ManaOption, ManaPool, ManaSource, can_afford, plain_payment, propose};
use crate::spend::{Spend, SpendKind};

const COLORS: [ManaType; 5] = [
    ManaType::White,
    ManaType::Blue,
    ManaType::Black,
    ManaType::Red,
    ManaType::Green,
];

fn option(id: u64, ability: usize, mana: ManaType) -> ManaOption {
    ManaOption::new(
        ManaSource {
            object: ObjectId(id),
            ability,
        },
        mana,
    )
}

fn board(sources: usize) -> Vec<ManaOption> {
    let mut all = Vec::new();
    let mut id = 100;
    let mut next = || {
        id += 1;
        id
    };
    for n in 0..sources {
        let id = next();
        let first = COLORS[n % 5];
        match n % 12 {
            // A dual land.
            0 | 4 => {
                all.push(option(id, 0, first));
                all.push(option(id, 1, COLORS[(n + 1) % 5]));
            }
            // A Treasure.
            1 => all.extend(COLORS.iter().enumerate().map(|(a, &m)| {
                let mut o = option(id, a, m);
                o.sacrifice = true;
                o
            })),
            // Sol Ring.
            2 => {
                let mut o = option(id, 0, ManaType::Colorless);
                o.count = 2;
                all.push(o);
            }
            // A Signet.
            3 => {
                let mut o = option(id, 0, first);
                o.also = vec![COLORS[(n + 1) % 5]];
                o.cost = ManaCost::parse("{1}").unwrap();
                all.push(o);
            }
            // A filter land.
            5 => {
                for (a, (m, also)) in [
                    (ManaType::White, ManaType::White),
                    (ManaType::White, ManaType::Blue),
                    (ManaType::Blue, ManaType::Blue),
                ]
                .into_iter()
                .enumerate()
                {
                    let mut o = option(id, a, m);
                    o.also = vec![also];
                    o.cost = ManaCost::parse("{W/U}").unwrap();
                    all.push(o);
                }
            }
            // Cabal Coffers.
            6 => {
                let mut o = option(id, 0, ManaType::Black);
                o.count = 4;
                o.cost = ManaCost::parse("{2}").unwrap();
                all.push(o);
            }
            // Mana for creature spells only, of any colour.
            7 => all.extend(COLORS.iter().enumerate().map(|(a, &m)| {
                let mut o = option(id, a, m);
                o.only = vec![SpendOn::Spell(SpendFilter::of_type(CardType::Creature))];
                o
            })),
            // A snow land.
            8 => {
                let mut o = option(id, 0, first);
                o.only = vec![SpendOn::Snow];
                all.push(o);
            }
            _ => all.push(option(id, 0, first)),
        }
    }
    all
}

fn creature() -> Spend {
    Spend {
        kind: SpendKind::Spell,
        traits: SpendTraits {
            types: [CardType::Creature].into_iter().collect(),
            ..SpendTraits::default()
        },
        ..Spend::default()
    }
}

/// The costs a board of `n` sources is asked about: colored and hybrid ones the lands pay, `{S}`, generic ones that
/// need the rocks, Signets, filter lands and Cabal Coffers, and one too large to pay.
fn costs(n: usize) -> Vec<String> {
    let mut all: Vec<String> = [
        "{3}{G}{G}",
        "{W}{U}{B}{R}{G}",
        "{2}{G/U}{G/U}",
        "{S}{2}{G}",
        "{6}{U}{U}",
    ]
    .map(String::from)
    .to_vec();
    all.extend([n / 2, n * 3 / 4, n, n * 4].map(|g| format!("{{{g}}}{{B}}")));
    all
}

#[rstest]
#[case::sixty(60, Duration::from_millis(1_500))]
#[case::one_twenty(120, Duration::from_millis(3_000))]
fn affording_and_proposing_stay_quick_on_a_crowded_board(#[case] n: usize, #[case] most: Duration) {
    let sources = board(n);
    let spend = creature();
    let pool = ManaPool::default();
    for text in costs(n) {
        let cost = ManaCost::parse(&text).unwrap();
        let start = Instant::now();
        let afford = can_afford(&cost, &pool, &sources, 20, &spend);
        let afforded = start.elapsed();
        let start = Instant::now();
        let proposal = propose(&cost, &pool, &sources, Help::NONE, 20, &spend);
        let proposed = start.elapsed();
        let start = Instant::now();
        let plain = plain_payment(&cost, &pool, &sources, &spend);
        let plained = start.elapsed();
        eprintln!(
            "n={n} {text}: afford {afford} {afforded:?} propose {proposed:?} plain {plained:?}"
        );
        assert_eq!(afford, proposal.payment.is_some(), "{text}");
        assert!(plain.is_none() || afford, "{text}");
        assert!(
            afforded < most && proposed < most && plained < most,
            "{text}"
        );
    }
}
