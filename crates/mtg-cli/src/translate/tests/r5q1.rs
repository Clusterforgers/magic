//! Goal: audit faults found by playing translated cards stay fixed: a declined or partial "up to" choice resolves,
//! totals count every sacrificed object, a trigger's damage reaches only the player, and hidden looks stay hidden.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{
    CardsChoice, CostsChoice, ModeChoice, NumberChoice, Target, TargetChoice, YesNoChoice,
};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::forest;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use super::abilities;

fn seat(i: usize) -> PlayerId {
    PlayerId::from_index(i)
}

/// A free card of the given types with the abilities a Forge script translates to.
fn card(
    script: &str,
    name: &str,
    types: &[CardType],
    stats: Option<(i32, i32)>,
) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name).with_mana_cost(ManaCost::parse("{0}").unwrap());
    for ty in types {
        card = card.with_type(*ty);
    }
    if let Some((power, toughness)) = stats {
        card = card.with_power_toughness(power, toughness);
    }
    for value in abilities(script) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

fn library(n: usize) -> Vec<Arc<CardDefinition>> {
    (0..n).map(|_| forest()).collect()
}

fn names(driver: &Driver, zone: Zone) -> Vec<String> {
    let state = driver.state();
    state
        .zone(zone)
        .iter()
        .map(|id| state.object(*id).unwrap().card().name.clone())
        .collect()
}

fn power_of(driver: &Driver, name: &str) -> Option<i32> {
    let id = driver.id_of(name).ok()?;
    mtg_engine::characteristics::characteristics(driver.state(), id)?.power
}

/// Answers the announcements of a cast or activation in order, paying automatically, until priority or payment is done.
fn announce(driver: &mut Driver, mut targets: Vec<Vec<Target>>, mode: Option<Vec<usize>>, x: u32) {
    for _ in 0..30 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseCosts(_) => {
                driver
                    .answer(Answer::ChooseCosts(CostsChoice::Costs(vec![])))
                    .unwrap();
            }
            DecisionKind::ChooseMode(_) => {
                let modes = mode.clone().unwrap_or(vec![0]);
                driver
                    .answer(Answer::ChooseMode(ModeChoice::Modes(modes)))
                    .unwrap();
            }
            DecisionKind::ChooseNumber(_) => {
                driver
                    .answer(Answer::ChooseNumber(NumberChoice::Number(x)))
                    .unwrap();
            }
            DecisionKind::ChooseTargets(_) if !targets.is_empty() => {
                let chosen = targets.remove(0);
                driver
                    .answer(Answer::ChooseTargets(TargetChoice::Targets(chosen)))
                    .unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
                return;
            }
            DecisionKind::Priority(_) => return,
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
}

/// Resolves the stack: `take` cards of each card choice (`None` takes the most), yes or no to every question.
fn settle(driver: &mut Driver, yes: bool, take: Option<usize>) {
    for _ in 0..400 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return,
            DecisionKind::YesNo(o) if !o.only_no => {
                driver.answer(Answer::YesNo(YesNoChoice(yes))).unwrap();
            }
            DecisionKind::ChooseCards(o) => {
                let n = take
                    .unwrap_or(o.max as usize)
                    .clamp(o.min as usize, o.max as usize);
                let chosen = o.from.iter().copied().take(n.min(o.from.len())).collect();
                driver
                    .answer(Answer::ChooseCards(CardsChoice(chosen)))
                    .unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    panic!("the stack does not settle");
}

const PYRE: &str = "Name:Pyre\nTypes:Instant\nA:SP$ Discard | Mode$ TgtChoose | NumCards$ 2 | Optional$ True | RememberDiscarded$ True | SubAbility$ DBDraw\nSVar:DBDraw:DB$ Draw | NumCards$ X | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\nSVar:X:Remembered$Amount\n";

#[rstest]
#[case::none(0, 0)]
#[case::one(1, 1)]
#[case::two(2, 2)]
fn discarding_up_to_two_draws_that_many(#[case] discard: usize, #[case] drawn: usize) {
    let spell = card(PYRE, "Pyre", &[CardType::Instant], None);
    let scenario = Scenario::new(2)
        .hand(0, [spell, forest(), forest(), forest()])
        .library(0, library(9))
        .library(1, library(9));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_cast("Pyre").unwrap();
    announce(&mut driver, vec![], None, 0);
    settle(&mut driver, true, Some(discard));
    assert_eq!(
        names(&driver, Zone::Hand(seat(0))).len(),
        3 - discard + drawn
    );
    assert_eq!(names(&driver, Zone::Graveyard(seat(0))).len(), 1 + discard);
}

#[rstest]
#[case::declined(false, 3, 0)]
#[case::accepted(true, 3, 1)]
fn a_declined_may_leaves_the_count_of_its_binding_at_zero(
    #[case] yes: bool,
    #[case] hand: usize,
    #[case] graveyard: usize,
) {
    let script = PYRE
        .replace("NumCards$ 2", "NumCards$ 1")
        .replace("Optional$ True", "OptionalDecider$ You");
    let spell = card(&script, "Pyre", &[CardType::Instant], None);
    let scenario = Scenario::new(2)
        .hand(0, [spell, forest(), forest(), forest()])
        .library(0, library(9))
        .library(1, library(9));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_cast("Pyre").unwrap();
    announce(&mut driver, vec![], None, 0);
    settle(&mut driver, yes, None);
    // A declined May draws nothing; an accepted one discards a card and draws one.
    assert_eq!(names(&driver, Zone::Hand(seat(0))).len(), hand);
    assert_eq!(
        names(&driver, Zone::Graveyard(seat(0))).len(),
        1 + graveyard
    );
}

const REIGN: &str = "Name:Reign\nTypes:Sorcery\nA:SP$ Sacrifice | SacValid$ Creature | Defined$ Player | SubAbility$ DBToken | RememberSacrificed$ True\nSVar:DBToken:DB$ Token | TokenScript$ b_x_x_demon_flying | TokenOwner$ You | TokenPower$ X | TokenToughness$ X | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\nSVar:X:RememberedLKI$CardPower\n";
const COBBLE: &str = "Name:Cobble\nTypes:Instant\nA:SP$ Token | Cost$ Sac<X/Creature/creature> | TokenScript$ ub_x_x_zombie_menace | AnnounceTitle$ number of creatures to sacrifice | TokenPower$ Y | TokenToughness$ Y\nSVar:X:Count$xPaid\nSVar:Y:Sacrificed$CardPower\n";

fn creature(name: &str, power: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(power, power)
            .with_mana_cost(ManaCost::parse("{1}").unwrap()),
    )
}

#[test]
fn a_token_as_big_as_the_total_power_of_what_each_player_sacrificed() {
    let spell = card(REIGN, "Reign", &[CardType::Sorcery], None);
    let scenario = Scenario::new(3)
        .hand(0, [spell])
        .battlefield(0, [ready(creature("A", 2))])
        .battlefield(1, [ready(creature("B", 3))])
        .battlefield(2, [ready(creature("C", 4))])
        .library(0, library(9))
        .library(1, library(9))
        .library(2, library(9));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_cast("Reign").unwrap();
    announce(&mut driver, vec![], None, 0);
    settle(&mut driver, true, None);
    assert_eq!(power_of(&driver, "Demon"), Some(9));
}

#[rstest]
#[case::one(1, Some(2))]
#[case::two(2, Some(5))]
#[case::none(0, None)]
fn a_token_as_big_as_the_total_power_of_the_creatures_sacrificed_to_cast(
    #[case] sacrificed: u32,
    #[case] power: Option<i32>,
) {
    let spell = card(COBBLE, "Cobble", &[CardType::Instant], None);
    let scenario = Scenario::new(2)
        .hand(0, [spell])
        .battlefield(0, [ready(creature("A", 2)), ready(creature("B", 3))])
        .library(0, library(9))
        .library(1, library(9));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_cast("Cobble").unwrap();
    announce(&mut driver, vec![], None, sacrificed);
    settle(&mut driver, true, None);
    assert_eq!(power_of(&driver, "Zombie"), power);
}

const MENTOR: &str = "Name:Mentor\nTypes:Creature Human Cleric\nPT:2/2\nT:Mode$ AbilityCast | ValidActivatingPlayer$ Opponent | ValidCard$ Artifact.inZoneBattlefield,Creature.inZoneBattlefield,Land.inZoneBattlefield | ValidSA$ SpellAbility.!ManaAbility | TriggerZones$ Battlefield | Execute$ TrigDmg | TriggerDescription$ x\nSVar:TrigDmg:DB$ DealDamage | NumDmg$ 2 | Defined$ TriggeredActivator\n";
const SHAMAN: &str = "Name:Shaman\nTypes:Creature Centaur Shaman\nPT:3/4\nT:Mode$ AbilityCast | ValidActivatingPlayer$ Player | ValidSA$ SpellAbility.!ManaAbility | TriggerZones$ Battlefield | Execute$ TrigDmg | TriggerDescription$ x\nSVar:TrigDmg:DB$ DealDamage | NumDmg$ 1 | Defined$ TriggeredActivator\n";

#[rstest]
#[case::opponent_activates(MENTOR, 2)]
#[case::opponent_activates_every_ability(SHAMAN, 1)]
fn damage_for_an_activation_hits_the_player_and_spares_the_activated_permanent(
    #[case] script: &str,
    #[case] damage: i32,
) {
    let watcher = card(script, "Watcher", &[CardType::Creature], Some((2, 2)));
    let worker = Arc::new(
        CardDefinition::new("Worker")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(ManaCost::parse("{0}").unwrap())
            .with_rules(
                serde_json::from_str::<Ability>(
                    r#"{"Activated":{"cost":"{0}","effects":[{"GainLife":{"amount":1}}]}}"#,
                )
                .unwrap(),
            ),
    );
    let scenario = Scenario::new(2)
        .at(3, 1, mtg_engine::turn::Step::Main1)
        .battlefield(0, [ready(watcher)])
        .battlefield(1, [ready(worker)])
        .library(0, library(9))
        .library(1, library(9));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Worker").unwrap();
    announce(&mut driver, vec![], None, 0);
    settle(&mut driver, true, None);
    let life = driver.state().player(seat(1)).unwrap().life();
    assert_eq!(life, 20 + 1 - damage);
    assert!(names(&driver, Zone::Battlefield).contains(&"Worker".to_owned()));
}

const SISTERS: &str = "Name:Sisters\nTypes:Creature Gorgon\nPT:7/5\nA:AB$ ChangeZone | Cost$ 0 | Origin$ Battlefield | Destination$ Exile | ValidTgts$ Creature | TgtPrompt$ x | SpellDescription$ x\nA:AB$ ChangeZone | Cost$ 0 | Origin$ Exile | Destination$ Battlefield | ChangeType$ Creature.ExiledWithSource | Hidden$ True | Mandatory$ True | ChangeNum$ 1 | GainControl$ True | SpellDescription$ x\n";

#[rstest]
#[case::from_an_opponent(1)]
#[case::from_you(0)]
fn a_creature_exiled_with_the_source_returns_whoever_owned_it(#[case] owner: usize) {
    let sisters = card(SISTERS, "Sisters", &[CardType::Creature], Some((7, 5)));
    let victim = ready(creature("Victim", 3));
    let (mine, theirs) = if owner == 0 {
        (vec![ready(sisters), victim], vec![])
    } else {
        (vec![ready(sisters)], vec![victim])
    };
    let scenario = Scenario::new(2)
        .at(3, 0, mtg_engine::turn::Step::Main1)
        .battlefield(0, mine)
        .battlefield(1, theirs)
        .library(0, library(9))
        .library(1, library(9));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let victim = driver.target_named("Victim").unwrap();
    driver.begin_ability("Sisters", 0).unwrap();
    announce(&mut driver, vec![vec![victim]], None, 0);
    settle(&mut driver, true, None);
    assert_eq!(names(&driver, Zone::Exile), ["Victim"]);
    driver.begin_ability("Sisters", 1).unwrap();
    announce(&mut driver, vec![], None, 0);
    settle(&mut driver, true, None);
    assert!(names(&driver, Zone::Exile).is_empty());
    assert!(names(&driver, Zone::Battlefield).contains(&"Victim".to_owned()));
}

#[rstest]
#[case::first("EQ1", true)]
#[case::at_most("LE1", true)]
#[case::at_least("GE2", false)]
fn a_cast_count_that_later_spells_change_is_not_rechecked_at_resolution(
    #[case] compare: &str,
    #[case] gapped: bool,
) {
    let script = format!(
        "Name:Gargoyle\nTypes:Artifact Creature Gargoyle\nPT:4/5\nT:Mode$ SpellCast | ValidCard$ Card.nonCreature | CheckSVar$ X | SVarCompare${compare} | TriggerZones$ Battlefield | Execute$ TrigCounter | TriggerDescription$ x\nSVar:TrigCounter:DB$ Counter | Defined$ TriggeredSpellAbility\nSVar:X:Count$ThisTurnCast_Card.nonCreature\n"
    );
    let named = super::gaps(&script)
        .iter()
        .any(|gap| gap.contains("SpellCast-exact-count-recheck"));
    assert_eq!(named, gapped);
}

#[rstest]
#[case::look_at("look at target player's hand. You may choose a card from it", false)]
#[case::reveals("target player reveals their hand. You choose a card from it", true)]
fn only_a_hand_the_text_reveals_is_shown_to_every_seat(#[case] words: &str, #[case] revealed: bool) {
    let script = format!(
        "Name:Gearhulk\nTypes:Artifact Creature Construct\nPT:4/4\nT:Mode$ ChangesZone | ValidCard$ Card.Self | Origin$ Any | Destination$ Battlefield | Execute$ TrigDiscard | TriggerDescription$ x\nSVar:TrigDiscard:DB$ Discard | ValidTgts$ Player | Mode$ RevealYouChoose | Optional$ True | DiscardValid$ Card\nOracle:When this creature enters, {words}.\n"
    );
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", super::gaps(&script));
    assert_eq!(found[0].to_string().contains("\"Reveal\""), revealed);
    assert!(found[0].to_string().contains("\"by\":\"You\""));
}
