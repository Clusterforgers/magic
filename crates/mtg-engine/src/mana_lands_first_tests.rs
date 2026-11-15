//! Goal: an automatic payment taps lands before artifacts and creatures that make mana whenever the cost can still be paid, whatever the order the sources come in and the number of taps they save, while sacrifices and sources that do more than add mana still come before it, a colored symbol still gets the one source that can make it, and a cost that was payable stays payable.

use mtg_core::ids::ObjectId;
use mtg_core::mana::{ManaCost, ManaType};
use rstest::rstest;

use crate::mana::{ManaOption, ManaPool, ManaSource, auto_payment, check_payment};
use crate::spend::Spend;

const LIFE: i32 = 20;

/// A permanent of a table and what it adds.
#[derive(Debug, Clone, Copy)]
enum Kind {
    /// A land that taps for one mana of the letter.
    Land(char),
    /// A land that taps for one mana of the letter and hurts its controller for it.
    Painland(char),
    /// A creature that taps for one mana of the letter.
    Dork(char),
    /// An artifact that taps for one colorless mana (Mind Stone).
    Stone,
    /// An artifact that taps for two colorless mana (Sol Ring).
    SolRing,
    /// An artifact that pays {1} to tap for one mana of each of two colors (a Signet).
    Signet(char, char),
    /// An artifact that is sacrificed for one mana of any color.
    Treasure,
}

use Kind::{Dork, Land, Painland, Signet, SolRing, Stone, Treasure};

fn mana(letter: char) -> ManaType {
    match letter {
        'W' => ManaType::White,
        'U' => ManaType::Blue,
        'B' => ManaType::Black,
        'R' => ManaType::Red,
        'G' => ManaType::Green,
        other => panic!("not a mana letter: {other}"),
    }
}

fn source(object: u64, ability: usize) -> ManaSource {
    ManaSource {
        object: ObjectId(object),
        ability,
    }
}

/// The options of the permanent number `object`.
fn options(object: u64, kind: Kind) -> Vec<ManaOption> {
    let one = |produces| ManaOption::new(source(object, 0), produces);
    match kind {
        Land(letter) => vec![one(mana(letter))],
        Painland(letter) => vec![ManaOption {
            extra: true,
            ..one(mana(letter))
        }],
        Dork(letter) => vec![ManaOption {
            nonland: true,
            ..one(mana(letter))
        }],
        Stone => vec![ManaOption {
            nonland: true,
            ..one(ManaType::Colorless)
        }],
        SolRing => vec![ManaOption {
            nonland: true,
            count: 2,
            ..one(ManaType::Colorless)
        }],
        Signet(first, second) => vec![ManaOption {
            nonland: true,
            also: vec![mana(second)],
            cost: ManaCost::parse("{1}").unwrap(),
            ..one(mana(first))
        }],
        Treasure => "WUBRG"
            .chars()
            .enumerate()
            .map(|(ability, letter)| ManaOption {
                nonland: true,
                sacrifice: true,
                ..ManaOption::new(source(object, ability), mana(letter))
            })
            .collect(),
    }
}

/// The sources of the permanents, numbered from 1 in the order given: the lower the number, the earlier a tie is broken.
fn sources(table: &[(&str, Kind)]) -> Vec<ManaOption> {
    (1u64..)
        .zip(table)
        .flat_map(|(object, (_, kind))| options(object, *kind))
        .collect()
}

/// The names of the permanents the automatic payment of `cost` taps, sorted; `None` when it cannot be paid.
fn tapped(cost: &str, table: &[(&str, Kind)]) -> Option<Vec<String>> {
    let (cost, pool, sources) = (
        ManaCost::parse(cost).unwrap(),
        ManaPool::default(),
        sources(table),
    );
    let payment = auto_payment(&cost, &pool, &sources, LIFE, &Spend::other())?;
    assert_eq!(
        check_payment(&cost, &pool, &sources, &[], LIFE, &Spend::other(), &payment).map(|_| ()),
        Ok(()),
        "{payment:?}"
    );
    let mut names: Vec<String> = payment
        .taps
        .iter()
        .map(|tap| {
            table[usize::try_from(tap.object.0).unwrap() - 1]
                .0
                .to_owned()
        })
        .collect();
    names.sort();
    names.dedup();
    Some(names)
}

fn names(list: &[&str]) -> Vec<String> {
    let mut found: Vec<String> = list.iter().map(|name| (*name).to_owned()).collect();
    found.sort();
    found
}

#[rstest]
#[case::sol_ring_stays_for_what_lands_pay("{2}", &[("Sol Ring", SolRing), ("Forest", Land('G')), ("Mountain", Land('R'))], &["Forest", "Mountain"])]
#[case::sol_ring_pays_the_rest("{3}", &[("Sol Ring", SolRing), ("Forest", Land('G'))], &["Forest", "Sol Ring"])]
#[case::sol_ring_pays_all_that_is_left("{4}", &[("Sol Ring", SolRing), ("Forest", Land('G')), ("Mountain", Land('R'))], &["Forest", "Mountain", "Sol Ring"])]
#[case::a_stone_stays("{3}", &[("Mind Stone", Stone), ("Forest", Land('G')), ("Mountain", Land('R')), ("Island", Land('U'))], &["Forest", "Island", "Mountain"])]
#[case::a_dork_stays_for_the_generic("{1}{G}", &[("Elves", Dork('G')), ("Forest", Land('G')), ("Mountain", Land('R'))], &["Forest", "Mountain"])]
#[case::a_dork_stays_for_the_color("{G}", &[("Elves", Dork('G')), ("Forest", Land('G'))], &["Forest"])]
#[case::a_dork_pays_the_color_a_land_cannot("{G}{G}", &[("Elves", Dork('G')), ("Forest", Land('G')), ("Mountain", Land('R'))], &["Elves", "Forest"])]
#[case::two_dorks_stay("{G}{1}", &[("Elves", Dork('G')), ("Birds", Dork('G')), ("Forest", Land('G')), ("Swamp", Land('B'))], &["Forest", "Swamp"])]
#[case::a_dork_pays_a_second_color("{R}{G}", &[("Elves", Dork('G')), ("Mountain", Land('R')), ("Forest", Land('G'))], &["Forest", "Mountain"])]
#[case::a_dork_pays_where_the_land_is_the_other_color("{R}{G}", &[("Elves", Dork('G')), ("Mountain", Land('R'))], &["Elves", "Mountain"])]
#[case::a_signet_stays("{2}{U}", &[("Signet", Signet('W', 'U')), ("Island", Land('U')), ("Plains", Land('W')), ("Mountain", Land('R'))], &["Island", "Mountain", "Plains"])]
#[case::a_signet_makes_what_lands_cannot("{U}{W}{1}{1}", &[("Signet", Signet('W', 'U')), ("Island", Land('U')), ("Plains", Land('W')), ("Mountain", Land('R'))], &["Island", "Mountain", "Plains", "Signet"])]
#[case::a_treasure_stays("{1}", &[("Treasure", Treasure), ("Forest", Land('G'))], &["Forest"])]
#[case::a_treasure_pays_the_last("{2}", &[("Treasure", Treasure), ("Forest", Land('G'))], &["Forest", "Treasure"])]
#[case::a_treasure_comes_after_a_stone("{2}", &[("Treasure", Treasure), ("Mind Stone", Stone), ("Forest", Land('G'))], &["Forest", "Mind Stone"])]
#[case::sol_ring_before_a_treasure("{3}", &[("Treasure", Treasure), ("Sol Ring", SolRing), ("Forest", Land('G'))], &["Forest", "Sol Ring"])]
#[case::a_land_that_hurts_comes_after_a_dork("{G}", &[("Elves", Dork('G')), ("Painland", Painland('G'))], &["Elves"])]
#[case::a_land_that_hurts_pays_when_nothing_else_can("{G}{G}{G}", &[("Elves", Dork('G')), ("Painland", Painland('G')), ("Forest", Land('G'))], &["Elves", "Forest", "Painland"])]
#[case::lands_are_free_of_the_rest("{2}", &[("Elves", Dork('G')), ("Mind Stone", Stone), ("Sol Ring", SolRing), ("Forest", Land('G')), ("Swamp", Land('B'))], &["Forest", "Swamp"])]
fn lands_are_tapped_before_artifacts_and_creatures(
    #[case] cost: &str,
    #[case] table: &[(&str, Kind)],
    #[case] expected: &[&str],
    #[values(false, true)] lands_first: bool,
) {
    let mut table = table.to_vec();
    if lands_first {
        // The earlier a source is numbered, the earlier it is taken among equals: the lands come first as well.
        table.sort_by_key(|(_, kind)| !matches!(kind, Land(_) | Painland(_)));
    }
    assert_eq!(tapped(cost, &table), Some(names(expected)));
}

#[rstest]
#[case::generic_and_colored("{1}{G}", &[("Elves", Dork('G')), ("Mountain", Land('R'))])]
#[case::two_colors_from_a_dork_and_a_land("{R}{G}", &[("Elves", Dork('G')), ("Mountain", Land('R'))])]
#[case::a_dork_and_a_signet("{3}", &[("Elves", Dork('G')), ("Signet", Signet('W', 'U')), ("Plains", Land('W'))])]
#[case::sol_ring_alone("{2}", &[("Sol Ring", SolRing)])]
#[case::a_treasure_alone("{G}", &[("Treasure", Treasure)])]
#[case::too_much("{4}", &[("Sol Ring", SolRing), ("Forest", Land('G'))])]
#[case::a_color_nobody_makes("{U}", &[("Elves", Dork('G')), ("Forest", Land('G'))])]
fn a_cost_that_could_be_paid_before_can_be_paid_now(
    #[case] cost: &str,
    #[case] table: &[(&str, Kind)],
) {
    let cleared: Vec<(&str, Kind)> = table.to_vec();
    let before = {
        let cost = ManaCost::parse(cost).unwrap();
        let mut all = sources(&cleared);
        for option in &mut all {
            option.nonland = false;
        }
        auto_payment(&cost, &ManaPool::default(), &all, LIFE, &Spend::other()).is_some()
    };
    assert_eq!(tapped(cost, table).is_some(), before);
}
