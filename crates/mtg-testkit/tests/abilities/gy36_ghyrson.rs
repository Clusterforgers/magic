//! Goal: Ghyrson Starn, Kelermorph, loaded from its draft as the server loads it, deals 2 more damage to each permanent or player another source its controller controls deals exactly 1 damage to (once per recipient of a divided spell), never for its own, an opponent's or a bigger source's damage, and its ward {2} counters an opponent's spell unless paid, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, LazyLock};

use mtg_cards::ability::Ability;
use mtg_cards::card::Card;
use mtg_cards::load::Library;
use mtg_cards::support::{PrintedCard, PrintedFace, Support, read_hash};
use mtg_cards::value::CounterKind;
use mtg_cards::{CardDefinition, DraftRules, ReadState, builtin};
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{DivideChoice, Target, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

const GHYRSON: &str = "Ghyrson Starn, Kelermorph";
const DRAFT: &str = include_str!("../../../mtg-cli/drafts/ghyrson_starn_kelermorph.json");
const PING: &str = r#"{"Activated":{"cost":["Tap"],"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}}]}}"#;

/// The card as printed (Scryfall's oracle record).
fn printed() -> PrintedCard {
    PrintedCard {
        oracle_id: "b110b8e6-859b-4370-aceb-b21c6cf61532".to_owned(),
        name: GHYRSON.to_owned(),
        keywords: vec!["Three Autostubs".to_owned(), "Ward".to_owned()],
        layout: "normal".to_owned(),
        faces: vec![PrintedFace {
            name: GHYRSON.to_owned(),
            mana_cost: "{1}{U}{R}".to_owned(),
            type_line: "Legendary Creature \u{2014} Tyranid Human".to_owned(),
            oracle_text: "Ward {2} (Whenever this creature becomes the target of a spell or ability an opponent controls, counter it unless that player pays {2}.)\nThree Autostubs \u{2014} Whenever another source you control deals exactly 1 damage to a permanent or player, Ghyrson Starn deals 2 damage to that permanent or player.".to_owned(),
            power: Some("3".to_owned()),
            toughness: Some("2".to_owned()),
            colors: vec!["R".to_owned(), "U".to_owned()],
            ..Default::default()
        }],
    }
}

/// Ghyrson as the server plays it: the built-in library with the stored draft (a trusted Forge translation, read and approved).
static CARD: LazyLock<Arc<CardDefinition>> = LazyLock::new(|| {
    let shown = printed();
    let rules: Card = serde_json::from_str(DRAFT).unwrap();
    let row = DraftRules {
        oracle_id: rules.oracle_id.clone(),
        read_hash: read_hash(&shown, &rules),
        rules: serde_json::from_str(DRAFT).unwrap(),
        read: ReadState::Approved,
        forge: true,
        proof: None,
        differs: false,
    };
    let base: &Library = builtin::library().unwrap();
    let (library, left_out) = base.with_drafts(&[row]);
    assert!(left_out.is_empty(), "{left_out:?}");
    let assembled = library.assemble(&shown, &mtg_engine::caps());
    assert!(
        matches!(assembled.support.support, Support::Read | Support::Forge),
        "the draft runs: {:?}",
        assembled.support
    );
    Arc::new(assembled.definition)
});

fn ghyrson() -> Arc<CardDefinition> {
    Arc::clone(&CARD)
}

fn with_rule(def: CardDefinition, rule: &str) -> Arc<CardDefinition> {
    Arc::new(def.with_rules(serde_json::from_str::<Ability>(rule).unwrap()))
}

/// "{T}: ~ deals 1 damage to any target."
fn pinger(name: &str) -> Arc<CardDefinition> {
    with_rule((*free_creature(name, 1, 1)).clone(), PING)
}

/// An instant for {0}: `amount` damage to any target.
fn burn(name: &str, amount: i32) -> Arc<CardDefinition> {
    let rule = format!(
        r#"{{"Spell":{{"targets":[{{"what":"AnyTarget"}}],"effects":[{{"DealDamage":{{"to":{{"Target":0}},"amount":{amount}}}}}]}}}}"#
    );
    instant(name, &rule)
}

/// An instant for {0}: 3 damage divided among one to three targets.
fn split(name: &str) -> Arc<CardDefinition> {
    instant(
        name,
        r#"{"Spell":{"targets":[{"what":"AnyTarget","bounds":{"min":1,"max":3},"divide":3}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"Divided"}}]}}"#,
    )
}

fn instant(name: &str, rule: &str) -> Arc<CardDefinition> {
    let def = CardDefinition::new(name)
        .with_type(CardType::Instant)
        .with_mana_cost(ManaCost::parse("{0}").unwrap());
    with_rule(def, rule)
}

fn walker() -> PresetPermanent {
    let card = CardDefinition::new("Walker")
        .with_type(CardType::Planeswalker)
        .with_loyalty(5);
    let mut preset = PresetPermanent::new(Arc::new(card));
    preset.counters = vec![(CounterKind::Loyalty, 5)];
    preset
}

/// Seat `active` has priority in its main phase; seat 0 has Ghyrson, a pinger, a wall and the burn spells, seat 1 the
/// same pinger and wall, a planeswalker and two Mountains.
fn table(seats: usize, active: usize, me: Arc<CardDefinition>) -> Scenario {
    Scenario::new(seats)
        .at(1, active, Step::Main1)
        .battlefield(
            0,
            [
                ready(me),
                ready(pinger("Pinger")),
                ready(free_creature("Wall", 0, 5)),
            ],
        )
        .hand(0, [burn("Zap", 1), burn("Blast", 3), split("Split")])
        .battlefield(
            1,
            [
                ready(pinger("Foe Pinger")),
                ready(free_creature("Foe Wall", 0, 5)),
                walker(),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .hand(1, [burn("Foe Zap", 1)])
}

/// What the targets of the cases are.
#[derive(Clone, Copy, Debug)]
enum Aim {
    Creature,
    Player,
    Walker,
}

fn aim(driver: &Driver, aim: Aim) -> Target {
    match aim {
        Aim::Creature => driver.target_named("Foe Wall").unwrap(),
        Aim::Player => driver.target_player(1),
        Aim::Walker => driver.target_named("Walker").unwrap(),
    }
}

/// Damage on the foe's wall, the foe's life and the walker's loyalty.
fn marks(driver: &Driver) -> (u32, i32, u32) {
    let state = driver.state();
    let on = |name: &str| {
        state
            .zone(Zone::Battlefield)
            .iter()
            .copied()
            .find(|id| name_of(state, *id) == name)
            .and_then(|id| state.object(id))
            .and_then(|object| object.permanent().cloned())
    };
    let wall = on("Foe Wall").map_or(0, |p| p.damage());
    let loyalty = on("Walker").map_or(0, |p| p.counters_of(&CounterKind::Loyalty));
    (wall, state.player(pid(1)).unwrap().life(), loyalty)
}

/// Answers passively (paying ward when `pay_ward`) until the stack and the pending triggers are empty.
fn settle(driver: &mut Driver, pay_ward: bool) -> Result<bool, ScriptError> {
    let mut asked = false;
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(asked);
        }
        let answer = match &decision.kind {
            DecisionKind::YesNo(_) => {
                asked = true;
                Answer::YesNo(YesNoChoice(pay_ward))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("an empty stack".to_owned()))
}

fn rebuilds(driver: &Driver) {
    let again = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(again.state_hash(), driver.state().state_hash());
}

/// What 1 damage plus Ghyrson's 2 leave on the aimed recipient, from the start (0 damage, `life`, 5 loyalty).
fn hit(aim: Aim, start: (u32, i32, u32), by: u32) -> (u32, i32, u32) {
    let by_i = i32::try_from(by).unwrap();
    match aim {
        Aim::Creature => (start.0 + by, start.1, start.2),
        Aim::Player => (start.0, start.1 - by_i, start.2),
        Aim::Walker => (start.0, start.1, start.2 - by),
    }
}

#[rstest]
fn a_pinger_you_control_is_followed_by_two_more(
    #[values(2, 3, 4)] seats: usize,
    #[values(Aim::Creature, Aim::Player, Aim::Walker)] at: Aim,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 0, ghyrson()))?;
    let start = marks(&driver);
    driver.begin_activation("Pinger")?;
    let target = aim(&driver, at);
    driver.choose_targets(&[target])?;
    settle(&mut driver, false)?;
    assert_eq!(marks(&driver), hit(at, start, 3), "1 and then Ghyrson's 2");
    rebuilds(&driver);
    Ok(())
}

#[rstest]
#[case::exactly_one("Zap", 3)]
#[case::three_is_not_one("Blast", 3)]
fn a_spell_you_control_triggers_only_for_exactly_one(
    #[values(2, 3, 4)] seats: usize,
    #[values(Aim::Creature, Aim::Player, Aim::Walker)] at: Aim,
    #[case] spell: &str,
    #[case] total: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 0, ghyrson()))?;
    let start = marks(&driver);
    let target = aim(&driver, at);
    driver.cast_at(spell, &[target])?;
    settle(&mut driver, false)?;
    assert_eq!(marks(&driver), hit(at, start, total), "{spell}");
    rebuilds(&driver);
    Ok(())
}

#[rstest]
fn ghyrsons_own_damage_does_not_trigger_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(Aim::Creature, Aim::Player, Aim::Walker)] at: Aim,
) -> Result<(), ScriptError> {
    let pinging = with_rule((*ghyrson()).clone(), PING);
    let mut driver = Driver::from_scenario(table(seats, 0, pinging))?;
    let start = marks(&driver);
    let offered = driver.offered_abilities(GHYRSON)?;
    assert_eq!(offered.len(), 1, "the added ping");
    driver.begin_ability(GHYRSON, offered[0])?;
    let target = aim(&driver, at);
    driver.choose_targets(&[target])?;
    settle(&mut driver, false)?;
    assert_eq!(
        marks(&driver),
        hit(at, start, 1),
        "Ghyrson is not another source"
    );
    Ok(())
}

#[rstest]
fn an_opponents_pinger_does_not_trigger_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] at_me: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 1, ghyrson()))?;
    driver.begin_activation("Foe Pinger")?;
    let target = if at_me {
        driver.target_player(0)
    } else {
        driver.target_named("Wall")?
    };
    let life = driver.state().player(pid(0)).unwrap().life();
    driver.choose_targets(&[target])?;
    settle(&mut driver, false)?;
    let wall = driver.id_of("Wall")?;
    let damage = driver
        .state()
        .object(wall)
        .unwrap()
        .permanent()
        .unwrap()
        .damage();
    let lost = life - driver.state().player(pid(0)).unwrap().life();
    assert_eq!((damage, lost), if at_me { (0, 1) } else { (1, 0) });
    Ok(())
}

#[rstest]
fn one_to_each_of_several_recipients_triggers_once_for_each(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 0, ghyrson()))?;
    let (_, life, _) = marks(&driver);
    driver.begin_cast("Split")?;
    let among = [
        aim(&driver, Aim::Creature),
        aim(&driver, Aim::Player),
        aim(&driver, Aim::Walker),
    ];
    driver.choose_targets(&among)?;
    driver.answer(Answer::DivideAmount(DivideChoice::Division(
        among.iter().map(|target| (*target, 1)).collect(),
    )))?;
    settle(&mut driver, false)?;
    assert_eq!(marks(&driver), (3, life - 3, 2), "three triggers, one each");
    rebuilds(&driver);
    Ok(())
}

#[rstest]
fn ward_counters_an_opponents_spell_unless_two_is_paid(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] pays: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 1, ghyrson()))?;
    let me = driver.target_named(GHYRSON)?;
    driver.cast_at("Foe Zap", &[me])?;
    let asked = settle(&mut driver, pays)?;
    assert!(asked, "ward asks for {{2}}");
    let id = driver.id_of(GHYRSON)?;
    let damage = driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .damage();
    assert_eq!(damage, u32::from(pays), "the spell resolves only when paid");
    let tapped = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| name_of(driver.state(), **id) == "Mountain")
        .filter(|id| {
            driver
                .state()
                .object(**id)
                .unwrap()
                .permanent()
                .unwrap()
                .tapped()
        })
        .count();
    assert_eq!(tapped, if pays { 2 } else { 0 });
    rebuilds(&driver);
    Ok(())
}
