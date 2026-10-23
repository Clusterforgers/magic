//! Goal: the four Umbra Auras of the precons (Eel, Snake, Bear and Octopus Umbra) save the creature they enchant at tables of two, three and four seats: a destroy effect, a destroy that can't be regenerated (Wrath of God), burn of lethal damage and lethal damage or deathtouch in combat each remove all damage from the creature and destroy the Aura instead, the Aura's own pump and granted ability go with it, an indestructible creature never uses the armor, a second Aura saves the creature from the next destruction and the third kills it, and a regeneration shield and the armor are offered together so that the controller picks which one is used up; every game replays from its log.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::names;
use mtg_cards::keyword::Keyword;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::ReplacementChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::PermanentState;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{burn, forest, free_creature, keyworded, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::prevent_cards::smite;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

/// Passes priority until the stack is empty, answering each question of the replacement step with `pick`.
fn settle(driver: &mut Driver, pick: usize) -> Result<(), ScriptError> {
    for _ in 0..400 {
        let Some(decision) = driver.engine().decision().cloned() else {
            return Ok(());
        };
        match decision.kind {
            DecisionKind::ChooseReplacement(_) => {
                driver.answer(Answer::ChooseReplacement(ReplacementChoice(pick)))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// A table of `seats` where every seat has cards to draw.
fn table(seats: usize) -> Scenario {
    (0..seats).fold(Scenario::new(seats), |scenario, seat| {
        scenario.library(seat, (0..10).map(|_| forest()))
    })
}

/// Seat 1, the next to hold priority, casts `spell` at the creature named `Subject`; Wrath of God is a sorcery that
/// seat 0 casts in its own turn.
fn destroys(
    driver: &mut Driver,
    destroyer: Destroyer,
    spell: &str,
    pick: usize,
) -> Result<(), ScriptError> {
    if matches!(destroyer, Destroyer::Wrath) {
        driver.cast(spell)?;
    } else {
        driver.pass()?;
        let at = driver.target_named("Subject")?;
        driver.cast_at(spell, &[at])?;
    }
    settle(driver, pick)
}

fn count(driver: &Driver, wanted: impl Fn(&Event) -> bool) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|e| wanted(e))
        .count()
}

fn removed(driver: &Driver) -> usize {
    count(driver, |event| matches!(event, Event::DamageRemoved { .. }))
}

fn deaths(driver: &Driver) -> usize {
    count(driver, |event| {
        matches!(event, Event::CreatureDied { .. })
            || matches!(
                event,
                Event::ZoneChanged {
                    from: Zone::Battlefield,
                    cause: mtg_engine::events::ZoneCause::Destroyed
                        | mtg_engine::events::ZoneCause::Buried,
                    ..
                }
            )
    })
}

fn permanent(driver: &Driver, name: &str) -> PermanentState {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .cloned()
        .unwrap()
}

fn on_battlefield(driver: &Driver, name: &str) -> bool {
    names(driver.state(), Zone::Battlefield).contains(&name.to_owned())
}

fn in_graveyard(driver: &Driver, seat: usize, name: &str) -> bool {
    names(driver.state(), Zone::Graveyard(common::pid(seat))).contains(&name.to_owned())
}

/// The power and toughness the layers leave the object.
fn size(driver: &Driver, id: mtg_core::ids::ObjectId) -> Option<(i32, i32)> {
    let shown = characteristics(driver.state(), id)?;
    Some((shown.power?, shown.toughness?))
}

fn replays(driver: &Driver, setup: &mtg_engine::GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// What seat 1 destroys the creature with.
#[derive(Clone, Copy, Debug)]
enum Destroyer {
    /// A spell that destroys target creature.
    Effect,
    /// Wrath of God, whose destruction can't be regenerated.
    Wrath,
    /// A burn of more damage than any of the creatures has toughness.
    Burn,
}

impl Destroyer {
    /// The cards of seat 1's hand and lands, `copies` of the spell named `Foe0`, `Foe1` and so on.
    fn hand(self, copies: usize) -> Vec<std::sync::Arc<mtg_cards::CardDefinition>> {
        (0..copies)
            .map(|at| match self {
                Destroyer::Effect => smite(&format!("Foe{at}")).unwrap(),
                Destroyer::Wrath => {
                    let wrath = card("Wrath of God").unwrap();
                    let mut named = (*wrath).clone();
                    named.name = format!("Foe{at}");
                    std::sync::Arc::new(named)
                }
                Destroyer::Burn => burn(&format!("Foe{at}"), 12),
            })
            .collect()
    }

    fn lands(self, copies: usize) -> Vec<mtg_engine::PresetPermanent> {
        match self {
            Destroyer::Effect => (0..copies).map(|_| ready(mountain())).collect(),
            Destroyer::Wrath => (0..4 * copies).map(|_| ready(plains())).collect(),
            Destroyer::Burn => (0..copies).map(|_| ready(mountain())).collect(),
        }
    }
}

/// Seat 0 controls the `Subject` (a 2/2) with these Auras attached and a Forest; seat 1 holds the destroyer (seat 0 holds Wrath of God).
fn scenario(seats: usize, auras: &[&str], destroyer: Destroyer, copies: usize) -> Scenario {
    let mut field = vec![ready(free_creature("Subject", 2, 2)), ready(forest())];
    field.extend(
        auras
            .iter()
            .map(|aura| attached_to(ready(card(aura).unwrap()), 0, 0)),
    );
    let (hand, lands) = (destroyer.hand(copies), destroyer.lands(copies));
    if matches!(destroyer, Destroyer::Wrath) {
        field.extend(lands);
        return table(seats).battlefield(0, field).hand(0, hand);
    }
    table(seats)
        .battlefield(0, field)
        .hand(1, hand)
        .battlefield(1, lands)
}

#[rstest]
fn an_umbra_aura_saves_its_creature_from_a_destruction_and_is_destroyed_in_its_place(
    #[values(2, 3, 4)] seats: usize,
    #[values("Eel Umbra", "Snake Umbra", "Bear Umbra", "Octopus Umbra")] aura: &str,
    #[values(Destroyer::Effect, Destroyer::Wrath, Destroyer::Burn)] destroyer: Destroyer,
) -> Result<(), ScriptError> {
    let setup = scenario(seats, &[aura], destroyer, 1).setup().unwrap();
    let mut driver = Driver::from_scenario(scenario(seats, &[aura], destroyer, 1))?;
    assert!(on_battlefield(&driver, aura));
    destroys(&mut driver, destroyer, "Foe0", 0)?;
    assert!(on_battlefield(&driver, "Subject"), "the creature is saved");
    assert_eq!(permanent(&driver, "Subject").damage(), 0);
    assert!(!on_battlefield(&driver, aura));
    assert!(in_graveyard(&driver, 0, aura));
    assert_eq!(removed(&driver), 1);
    assert_eq!(deaths(&driver), 1, "only the Aura was destroyed");
    // The Aura's pump went with it.
    let id = driver.id_of("Subject")?;
    assert_eq!(size(&driver, id), Some((2, 2)));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn the_aura_pump_applies_while_it_is_attached_and_the_armor_ends_with_the_aura(
    #[values(2, 3, 4)] seats: usize,
    #[values(("Eel Umbra", 3), ("Snake Umbra", 3), ("Bear Umbra", 4), ("Octopus Umbra", 8))] aura: (
        &str,
        i32,
    ),
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(scenario(seats, &[aura.0], Destroyer::Effect, 2))?;
    let id = driver.id_of("Subject")?;
    assert_eq!(
        size(&driver, id).map(|(_, toughness)| toughness),
        Some(aura.1)
    );
    Ok(())
}

#[rstest]
fn a_second_aura_saves_the_creature_from_the_next_destruction_and_the_third_kills_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(Destroyer::Effect, Destroyer::Burn)] destroyer: Destroyer,
    #[values(0, 1)] pick: usize,
) -> Result<(), ScriptError> {
    let auras = ["Eel Umbra", "Snake Umbra"];
    let setup = scenario(seats, &auras, destroyer, 3).setup().unwrap();
    let mut driver = Driver::from_scenario(scenario(seats, &auras, destroyer, 3))?;
    destroys(&mut driver, destroyer, "Foe0", pick)?;
    // The controller picked which of the two Auras is destroyed; the other is still attached.
    let left = auras
        .iter()
        .filter(|aura| on_battlefield(&driver, aura))
        .count();
    assert_eq!(left, 1);
    assert!(on_battlefield(&driver, "Subject"));
    destroys(&mut driver, destroyer, "Foe1", pick)?;
    assert!(on_battlefield(&driver, "Subject"));
    assert!(auras.iter().all(|aura| in_graveyard(&driver, 0, aura)));
    assert_eq!(removed(&driver), 2);
    destroys(&mut driver, destroyer, "Foe2", pick)?;
    assert!(!on_battlefield(&driver, "Subject"));
    assert!(in_graveyard(&driver, 0, "Subject"));
    assert_eq!(removed(&driver), 2);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn an_indestructible_creature_is_never_destroyed_so_the_armor_stays_unused(
    #[values(2, 3, 4)] seats: usize,
    #[values(Destroyer::Effect, Destroyer::Burn)] destroyer: Destroyer,
) -> Result<(), ScriptError> {
    let build = || {
        table(seats)
            .battlefield(
                0,
                [
                    ready(keyworded("Subject", 2, 2, &[Keyword::Indestructible])),
                    attached_to(ready(card("Eel Umbra").unwrap()), 0, 0),
                ],
            )
            .hand(1, destroyer.hand(1))
            .battlefield(1, destroyer.lands(1))
    };
    let setup = build().setup().unwrap();
    let mut driver = Driver::from_scenario(build())?;
    destroys(&mut driver, destroyer, "Foe0", 0)?;
    assert!(on_battlefield(&driver, "Subject"));
    assert!(on_battlefield(&driver, "Eel Umbra"));
    assert_eq!(removed(&driver), 0);
    assert_eq!(deaths(&driver), 0);
    replays(&driver, &setup);
    Ok(())
}

/// What blocks the creature with the Aura.
#[derive(Clone, Copy, Debug)]
enum Blocker {
    /// A creature with power for lethal damage.
    Big,
    /// A creature with 1 power and deathtouch.
    Deathtouch,
}

#[rstest]
fn lethal_damage_and_deathtouch_in_combat_are_removed_and_the_aura_is_destroyed_instead(
    #[values(2, 3, 4)] seats: usize,
    #[values(Blocker::Big, Blocker::Deathtouch)] blocker: Blocker,
    #[values(("Eel Umbra", 3), ("Bear Umbra", 4))] aura: (&str, u32),
) -> Result<(), ScriptError> {
    let (aura, power) = aura;
    let guard = match blocker {
        Blocker::Big => free_creature("Guard", 6, 6),
        Blocker::Deathtouch => keyworded("Guard", 1, 6, &[Keyword::Deathtouch]),
    };
    let build = || {
        table(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(
                0,
                [
                    ready(free_creature("Subject", 2, 2)),
                    attached_to(ready(card(aura).unwrap()), 0, 0),
                ],
            )
            .battlefield(1, [ready(guard.clone())])
    };
    let setup = build().setup().unwrap();
    let mut driver = Driver::from_scenario(build())?;
    driver.attack(&[("Subject", 1)])?;
    // A trigger of the Aura goes on the stack and resolves first.
    for _ in 0..20 {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            break;
        }
        driver.pass()?;
    }
    driver.block(&[("Guard", "Subject")])?;
    driver.idle_until(2, Step::EndCombat)?;
    assert!(on_battlefield(&driver, "Subject"), "the attacker is saved");
    assert_eq!(permanent(&driver, "Subject").damage(), 0);
    assert!(in_graveyard(&driver, 0, aura));
    assert_eq!(removed(&driver), 1);
    assert!(
        !driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::CreatureDied { .. }))
    );
    // It dealt its damage to the blocker and is still in combat.
    assert_eq!(permanent(&driver, "Guard").damage(), power);
    replays(&driver, &setup);
    Ok(())
}

/// Which effect the controller of the creature uses up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Saved {
    /// The armor: the Aura is destroyed and the shield stays.
    ByArmor,
    /// The regeneration shield: the creature is tapped and the Aura stays.
    ByRegeneration,
}

#[rstest]
#[case::armor(0, Saved::ByArmor)]
#[case::regeneration(1, Saved::ByRegeneration)]
fn a_regeneration_shield_and_the_armor_are_offered_together_and_the_controller_picks(
    #[values(2, 3, 4)] seats: usize,
    #[case] pick: usize,
    #[case] saved: Saved,
) -> Result<(), ScriptError> {
    let build = || {
        table(seats)
            .hand(0, vec![card("Death Ward").unwrap()])
            .battlefield(
                0,
                [
                    ready(free_creature("Subject", 2, 2)),
                    attached_to(ready(card("Eel Umbra").unwrap()), 0, 0),
                    ready(plains()),
                ],
            )
            .hand(1, Destroyer::Effect.hand(1))
            .battlefield(1, Destroyer::Effect.lands(1))
    };
    let setup = build().setup().unwrap();
    let mut driver = Driver::from_scenario(build())?;
    let at = driver.target_named("Subject")?;
    driver.cast_at("Death Ward", &[at])?;
    settle(&mut driver, 0)?;
    // The destruction asks seat 0 which of the two applies first.
    driver.pass()?;
    let at = driver.target_named("Subject")?;
    driver.cast_at("Foe0", &[at])?;
    let mut asked = false;
    for _ in 0..40 {
        match driver
            .engine()
            .decision()
            .map(|d| (d.player, d.kind.clone()))
        {
            Some((player, DecisionKind::ChooseReplacement(options))) => {
                assert_eq!(player, common::pid(0));
                assert_eq!(options.candidates.len(), 2);
                asked = true;
                driver.answer(Answer::ChooseReplacement(ReplacementChoice(pick)))?;
            }
            Some((_, DecisionKind::Priority(_)))
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                break;
            }
            Some((_, DecisionKind::Priority(_))) => {
                driver.pass()?;
            }
            Some(_) => {
                driver.answer_passively()?;
            }
            None => break,
        }
    }
    assert!(asked, "the controller was asked");
    assert!(on_battlefield(&driver, "Subject"));
    assert_eq!(permanent(&driver, "Subject").damage(), 0);
    match saved {
        Saved::ByArmor => {
            assert!(in_graveyard(&driver, 0, "Eel Umbra"));
            assert!(!permanent(&driver, "Subject").tapped());
            assert_eq!(removed(&driver), 1);
        }
        Saved::ByRegeneration => {
            assert!(on_battlefield(&driver, "Eel Umbra"));
            assert!(permanent(&driver, "Subject").tapped());
            assert_eq!(removed(&driver), 0);
            assert_eq!(
                count(&driver, |e| matches!(e, Event::Regenerated { .. })),
                1
            );
        }
    }
    replays(&driver, &setup);
    Ok(())
}
