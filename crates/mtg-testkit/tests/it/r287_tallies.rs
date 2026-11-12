//! Goal: the turn facts r28-7 adds hold at two to four seats and replay to the live state: a card put into exile from any
//! graveyard (`CardsExiled`, Ennis: "if one or more cards were put into exile this turn"), a permanent put into your own
//! hand from the battlefield and not an opponent's (`PermanentsToHand`, Barrin), a card put into your graveyard from
//! anywhere but the battlefield and not a creature that died (`CardsGravedNotFromBattlefield`, Dimir Strandcatcher), a
//! land played from exile and none (`PlayedFromExile`, Visions of Phyrexia: "if you didn't play a card from exile this
//! turn"), and a creature with a macro keyword named by its own name (Arcbound Overseer: "each creature you control with modular").
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn card(name: &str, kind: CardType, rules: &[&str]) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name).with_type(kind);
    if kind == CardType::Creature {
        card = card.with_power_toughness(2, 2);
    }
    if kind == CardType::Sorcery {
        card = card.with_mana_cost(mtg_core::mana::ManaCost::parse("{0}").unwrap());
    }
    for rule in rules {
        let ability: Ability = serde_json::from_str(rule).unwrap();
        card = card.with_rules(ability);
    }
    Arc::new(card)
}

/// A watcher that gains 1 life at its controller's end step when the tally reaches `at`.
fn watcher(who: &str, what: &str, at: u32) -> Arc<CardDefinition> {
    watcher_by(who, what, "Ge", at)
}

/// A watcher that gains 1 life at its controller's end step when the tally compares to `at` by `op`.
fn watcher_by(who: &str, what: &str, op: &str, at: u32) -> Arc<CardDefinition> {
    let rule = format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"End","whose":"You"}}}},"if":{{"Compare":{{"left":{{"Tallied":{{"who":"{who}","what":"{what}"}}}},"op":"{op}","right":{at}}}}},"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#
    );
    card("Watcher", CardType::Creature, &[&rule])
}

/// Casts the sorcery, then plays the turn out; the life seat 0 ends the turn with, and the replay checked.
fn life_after(scenario: Scenario) -> i64 {
    life_after_playing(scenario, None)
}

/// Casts the sorcery, plays the named land if one is given, then plays the turn out; the life seat 0 ends the turn with,
/// and the replay checked.
fn life_after_playing(scenario: Scenario, land: Option<&str>) -> i64 {
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.cast("Act").unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    if let Some(land) = land {
        driver.play_land(land).unwrap();
    }
    for _ in 0..400 {
        if driver.state().turn().number() != 3 {
            break;
        }
        driver.answer_passively().unwrap();
    }
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    i64::from(driver.state().player(PlayerId(0)).unwrap().life())
}

const EXILE_GRAVEYARDS: &str =
    r#"{"Spell":{"effects":[{"Exile":{"what":{"Each":[{"In":"Graveyard"}]}}}]}}"#;

#[rstest]
fn r287_a_card_exiled_from_any_graveyard_counts(
    #[values(2, 3, 4)] seats: usize,
    #[values(None, Some(0), Some(1))] graved: Option<usize>,
) {
    let mut scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [ready(watcher("EachPlayer", "CardsExiled", 1))])
        .hand(0, [card("Act", CardType::Sorcery, &[EXILE_GRAVEYARDS])]);
    if let Some(seat) = graved {
        scenario = scenario.graveyard(seat, [card("Bear", CardType::Creature, &[])]);
    }
    let gained = life_after(scenario) - 20;
    // The sorcery itself goes to its owner's graveyard, not to exile.
    assert_eq!(gained, i64::from(graved.is_some()));
}

const BOUNCE_MINE: &str = r#"{"Spell":{"effects":[{"Move":{"what":{"Each":[{"Named":"Bear"},{"ControlledBy":"You"}]},"to":"Hand"}}]}}"#;
const BOUNCE_THEIRS: &str = r#"{"Spell":{"effects":[{"Move":{"what":{"Each":[{"Named":"Bear"},{"ControlledBy":"Opponents"}]},"to":"Hand"}}]}}"#;

#[rstest]
fn r287_only_a_permanent_put_into_your_own_hand_counts(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] mine: bool,
) {
    let act = if mine { BOUNCE_MINE } else { BOUNCE_THEIRS };
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(watcher("You", "PermanentsToHand", 1)),
                ready(card("Bear", CardType::Creature, &[])),
            ],
        )
        .battlefield(1, [ready(card("Bear", CardType::Creature, &[]))])
        .hand(0, [card("Act", CardType::Sorcery, &[act])]);
    assert_eq!(life_after(scenario) - 20, i64::from(mine));
}

const MILL_TWO: &str = r#"{"Spell":{"effects":[{"Mill":{"count":2}}]}}"#;
const WRATH_MINE: &str = r#"{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"Named":"Bear"},{"ControlledBy":"You"}]}}}]}}"#;

#[rstest]
#[case::milled_two_and_the_spell(MILL_TWO, 3, 1)]
#[case::milled_two_and_the_spell_short_of_four(MILL_TWO, 4, 0)]
#[case::a_creature_that_died_is_not_counted(WRATH_MINE, 2, 0)]
fn r287_cards_graved_from_anywhere_but_the_battlefield(
    #[values(2, 3, 4)] seats: usize,
    #[case] act: &str,
    #[case] at: u32,
    #[case] gained: i64,
) {
    let library: Vec<_> = (0..5)
        .map(|_| card("Filler", CardType::Creature, &[]))
        .collect();
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(watcher("You", "CardsGravedNotFromBattlefield", at)),
                ready(card("Bear", CardType::Creature, &[])),
            ],
        )
        .library(0, library)
        .hand(0, [card("Act", CardType::Sorcery, &[act])]);
    assert_eq!(life_after(scenario) - 20, gained);
}

const OVERSEER: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Main1","whose":"You"}},"effects":[{"PutCounters":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasKeywordNamed":"Modular"}]},"counter":"+1/+1"}}]}}"#;
const MODULAR: &str = r#"{"Keyword":{"Macro":{"name":"modular","amount":1}}}"#;
const BUSHIDO: &str = r#"{"Keyword":{"Macro":{"name":"bushido","amount":1}}}"#;

#[rstest]
#[case::modular(MODULAR, 1)]
#[case::another_macro_keyword(BUSHIDO, 0)]
fn r287_a_macro_keyword_is_named_by_its_own_name(
    #[values(2, 3, 4)] seats: usize,
    #[case] keyword: &str,
    #[case] counters: u32,
) {
    let scenario = Scenario::new(seats).at(3, 0, Step::Upkeep).battlefield(
        0,
        [
            ready(card("Overseer", CardType::Creature, &[OVERSEER])),
            ready(card("Arcbound", CardType::Creature, &[keyword])),
        ],
    );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.pass_until(Step::Main1).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    let id = driver.id_of("Arcbound").unwrap();
    let plus = driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&mtg_cards::value::CounterKind::PlusOnePlusOne)
        });
    // Modular 1 enters with its own counter only when cast; set up on the battlefield it has none of its own.
    assert_eq!(plus, counters);
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

const IMPULSE: &str = r#"{"Spell":{"effects":[{"ExileTop":{"as":"impulse"}},{"Rule":{"rule":{"MayCast":{"from":"Exile","only":{"Bound":"impulse"},"play":true}},"duration":"UntilEndOfTurn"}}]}}"#;

#[rstest]
fn r287_a_card_played_from_exile_counts(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] played: bool,
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [ready(watcher_by("You", "PlayedFromExile", "Lt", 1))])
        .library(0, [card("Waste", CardType::Land, &[])])
        .hand(0, [card("Act", CardType::Sorcery, &[IMPULSE])]);
    let life = life_after_playing(scenario, played.then_some("Waste"));
    assert_eq!(life - 20, i64::from(!played));
}
