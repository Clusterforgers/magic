//! Goal: the deck check and the game judge a pair of commanders the same way over the printed text of real cards of every pair kind (Partner, Partner with, Partner—Friends forever, Choose a Background, Doctor's companion), whether the engine runs the cards' rules or leaves them to the table: an allowed pair passes the check and starts a game at 2 to 4 seats with both commanders in the command zone, and a refused pair (kinds that do not mix, two partner groups, a Partner with beside a plain partner) is refused by both.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::assemble;
use mtg_cards::compile::compile;
use mtg_cards::deck::{CardFacts, DeckLine, DeckZone, IssueCode, Legality, deck_report};
use mtg_cards::load::Library;
use mtg_cards::support::{Caps, PrintedCard, PrintedFace};
use mtg_core::ids::PlayerId;
use mtg_engine::format::Commander;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup};
use mtg_testkit::cards::wastes;
use rstest::rstest;

/// A real card: name, mana cost, type line, Scryfall keywords, oracle text, power and toughness.
type Real = (&'static str, &'static str, &'static str, &'static [&'static str], &'static str, Option<&'static str>);

const TYMNA: Real = (
    "Tymna the Weaver",
    "{1}{W}{B}",
    "Legendary Creature \u{2014} Human Cleric",
    &["Lifelink", "Partner"],
    "Lifelink\nAt the beginning of each of your postcombat main phases, you may pay X life, where X is the number of opponents that were dealt combat damage this turn. If you do, draw X cards.\nPartner (You can have two commanders if both have partner.)",
    Some("2"),
);
const THRASIOS: Real = (
    "Thrasios, Triton Hero",
    "{G}{U}",
    "Legendary Creature \u{2014} Merfolk Wizard",
    &["Partner", "Scry"],
    "{4}: Scry 1, then reveal the top card of your library. If it's a land card, put it onto the battlefield tapped. Otherwise, draw a card.\nPartner (You can have two commanders if both have partner.)",
    Some("1"),
);
const PIR: Real = (
    "Pir, Imaginative Rascal",
    "{2}{G}",
    "Legendary Creature \u{2014} Human",
    &["Partner with", "Partner"],
    "Partner with Toothy, Imaginary Friend (When this creature enters, target player may put Toothy into their hand from their library, then shuffle.)\nIf one or more counters would be put on a permanent your team controls, that many plus one of each of those kinds of counters are put on that permanent instead.",
    Some("1"),
);
const TOOTHY: Real = (
    "Toothy, Imaginary Friend",
    "{3}{U}",
    "Legendary Creature \u{2014} Illusion",
    &["Partner with", "Partner"],
    "Partner with Pir, Imaginative Rascal (When this creature enters, target player may put Pir into their hand from their library, then shuffle.)\nWhenever you draw a card, put a +1/+1 counter on Toothy.\nWhen Toothy leaves the battlefield, draw a card for each +1/+1 counter on it.",
    Some("1"),
);
const BJORNA: Real = (
    "Bjorna, Nightfall Alchemist",
    "{U}{R}",
    "Legendary Creature \u{2014} Human",
    &["Goad", "Partner"],
    "{T}, Sacrifice an artifact: Bjorna, Nightfall Alchemist deals 1 damage to target creature. Goad that creature. (Until your next turn, that creature attacks each combat if able and attacks a player other than you if able.)\nPartner\u{2014}Friends forever (You can have two commanders if both have this ability.)",
    Some("1"),
);
const ELMAR: Real = (
    "Elmar, Ulvenwald Informant",
    "{1}{R}{G}",
    "Legendary Creature \u{2014} Human",
    &["Investigate", "Haste", "Partner"],
    "Haste\nWhenever you cast your second spell each turn, untap target creature, then investigate. (Create a colorless Clue artifact token with \"{2}, Sacrifice this artifact: Draw a card.\")\nPartner\u{2014}Friends forever (You can have two commanders if both have this ability.)",
    Some("3"),
);
const ELLIE: Real = (
    "Ellie, Brick Master",
    "{1}{R}",
    "Legendary Creature \u{2014} Human Survivor",
    &["Distract the Horde", "Partner"],
    "Distract the Horde \u{2014} Whenever a player attacks one of your opponents, that attacking player creates a tapped 1/1 black Fungus Zombie creature token named Cordyceps Infected that's attacking that opponent.\nPartner\u{2014}Survivors (You can have two commanders if both have this ability.)",
    Some("2"),
);
const WILSON: Real = (
    "Wilson, Refined Grizzly",
    "{1}{G}",
    "Legendary Creature \u{2014} Bear Warrior",
    &["Reach", "Vigilance", "Choose a background", "Trample", "Ward"],
    "This spell can't be countered.\nReach, vigilance, trample\nWard {2} (Whenever this creature becomes the target of a spell or ability an opponent controls, counter it unless that player pays {2}.)\nChoose a Background (You can have a Background as a second commander.)",
    Some("2"),
);
const CANDLEKEEP_SAGE: Real = (
    "Candlekeep Sage",
    "{2}{U}",
    "Legendary Enchantment \u{2014} Background",
    &[],
    "Commander creatures you own have \"When this creature enters or leaves the battlefield, draw a card.\"",
    None,
);
const CLARA: Real = (
    "Clara Oswald",
    "{6}",
    "Legendary Creature \u{2014} Human Advisor",
    &["Impossible Girl", "Doctor's companion"],
    "Impossible Girl \u{2014} If Clara Oswald is your commander, choose a color before the game begins. Clara Oswald is the chosen color.\nIf a triggered ability of a Doctor you control triggers, that ability triggers an additional time.\nDoctor's companion (You can have two commanders if the other is the Doctor.)",
    Some("2"),
);
const FOURTEENTH_DOCTOR: Real = (
    "The Fourteenth Doctor",
    "{R/G}{W}{U}",
    "Legendary Creature \u{2014} Time Lord Doctor",
    &[],
    "When you cast this spell, reveal the top fourteen cards of your library. Put all Doctor cards revealed this way into your graveyard and the rest on the bottom of your library in a random order.\nYou may have The Fourteenth Doctor enter as a copy of a Doctor card in your graveyard that was put there from your library this turn. If you do, it gains haste until end of turn.",
    Some("3"),
);
const LYSE_HEXT: Real = (
    "Lyse Hext",
    "{1}{W}{U}",
    "Legendary Creature \u{2014} Human Rebel Monk",
    &["Prowess"],
    "Prowess (Whenever you cast a noncreature spell, this creature gets +1/+1 until end of turn.)\nNoncreature spells you cast cost {1} less to cast.\nAs long as you've cast two or more noncreature spells this turn, Lyse Hext has double strike.",
    Some("2"),
);
const WASTES: Real = ("Wastes", "", "Basic Land", &[], "{T}: Add {C}.", None);

fn printed(card: Real, n: u32) -> PrintedCard {
    let (name, mana_cost, type_line, keywords, text, power) = card;
    PrintedCard {
        layout: "normal".into(),
        oracle_id: format!("00000000-0000-4000-8000-{n:012}"),
        name: name.into(),
        keywords: keywords.iter().map(|k| (*k).to_owned()).collect(),
        faces: vec![PrintedFace {
            name: name.into(),
            mana_cost: mana_cost.into(),
            type_line: type_line.into(),
            oracle_text: text.into(),
            power: power.map(Into::into),
            toughness: power.map(Into::into),
            ..PrintedFace::default()
        }],
    }
}

/// The definition the game plays: with the rules the compiler reads when `compiled`, else the printed side alone (a
/// card the table plays by hand).
fn definition(card: &PrintedCard, compiled: bool) -> Arc<CardDefinition> {
    let rules = compiled.then(|| compile(card).card).flatten();
    Arc::new(assemble(card, rules.as_ref(), &Caps::everything()).definition)
}

/// The facts the deck check reads, with the identity the game derives.
fn facts(card: &PrintedCard) -> CardFacts {
    CardFacts {
        printed: card.clone(),
        color_identity: definition(card, false).color_identity(),
        legalities: BTreeMap::from([("commander".to_owned(), Legality::Legal)]),
    }
}

/// Whether the deck check accepts the pair as the commanders of a deck filled with Wastes.
fn check_accepts(pair: [&PrintedCard; 2]) -> bool {
    let wastes = printed(WASTES, 99);
    let mut lines: Vec<DeckLine> = pair
        .iter()
        .map(|card| DeckLine {
            zone: DeckZone::Commander,
            oracle_id: card.oracle_id.clone(),
            count: 1,
        })
        .collect();
    lines.push(DeckLine {
        zone: DeckZone::Main,
        oracle_id: wastes.oracle_id.clone(),
        count: 98,
    });
    let cards: BTreeMap<String, CardFacts> = pair
        .into_iter()
        .chain([&wastes])
        .map(|card| (card.oracle_id.clone(), facts(card)))
        .collect();
    let report = deck_report(
        "commander",
        &lines,
        &cards,
        &Library::from_sources(&[]).unwrap(),
        &Caps::none(),
    );
    let codes: Vec<IssueCode> = report.legality.iter().map(|issue| issue.code).collect();
    assert!(
        codes
            .iter()
            .all(|code| matches!(code, IssueCode::CommanderPair | IssueCode::CommanderEligibility)),
        "{codes:?}"
    );
    codes.is_empty()
}

/// A Commander game where every seat leads with the pair and plays 98 Wastes.
fn table(pair: &[Arc<CardDefinition>; 2], seats: usize) -> GameSetup {
    GameSetup::new(
        Arc::new(Commander),
        7,
        (0..seats).map(|_| vec![wastes(); 98]).collect(),
    )
    .with_commanders((0..seats).map(|_| pair.to_vec()).collect())
}

#[rstest]
#[case::partners(TYMNA, THRASIOS, true)]
#[case::partner_with_each_other(PIR, TOOTHY, true)]
#[case::friends_forever(BJORNA, ELMAR, true)]
#[case::a_creature_and_its_background(WILSON, CANDLEKEEP_SAGE, true)]
#[case::the_background_first(CANDLEKEEP_SAGE, WILSON, true)]
#[case::a_doctors_companion_and_the_doctor(CLARA, FOURTEENTH_DOCTOR, true)]
#[case::the_doctor_first(FOURTEENTH_DOCTOR, CLARA, true)]
#[case::partner_with_beside_a_plain_partner(PIR, TYMNA, false)]
#[case::friends_forever_beside_a_plain_partner(BJORNA, THRASIOS, false)]
#[case::two_partner_groups(BJORNA, ELLIE, false)]
#[case::a_background_beside_a_partner(TYMNA, CANDLEKEEP_SAGE, false)]
#[case::a_companion_beside_a_partner(CLARA, TYMNA, false)]
#[case::two_legends_without_a_pair_kind(LYSE_HEXT, WILSON, false)]
fn the_deck_check_and_the_game_judge_a_pair_alike(
    #[case] first: Real,
    #[case] second: Real,
    #[case] allowed: bool,
    #[values(false, true)] compiled: bool,
    #[values(2, 4)] seats: usize,
) {
    let (a, b) = (printed(first, 1), printed(second, 2));
    assert_eq!(check_accepts([&a, &b]), allowed, "the deck check");
    let pair = [definition(&a, compiled), definition(&b, compiled)];
    let setup = table(&pair, seats);
    assert_eq!(setup.validate().is_ok(), allowed, "the game");
    if allowed {
        let engine = Engine::new(setup).unwrap();
        for seat in 0..seats {
            let names: Vec<&str> = engine
                .state()
                .zone(Zone::Command(PlayerId(seat as u64)))
                .iter()
                .map(|id| engine.state().object(*id).unwrap().whole_card().name.as_str())
                .collect();
            assert_eq!(names.len(), 2, "{names:?}");
            assert!(names.contains(&first.0) && names.contains(&second.0), "{names:?}");
        }
    }
}
