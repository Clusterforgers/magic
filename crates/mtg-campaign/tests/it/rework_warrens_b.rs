//! Goal: an innate level may take its own values, abilities and words; the Warrens' reworked bosses Korrath to the
//! Unmaker load whole (precon deck file, five innate levels, two
//! phases, four satchel spells, life by party size, colours of the deck); each innate does its thing at level I and at
//! level V; each phase begins at its round with its begin effect once; each satchel spell casts from the satchel, a
//! reusable one coming back with its tax; and a real fight at level V plays on with the invariants checked.

use std::path::Path;
use std::sync::Arc;

use mtg_campaign::content::{CardKind, SatchelCard};
use mtg_campaign::innate::{LEVELS, innate_id};
use mtg_campaign::scale::PHASE_ROUNDS;
use mtg_campaign::{Run, RunEvent, content};
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::color::{Color, ColorSet};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{
    CostsChoice, DivideChoice, ModeChoice, NumberChoice, TargetChoice, YesNoChoice,
};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::format::Commander;
use mtg_engine::keywords::has_keyword;
use mtg_engine::phases::{BossPhase, PhaseStart};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Decision, Engine, GameSetup};
use mtg_testkit::brain::{Level, SeatBrain, brain};
use mtg_testkit::cards::{burn, creature, forest, island, mountain, plains, swamp, wastes};
use mtg_testkit::corpus;
use mtg_testkit::fixture_decks::fixture_deck;
use mtg_testkit::harness::{FailureKind, PlayOptions, play_brains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;
use serde::Deserialize;

/// The two heroes sit at 0 and 1, the boss last.
const BOSS: usize = 2;

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

fn boss(id: &str) -> &'static mtg_campaign::content::Boss {
    let mut themes = content().unwrap().themes.iter();
    themes.find_map(|theme| theme.boss(id)).unwrap()
}

fn custom(id: &str) -> Arc<CardDefinition> {
    content().unwrap().card(id).unwrap().definition()
}

/// A deck file of `content/decks`.
#[derive(Debug, Deserialize)]
struct DeckFile {
    cards: Vec<(String, u32)>,
}

fn deck_file(name: &str) -> DeckFile {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("content/decks")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn colours(letters: &str) -> ColorSet {
    letters
        .chars()
        .filter_map(Color::from_symbol)
        .fold(ColorSet::EMPTY, ColorSet::with)
}

/// A custom card keeps every ability it is written with when it becomes an emblem (an unsupported one would be dropped).
fn whole(id: &str) {
    let card = content().unwrap().card(id).unwrap();
    assert_eq!(
        card.definition().abilities.len() + card.definition().rules.len(),
        card.abilities.len(),
        "{id} lost an ability as an emblem"
    );
}

#[rstest]
fn the_reworked_record_loads_whole(
    #[values(
        "korrath_wilds",
        "sahrra_pride",
        "valdric_manor",
        "ozrith_observatory",
        "aurenne_bastion",
        "vorthana_hive",
        "sivanne_mire",
        "unmaker_void"
    )]
    id: &str,
) {
    let content = content().unwrap();
    let b = boss(id);
    let deck = deck_file(b.deck_file.as_str());
    assert_eq!(deck.cards.iter().map(|(_, n)| n).sum::<u32>(), 100);
    // The deck's corpus cards together span the boss's colours (its face commander may play from a draft).
    let identity = deck
        .cards
        .iter()
        .filter_map(|(name, _)| corpus::card(name).ok())
        .fold(ColorSet::EMPTY, |all, card| {
            card.color_identity().iter().fold(all, ColorSet::with)
        });
    assert_eq!(colours(&b.colors), identity, "{id} colours");
    let innate = b.innate.as_str();
    for level in 1..=LEVELS {
        let at = innate_id(innate, level);
        let card = content.card(&at).unwrap();
        assert_eq!((card.kind, card.level), (CardKind::Innate, level));
        assert!(!card.text.is_empty() && !card.text.contains("{N}"), "{at}");
        whole(&at);
    }
    assert_eq!(b.round_phases.len(), PHASE_ROUNDS.len());
    for phase in &b.round_phases {
        assert_eq!(content.card(phase).unwrap().kind, CardKind::Phase);
        whole(phase);
    }
    assert_eq!(b.satchel.len(), 4);
    for SatchelCard { card, .. } in &b.satchel {
        assert!(corpus::card(card).is_ok(), "{card}");
    }
}

/// An innate whose levels differ in a named value, an ability left out below level IV and their own words.
fn innate_with(params: &str, texts: &str) -> String {
    format!(
        r#"[{{"id":"tithe","name":"Tithe","kind":"Innate","type_line":"Boss — Innate","levels":[1,1,1,1,1],
        "params":{params},"texts":{texts},"text":"",
        "abilities":[{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep","whose":"You"}}}},"effects":[{{"GainLife":{{"amount":"$GAIN"}}}}]}}}},"$MORE"]}}]"#
    )
}

const MORE: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"Draw":{"count":1}}]}}"#;

#[test]
fn an_innate_level_takes_its_own_values_abilities_and_words() {
    let params = format!(r#"{{"GAIN":[1,2,3,5,8],"MORE":[null,null,null,{MORE},{MORE}]}}"#);
    let texts = r#"["one","two","three {N}","four","five"]"#;
    let content =
        mtg_campaign::content::Content::parse(&[&innate_with(&params, texts)], &[], "[]").unwrap();
    for (level, (gain, abilities, text)) in [
        (1, 1, "one"),
        (2, 2, "two"),
        (3, 3, "three 1"),
        (4, 5, "four"),
        (5, 8, "five"),
    ]
    .into_iter()
    .zip([1, 1, 1, 2, 2])
    .map(|((l, g, t), a)| (l, (g, a, t)))
    {
        let card = content.card(&innate_id("tithe", level)).unwrap();
        assert_eq!(
            (card.abilities.len(), card.text.as_str()),
            (abilities, text),
            "level {level}"
        );
        let json = serde_json::to_string(&card.abilities).unwrap();
        assert!(json.contains(&format!("\"amount\":{gain}")), "{json}");
    }
}

#[rstest]
#[case::four_values(
    r#"{"GAIN":[1,2,3,4],"MORE":[null,null,null,null,null]}"#,
    r#"["a","b","c","d","e"]"#
)]
#[case::four_texts(
    r#"{"GAIN":[1,2,3,4,5],"MORE":[null,null,null,null,null]}"#,
    r#"["a","b","c","d"]"#
)]
fn an_innate_needs_five_of_each_per_level_value(#[case] params: &str, #[case] texts: &str) {
    assert!(
        mtg_campaign::content::Content::parse(&[&innate_with(params, texts)], &[], "[]").is_err()
    );
}

/// One innate using every substitution at once: a named value, `$N`, `$L`, a cost's `{N}` and its own words.
const MIXED: &str = r#"[{"id":"levy","name":"Levy","kind":"Innate","type_line":"Boss — Innate","levels":[1,1,2,3,4],
  "params":{"GAIN":[1,2,3,5,8]},"texts":["a {N}","b {N}","c {N}","d {N}","e {N}"],"text":"",
  "abilities":[{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},
    "if":{"Compare":{"left":"$L","op":"Ge","right":"$N"}},
    "effects":[{"Pay":{"who":"Spotlit","cost":"{N}","unpaid":[{"GainLife":{"amount":"$GAIN"}}]}}]}}]}]"#;

#[rstest]
#[case::one(1, "a 1", "{1}", 1)]
#[case::three(3, "c 2", "{2}", 3)]
#[case::five(5, "e 4", "{4}", 8)]
fn params_level_and_mana_cost_combine_in_one_innate(
    #[case] level: u8,
    #[case] text: &str,
    #[case] cost: &str,
    #[case] gain: u32,
) {
    let content = mtg_campaign::content::Content::parse(&[MIXED], &[], "[]").unwrap();
    let card = content.card(&innate_id("levy", level)).unwrap();
    assert_eq!(card.text, text);
    let rules = serde_json::to_string(&card.abilities).unwrap();
    for part in [
        format!("\"cost\":\"{cost}\""),
        format!("\"amount\":{gain}"),
        format!("\"left\":{level}"),
    ] {
        assert!(rules.contains(&part), "{part} in {rules}");
    }
}

/// The heroes (seats 0 and 1) and the boss with libraries of lands, at the boss's first main phase.
fn table() -> Scenario {
    let mut scenario = Scenario::new(3).life(BOSS, 40).at(3, BOSS, Step::Main1);
    for seat in 0..3 {
        scenario = scenario.library(seat, (0..60).map(|_| mountain()));
    }
    scenario
}

fn bear() -> Arc<CardDefinition> {
    creature("Bear", 1, &[Color::Green], 2, 2)
}

/// A creature of `power`/`toughness` with these subtypes, costing nothing.
fn kin(name: &str, subtypes: &[&str], power: i32, toughness: i32) -> Arc<CardDefinition> {
    let card = subtypes.iter().fold(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(power, toughness),
        |card, s| card.with_subtype(*s),
    );
    Arc::new(card)
}

fn named(driver: &Driver, seat: usize, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.controller() == pid(seat) && o.card().name == name)
        .count()
}

fn creatures(driver: &Driver, seat: usize) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            state
                .object(**id)
                .is_some_and(|o| o.controller() == pid(seat))
                && characteristics(state, **id)
                    .is_some_and(|c| c.types.contains(CardType::Creature))
        })
        .count()
}

fn pt(driver: &Driver, name: &str) -> (i32, i32) {
    let c = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (c.power.unwrap_or(0), c.toughness.unwrap_or(0))
}

/// Answers passively until the stack is empty and the active player holds priority again.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let state = driver.state();
        let mine = driver.decision()?.player == state.turn().active();
        if state.stack().is_empty()
            && mine
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("settle".into()))
}

/// The boss casts each named burn spell at its target in turn, letting each resolve.
fn burn_all(driver: &mut Driver, shots: &[(&str, &str)]) -> Result<(), ScriptError> {
    for (spell, at) in shots {
        let target = driver.target_named(at)?;
        driver.cast_at(spell, &[target])?;
        settle(driver)?;
    }
    Ok(())
}

fn red_mana(n: usize) -> Vec<mtg_engine::setup::PresetPermanent> {
    (0..n).map(|_| ready(mountain())).collect()
}

#[rstest]
#[case::once_each_turn_at_i(1, (6, 6))]
#[case::each_time_at_ii(2, (7, 7))]
#[case::four_each_time_at_v(5, (13, 13))]
fn thick_hide_grows_a_damaged_survivor(
    #[case] level: u8,
    #[case] brute: (i32, i32),
) -> Result<(), ScriptError> {
    let mut battlefield = red_mana(2);
    battlefield.push(ready(kin("Brute", &["Dinosaur"], 5, 5)));
    let scenario = table()
        .battlefield(BOSS, battlefield)
        .hand(BOSS, [burn("Jolt", 1), burn("Zap", 1)])
        .emblems(BOSS, [custom(&innate_id("korrath_thick_hide", level))]);
    let mut driver = Driver::from_scenario(scenario)?;
    burn_all(&mut driver, &[("Jolt", "Brute"), ("Zap", "Brute")])?;
    assert_eq!(pt(&driver, "Brute"), brute);
    Ok(())
}

#[rstest]
#[case::the_strongest_cat_at_i(1, (4, 4), (2, 2), 0)]
#[case::every_cat_and_a_cub_at_v(5, (6, 6), (5, 5), 1)]
fn the_pride_grows_when_a_cat_dies(
    #[case] level: u8,
    #[case] tom: (i32, i32),
    #[case] lion: (i32, i32),
    #[case] cubs: usize,
) -> Result<(), ScriptError> {
    let mut battlefield = red_mana(1);
    battlefield.extend([
        ready(kin("Tom", &["Cat"], 3, 3)),
        ready(kin("Lion", &["Cat"], 2, 2)),
        ready(kin("Kitten", &["Cat"], 1, 1)),
    ]);
    let scenario = table()
        .battlefield(BOSS, battlefield)
        .hand(BOSS, [burn("Jolt", 1)])
        .emblems(BOSS, [custom(&innate_id("sahrra_the_pride", level))]);
    let mut driver = Driver::from_scenario(scenario)?;
    burn_all(&mut driver, &[("Jolt", "Kitten")])?;
    assert_eq!(pt(&driver, "Tom"), tom);
    assert_eq!(pt(&driver, "Lion"), lion);
    assert_eq!(named(&driver, BOSS, "Cat"), cubs);
    Ok(())
}

#[rstest]
#[case::the_strongest_once_at_i(1, (4, 4), (1, 1), 40)]
#[case::each_vampire_each_time_with_life_at_v(5, (5, 5), (3, 3), 44)]
fn the_crimson_feast_feeds_on_hero_deaths(
    #[case] level: u8,
    #[case] count: (i32, i32),
    #[case] thrall: (i32, i32),
    #[case] life: i32,
) -> Result<(), ScriptError> {
    let mut battlefield = red_mana(2);
    battlefield.extend([
        ready(kin("Count", &["Vampire"], 3, 3)),
        ready(kin("Thrall", &["Vampire"], 1, 1)),
    ]);
    let scenario = table()
        .battlefield(0, [ready(creature("Squire", 0, &[Color::White], 1, 1))])
        .battlefield(1, [ready(creature("Page", 0, &[Color::White], 1, 1))])
        .battlefield(BOSS, battlefield)
        .hand(BOSS, [burn("Jolt", 1), burn("Zap", 1)])
        .emblems(BOSS, [custom(&innate_id("valdric_crimson_feast", level))]);
    let mut driver = Driver::from_scenario(scenario)?;
    burn_all(&mut driver, &[("Jolt", "Squire"), ("Zap", "Page")])?;
    assert_eq!(pt(&driver, "Count"), count);
    assert_eq!(pt(&driver, "Thrall"), thrall);
    assert_eq!(driver.state().player(pid(BOSS)).unwrap().life(), life);
    Ok(())
}

#[rstest]
#[case::a_token_only_at_i(1, 1)]
#[case::a_token_and_four_damage_at_v(5, 0)]
fn stormcalling_makes_an_elemental_and_strikes(
    #[case] level: u8,
    #[case] ogres: usize,
) -> Result<(), ScriptError> {
    let scenario = table()
        .battlefield(0, [ready(creature("Ogre", 0, &[Color::Red], 4, 4))])
        .battlefield(BOSS, red_mana(1))
        .hand(BOSS, [burn("Jolt", 1)])
        .emblems(BOSS, [custom(&innate_id("ozrith_stormcalling", level))]);
    let mut driver = Driver::from_scenario(scenario)?;
    let hero = driver.target_player(1);
    driver.cast_at("Jolt", &[hero])?;
    settle(&mut driver)?;
    assert_eq!(named(&driver, BOSS, "Elemental"), 1);
    assert_eq!(named(&driver, 0, "Ogre"), ogres);
    Ok(())
}

#[rstest]
#[case::plus_one_at_i(1, (3, 2))]
#[case::plus_three_at_v(5, (5, 5))]
fn dawn_charge_bolsters_only_attackers(
    #[case] level: u8,
    #[case] attacking: (i32, i32),
) -> Result<(), ScriptError> {
    let scenario = table()
        .battlefield(
            BOSS,
            [
                ready(kin("Sentry", &["Soldier"], 2, 2)),
                ready(kin("Lancer", &["Soldier"], 2, 2)),
            ],
        )
        .emblems(BOSS, [custom(&innate_id("aurenne_dawn_charge", level))]);
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(pt(&driver, "Lancer"), (2, 2));
    driver.idle_until(3, Step::DeclareAttackers)?;
    driver.attack(&[("Lancer", 0)])?;
    assert_eq!(pt(&driver, "Lancer"), attacking);
    assert_eq!(pt(&driver, "Sentry"), (2, 2));
    Ok(())
}

#[rstest]
#[case::a_small_one_at_i(1, "Rat", true)]
#[case::not_a_big_one_at_i(1, "Wurm", false)]
#[case::any_at_v(5, "Wurm", true)]
fn rot_hive_raises_a_creature_card_at_the_end_step(
    #[case] level: u8,
    #[case] dead: &str,
    #[case] risen: bool,
) -> Result<(), ScriptError> {
    let card = if dead == "Rat" {
        creature("Rat", 1, &[Color::Black], 1, 1)
    } else {
        creature("Wurm", 5, &[Color::Green], 6, 6)
    };
    let scenario = table()
        .graveyard(BOSS, [card])
        .emblems(BOSS, [custom(&innate_id("vorthana_rot_hive", level))]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(4, Step::Upkeep)?;
    assert_eq!(named(&driver, BOSS, dead), usize::from(risen));
    Ok(())
}

#[rstest]
#[case::nontoken_only_at_i(1, (3, 3), (3, 3))]
#[case::four_on_each_at_v(5, (6, 6), (7, 7))]
fn coiled_growth_adds_counters_as_creatures_enter(
    #[case] level: u8,
    #[case] bear_pt: (i32, i32),
    #[case] dinosaur: (i32, i32),
) -> Result<(), ScriptError> {
    // The Sky Falls makes a 3/3 Dinosaur token at the boss's upkeep (and gives +2/+0 to its creatures).
    let scenario = Scenario::new(3)
        .at(2, 1, Step::Main2)
        .library(0, (0..20).map(|_| mountain()))
        .library(1, (0..20).map(|_| mountain()))
        .library(BOSS, (0..20).map(|_| mountain()))
        .battlefield(BOSS, (0..2).map(|_| ready(forest())))
        .hand(BOSS, [bear()])
        .emblems(
            BOSS,
            [
                custom(&innate_id("sivanne_coiled_growth", level)),
                custom("korrath_sky_falls"),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(3, Step::Main1)?;
    driver.cast("Bear")?;
    settle(&mut driver)?;
    let plus = |(p, t): (i32, i32)| (p + 2, t);
    assert_eq!(pt(&driver, "Bear"), plus(bear_pt));
    assert_eq!(pt(&driver, "Dinosaur"), plus(dinosaur));
    Ok(())
}

#[rstest]
#[case::only_the_huge_at_i(1, false, true)]
#[case::from_mana_value_five_at_v(5, true, true)]
fn unmaking_gives_big_colourless_creatures_annihilator(
    #[case] level: u8,
    #[case] six: bool,
    #[case] ten: bool,
) -> Result<(), ScriptError> {
    let scenario = table()
        .battlefield(
            BOSS,
            [
                ready(creature("Spawn", 6, &[], 6, 6)),
                ready(creature("Titan", 10, &[], 10, 10)),
                ready(creature("Drone", 0, &[Color::Black], 6, 6)),
            ],
        )
        .emblems(BOSS, [custom(&innate_id("unmaker_unmaking", level))]);
    let driver = Driver::from_scenario(scenario)?;
    let has = |name: &str| {
        has_keyword(
            driver.state(),
            driver.id_of(name).unwrap(),
            &Keyword::Annihilator(Amount::Fixed(1)),
        )
    };
    assert_eq!(
        (has("Spawn"), has("Titan"), has("Drone")),
        (six, ten, false)
    );
    Ok(())
}

/// What a Phase 1's begin effect left: hero creatures, boss creatures and tapped hero creatures.
type Left = (usize, usize, usize);

/// Seat 0 holds three Bears, seat 1 one; the boss a colourless 5/5 Brute and a black 1/1 Imp.
fn phase_table(id: &str) -> Scenario {
    let content = content().unwrap();
    let b = boss(id);
    let phases = b
        .round_phases
        .iter()
        .zip(PHASE_ROUNDS)
        .map(|(card, round)| BossPhase {
            begins: PhaseStart::AtRound(round),
            emblem: content.card(card).unwrap().definition(),
        });
    let mut scenario = Scenario::new(3)
        .life(BOSS, 40)
        .battlefield(0, (0..3).map(|_| ready(bear())))
        .battlefield(1, [ready(bear())])
        .battlefield(
            BOSS,
            [
                ready(creature("Brute", 5, &[], 5, 5)),
                ready(creature("Imp", 0, &[Color::Black], 1, 1)),
            ],
        )
        .phases(BOSS, phases);
    for seat in 0..3 {
        scenario = scenario.library(seat, (0..80).map(|_| mountain()));
    }
    scenario
}

fn tapped_heroes(driver: &Driver) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| {
            o.controller() != pid(BOSS)
                && o.permanent()
                    .is_some_and(mtg_engine::object::PermanentState::tapped)
        })
        .count()
}

#[rstest]
#[case::korrath("korrath_wilds", (0, 1, 0))]
#[case::sahrra("sahrra_pride", (4, 6, 0))]
#[case::valdric("valdric_manor", (1, 2, 0))]
#[case::ozrith("ozrith_observatory", (0, 2, 0))]
#[case::aurenne("aurenne_bastion", (4, 6, 0))]
#[case::vorthana("vorthana_hive", (1, 1, 0))]
#[case::sivanne("sivanne_mire", (4, 2, 4))]
#[case::unmaker("unmaker_void", (0, 1, 0))]
fn each_phase_begins_at_its_round_and_its_begin_effect_fires_once(
    #[case] id: &str,
    #[case] left: Left,
) -> Result<(), ScriptError> {
    let rounds = PHASE_ROUNDS;
    let mut driver = Driver::from_scenario(phase_table(id))?;
    // Round r ends with the boss's turn 3r; the phase begins in its end step.
    driver.idle_until(3 * rounds[0], Step::Main2)?;
    assert_eq!(driver.state().player(pid(BOSS)).unwrap().phases(), (2, 0));
    driver.idle_until(3 * rounds[0] + 1, Step::Upkeep)?;
    assert_eq!(driver.state().player(pid(BOSS)).unwrap().phases(), (2, 1));
    let now = (
        creatures(&driver, 0) + creatures(&driver, 1),
        creatures(&driver, BOSS),
        tapped_heroes(&driver),
    );
    assert_eq!(now, left, "{id}");
    driver.idle_until(3 * rounds[1], Step::Main2)?;
    assert_eq!(driver.state().player(pid(BOSS)).unwrap().phases(), (2, 1));
    driver.idle_until(3 * rounds[1] + 1, Step::Upkeep)?;
    assert_eq!(driver.state().player(pid(BOSS)).unwrap().phases(), (2, 2));
    Ok(())
}

/// An answer that goes ahead where a passive one backs out: the first legal targets, the first modes, the least number,
/// automatic payment, yes; anything else as the passive answer.
fn eager(decision: &Decision) -> Answer {
    let at_least_one = |min: u32| usize::try_from(min.max(1)).unwrap();
    match &decision.kind {
        DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
        DecisionKind::ChooseTargets(o) => Answer::ChooseTargets(TargetChoice::Targets(
            o.legal.iter().copied().take(at_least_one(o.min)).collect(),
        )),
        DecisionKind::ChooseMode(o) => Answer::ChooseMode(ModeChoice::Modes(
            o.modes
                .iter()
                .take(at_least_one(o.min))
                .map(|m| m.index)
                .collect(),
        )),
        DecisionKind::ChooseNumber(o) => Answer::ChooseNumber(NumberChoice::Number(
            o.max.map_or(o.min.max(1), |most| o.min.max(1).min(most)),
        )),
        DecisionKind::DivideAmount(o) => Answer::DivideAmount(DivideChoice::Division(
            o.among.iter().take(1).map(|t| (*t, o.total)).collect(),
        )),
        DecisionKind::ChooseCosts(_) => Answer::ChooseCosts(CostsChoice::Costs(Vec::new())),
        DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(true)),
        _ => Driver::passive_answer(decision),
    }
}

/// The satchel table at the boss's first main phase: the boss with five lands of each colour and four Wastes, a 5/5
/// Dinosaur Vampire Cat and a Vampire Thrall, two creature cards in its graveyard and a colourless creature in hand;
/// seat 0 with a Bear, an artifact and an enchantment, and a burn spell; seat 1 with a Bear.
fn satchel_table(card: &SatchelCard) -> Scenario {
    let spell = vec![corpus::card(&card.card).unwrap()];
    let scenario = if card.reusable {
        Scenario::new(3).satchel_reusable(BOSS, spell)
    } else {
        Scenario::new(3).satchel(BOSS, spell)
    };
    let mut lands: Vec<_> = Vec::new();
    for land in [plains, island, swamp, mountain, forest] {
        lands.extend((0..5).map(|_| ready(land())));
    }
    lands.extend((0..4).map(|_| ready(wastes())));
    lands.push(ready(kin("Raptor", &["Dinosaur", "Vampire", "Cat"], 5, 5)));
    lands.push(ready(kin("Thrall", &["Vampire"], 1, 1)));
    let thing = |name: &str, ty: CardType| {
        Arc::new(
            CardDefinition::new(name)
                .with_type(ty)
                .with_mana_cost(mtg_testkit::cards::cost(3, &[])),
        )
    };
    let mut scenario = scenario
        .life(BOSS, 40)
        .battlefield(BOSS, lands)
        .graveyard(
            BOSS,
            [
                creature("Rat", 1, &[Color::Black], 1, 1),
                creature("Ghoul", 2, &[Color::Black], 2, 2),
            ],
        )
        .hand(BOSS, [creature("Husk", 4, &[], 4, 4)])
        .battlefield(
            0,
            [
                ready(bear()),
                ready(thing("Idol", CardType::Artifact)),
                ready(thing("Ward", CardType::Enchantment)),
                ready(mountain()),
            ],
        )
        .hand(0, [burn("Jolt", 1)])
        .battlefield(1, [ready(bear())]);
    for seat in 0..3 {
        scenario = scenario.library(seat, (0..40).map(|_| mountain()));
    }
    scenario
}

/// Casts `card` from the boss's satchel (in answer to seat 0's burn spell at its Raptor for a counterspell) and lets
/// everything resolve; returns the satchel object's id.
fn cast_from_satchel(
    driver: &mut Driver,
    card: &str,
    respond: bool,
) -> Result<ObjectId, ScriptError> {
    let state = driver.state();
    let id = state
        .satchel(pid(BOSS))
        .into_iter()
        .find(|id| state.object(*id).is_some_and(|o| o.card().name == card))
        .unwrap();
    if respond {
        let raptor = driver.target_named("Raptor")?;
        driver.cast_at("Jolt", &[raptor])?;
    }
    for _ in 0..50 {
        let decision = driver.decision()?;
        if decision.player == pid(BOSS) && matches!(decision.kind, DecisionKind::Priority(_)) {
            break;
        }
        driver.answer_passively()?;
    }
    driver.answer(Answer::Priority(PriorityAction::CastSpell(id)))?;
    for _ in 0..400 {
        let decision = driver.decision()?.clone();
        if driver.state().stack().is_empty() && matches!(decision.kind, DecisionKind::Priority(_)) {
            return Ok(id);
        }
        let answer = if decision.player == pid(BOSS) {
            eager(&decision)
        } else {
            Driver::passive_answer(&decision)
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck(format!("{card} never resolved")))
}

#[rstest]
fn each_satchel_spell_casts_and_a_reusable_one_returns_with_its_tax(
    #[values(
        "korrath_wilds",
        "sahrra_pride",
        "valdric_manor",
        "ozrith_observatory",
        "aurenne_bastion",
        "vorthana_hive",
        "sivanne_mire",
        "unmaker_void"
    )]
    id: &str,
) -> Result<(), ScriptError> {
    for card in &boss(id).satchel {
        let respond = card.card == "Not of This World";
        let start = if respond { (4, 0) } else { (3, BOSS) };
        let scenario = satchel_table(card).at(start.0, start.1, Step::Main1);
        let mut driver = Driver::from_scenario(scenario)?;
        let spell = cast_from_satchel(&mut driver, &card.card, respond)?;
        let state = driver.state();
        let back = state.satchel(pid(BOSS)).contains(&spell);
        assert_eq!(back, card.reusable, "{}", card.card);
        if card.reusable {
            assert_eq!(
                (state.satchel_casts(spell), state.satchel_tax(spell)),
                (1, 2),
                "{}",
                card.card
            );
        }
    }
    Ok(())
}

/// Two heroes on fixture precons against the boss at innate level V with its precon, phases and satchel.
fn fight_setup(id: &str, seed: u64) -> GameSetup {
    let content = content().unwrap();
    let mut events = vec![RunEvent::Content {
        version: mtg_campaign::run::CONTENT,
    }];
    for at in 0..2u8 {
        events.push(RunEvent::Joined {
            name: format!("H{at}"),
            user: Some(format!("u{at}")),
            bot: None,
        });
        events.push(RunEvent::DeckChosen {
            hero: at,
            deck: mtg_campaign::HeroDeck {
                id: "x".into(),
                name: "x".into(),
                colors: "G".into(),
                commanders: Vec::new(),
                makeup: None,
            },
        });
    }
    events.push(RunEvent::Started);
    let run = Run::replay(content, "warrens", seed, 0, &events).unwrap();
    let mut plan = run.fight_plan(content).unwrap().unwrap();
    plan.boss = run.boss_seat(content, boss(id), 1, (2, 12)).unwrap();
    let mut decks = Vec::new();
    let mut commanders = Vec::new();
    for name in ["precon_tramplesaurus_rex", "precon_calling_all_angels"] {
        let deck = fixture_deck(name).unwrap();
        decks.push(deck.cards.clone());
        commanders.push(deck.commanders.clone());
    }
    decks.push(crate::common::boss_deck(&plan));
    commanders.push(Vec::new());
    let setup = GameSetup::new(Arc::new(Commander), seed, decks).with_commanders(commanders);
    plan.apply(content, setup).unwrap()
}

#[rstest]
fn a_real_fight_at_level_v_plays_on(
    #[values(
        "korrath_wilds",
        "sahrra_pride",
        "valdric_manor",
        "ozrith_observatory",
        "aurenne_bastion",
        "vorthana_hive",
        "sivanne_mire",
        "unmaker_void",
        "siren_queen",
        "drowned_archivist",
        "leviathan"
    )]
    id: &str,
) {
    let seed = 7;
    let setup = fight_setup(id, seed);
    let innate = innate_id(boss(id).innate.as_str(), LEVELS);
    let name = &content().unwrap().card(&innate).unwrap().name;
    let engine = Engine::new(setup.clone()).unwrap();
    let state = engine.state();
    assert!(
        state
            .zone(Zone::Command(pid(BOSS)))
            .iter()
            .filter_map(|o| state.object(*o))
            .any(|o| &o.card().name == name),
        "{id} holds {name}"
    );
    let levels = [Level::Normal; 3];
    let mut brains: Vec<Box<dyn SeatBrain>> = levels
        .iter()
        .enumerate()
        .map(|(seat, level)| brain(*level, seed, pid(seat), false))
        .collect();
    let options = PlayOptions {
        max_answers: 600,
        ..PlayOptions::default()
    };
    match play_brains(setup, seed, options, &mut brains) {
        Ok(_) => {}
        Err(failure) if matches!(failure.kind, FailureKind::StepLimit(_)) => {}
        Err(failure) => panic!("{id}: {failure}"),
    }
}
