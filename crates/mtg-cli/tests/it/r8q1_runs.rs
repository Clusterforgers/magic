//! Goal: audit fixes of round 8 play right in real games: last known information of a sacrificed creature, exile-instead counters, bans on activation, the defending player of an Aura, face-down looks, copies, delayed triggers and hands.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_possible_truncation
)]

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_cli::forge::script::Script;
use mtg_cli::translate::card::translate_script;
use mtg_cli::translate::ctx::World;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{CardsChoice, NameChoice, Target, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, free_creature, swamp};
use mtg_testkit::{Driver, Scenario, ready, with_counters};

/// The card a Forge script gives, with the rules the translator read; no gaps allowed.
fn card(text: &str, types: &[CardType], cost: &str, pt: Option<(i32, i32)>) -> Arc<CardDefinition> {
    let out = translate_script(&World::new(&[], BTreeMap::new()), &Script::parse(text));
    let face = &out.faces[0];
    assert!(face.gaps.is_empty(), "{:?}", face.gaps);
    let mut def = CardDefinition::new(&out.name).with_mana_cost(ManaCost::parse(cost).unwrap());
    for t in types {
        def = def.with_type(*t);
    }
    if let Some((p, t)) = pt {
        def = def.with_power_toughness(p, t);
    }
    for rules in &face.abilities {
        def = def.with_rules(serde_json::from_value::<Ability>(rules.clone()).unwrap());
    }
    Arc::new(def)
}

/// Plays until the stack is empty at priority: yes to questions, the names in `want` picked in cards and targets.
fn run(driver: &mut Driver, yes: bool, want: &[&str]) {
    for _ in 0..200 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return,
            DecisionKind::YesNo(o) if yes && !o.only_no => {
                driver.answer(Answer::YesNo(YesNoChoice(true))).unwrap();
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(false))).unwrap();
            }
            DecisionKind::ChooseCards(o) => {
                let picked: Vec<_> = o
                    .from
                    .iter()
                    .copied()
                    .filter(|i| {
                        want.contains(&driver.state().object(*i).unwrap().card().name.as_str())
                    })
                    .take(o.max as usize)
                    .collect();
                if picked.len() as u32 >= o.min && !picked.is_empty() {
                    driver
                        .answer(Answer::ChooseCards(CardsChoice(picked)))
                        .unwrap();
                } else {
                    driver.answer_passively().unwrap();
                }
            }
            DecisionKind::ChooseTargets(o) => {
                let picked: Vec<Target> = want
                    .iter()
                    .filter_map(|n| match *n {
                        "p0" => Some(driver.target_player(0)),
                        "p1" => Some(driver.target_player(1)),
                        _ => driver.target_named(n).ok(),
                    })
                    .filter(|t| o.legal.contains(t))
                    .take(o.max as usize)
                    .collect();
                if picked.len() as u32 >= o.min && !picked.is_empty() {
                    driver
                        .answer(Answer::ChooseTargets(TargetChoice::Targets(picked)))
                        .unwrap();
                } else {
                    driver.answer_passively().unwrap();
                }
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            DecisionKind::ChooseName(_) => {
                let name = want.first().copied().unwrap_or("");
                driver
                    .answer(Answer::ChooseName(NameChoice(name.into())))
                    .unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    panic!("the game did not settle");
}

/// Casts the named spell, paying automatically, and plays it out.
fn cast_and_run(driver: &mut Driver, name: &str, yes: bool, want: &[&str]) {
    driver.begin_cast(name).unwrap();
    run(driver, yes, want);
}

/// The names in a zone.
fn names(driver: &Driver, zone: Zone) -> Vec<String> {
    driver
        .state()
        .zone(zone)
        .iter()
        .map(|i| driver.state().object(*i).unwrap().card().name.clone())
        .collect()
}

/// The current power and toughness of a battlefield permanent.
fn pt(driver: &Driver, name: &str) -> (Option<i32>, Option<i32>) {
    let id = driver.id_of(name).unwrap();
    let c = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    (c.power, c.toughness)
}

const TIP: &str = "Name:Tip the Scales\nManaCost:2 B\nTypes:Sorcery\nA:SP$ Sacrifice | SacValid$ Creature | RememberSacrificed$ True | SubAbility$ DBImmediateTrigger | SpellDescription$ Sacrifice a creature.\nSVar:DBImmediateTrigger:DB$ ImmediateTrigger | Execute$ TrigPumpAll | ConditionDefined$ RememberedLKI | ConditionPresent$ Creature | RememberObjects$ RememberedLKI | SubAbility$ DBCleanup | SpellDescription$ When you do, all creatures get -X/-X until end of turn, where X is the sacrificed creature's toughness.\nSVar:TrigPumpAll:DB$ PumpAll | ValidCards$ Creature | NumAtt$ -X | NumDef$ -X\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\nSVar:X:TriggerRemembered$CardToughness\n";

fn main_phase() -> Scenario {
    Scenario::new(2).at(1, 0, mtg_engine::turn::Step::Main1)
}

#[test]
fn a_sacrificed_creature_gives_its_toughness_with_its_counters() {
    let tip = card(TIP, &[CardType::Sorcery], "{2}{B}", None);
    let fat = with_counters(ready(bear()), CounterKind::PlusOnePlusOne, 2);
    let scenario = main_phase()
        .hand(0, [tip])
        .battlefield(0, [fat, ready(swamp()), ready(swamp()), ready(swamp())])
        .battlefield(1, [ready(free_creature("Big", 6, 6))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    cast_and_run(&mut driver, "Tip the Scales", true, &["Grizzly Bears"]);
    assert!(
        names(
            &driver,
            Zone::Graveyard(mtg_core::ids::PlayerId::from_index(0))
        )
        .contains(&"Grizzly Bears".to_string())
    );
    assert_eq!(pt(&driver, "Big"), (Some(2), Some(2)));
}

/// A free sorcery with the rules given as JSON.
fn json_sorcery(name: &str, rules: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(ManaCost::parse("{0}").unwrap())
            .with_rules(serde_json::from_str::<Ability>(rules).unwrap()),
    )
}

#[test]
fn a_sacrificed_token_still_gives_its_toughness() {
    let tip = card(TIP, &[CardType::Sorcery], "{2}{B}", None);
    let make = json_sorcery(
        "Make Token",
        r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Tok","power":3,"toughness":3,"types":["Creature"]}}}}]}}"#,
    );
    let scenario = main_phase()
        .hand(0, [tip, make])
        .battlefield(0, [ready(swamp()), ready(swamp()), ready(swamp())])
        .battlefield(1, [ready(free_creature("Big", 6, 6))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    cast_and_run(&mut driver, "Make Token", true, &[]);
    cast_and_run(&mut driver, "Tip the Scales", true, &["Tok"]);
    assert_eq!(pt(&driver, "Big"), (Some(3), Some(3)));
}

#[test]
fn nothing_to_sacrifice_shrinks_nothing() {
    let tip = card(TIP, &[CardType::Sorcery], "{2}{B}", None);
    let scenario = main_phase()
        .hand(0, [tip])
        .battlefield(0, [ready(swamp()), ready(swamp()), ready(swamp())])
        .battlefield(1, [ready(free_creature("Big", 6, 6))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    cast_and_run(&mut driver, "Tip the Scales", true, &[]);
    assert_eq!(pt(&driver, "Big"), (Some(6), Some(6)));
}

const NO_MORE_LIES: &str = "Name:No More Lies\nManaCost:W U\nTypes:Instant\nA:SP$ Counter | TargetType$ Spell | UnlessCost$ 3 | TgtPrompt$ Select target spell | ValidTgts$ Card | Destination$ Exile | SpellDescription$ Counter target spell unless its controller pays {3}. If that spell is countered this way, exile it instead of putting it into its owner's graveyard.\n";

#[rstest::rstest]
#[case::countered_spell_is_exiled(false, true)]
#[case::uncounterable_spell_resolves_to_the_graveyard(true, false)]
fn a_spell_countered_this_way_is_exiled_and_one_that_is_not_is_not(
    #[case] uncounterable: bool,
    #[case] exiled: bool,
) {
    use mtg_testkit::cards::{island, plains};
    let lies = card(NO_MORE_LIES, &[CardType::Instant], "{W}{U}", None);
    let rules = if uncounterable {
        r#"{"Rule":{"rule":{"CantBeCountered":"This"},"from":"Stack"}}"#
    } else {
        r#"{"Spell":{"effects":[{"GainLife":{"amount":1}}]}}"#
    };
    let victim = Arc::new(
        CardDefinition::new("Victim")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse("{0}").unwrap())
            .with_rules(
                serde_json::from_str::<Ability>(
                    r#"{"Spell":{"effects":[{"GainLife":{"amount":1}}]}}"#,
                )
                .unwrap(),
            )
            .with_rules(serde_json::from_str::<Ability>(rules).unwrap()),
    );
    let scenario = Scenario::new(2)
        .at(1, 0, mtg_engine::turn::Step::Main1)
        .hand(0, [victim])
        .hand(1, [lies])
        .battlefield(
            1,
            [
                ready(island()),
                ready(plains()),
                ready(island()),
                ready(plains()),
            ],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Victim").unwrap();
    // The opponent answers with No More Lies on the victim.
    driver.pass().unwrap();
    let target = {
        let id = *driver
            .state()
            .stack()
            .iter()
            .find(|i| driver.state().object(**i).unwrap().card().name == "Victim")
            .unwrap();
        Target::Object(driver.state().object(id).unwrap().reference())
    };
    driver.begin_cast("No More Lies").unwrap();
    for _ in 0..4 {
        match driver.decision().unwrap().kind.clone() {
            DecisionKind::ChooseTargets(_) => {
                driver
                    .answer(Answer::ChooseTargets(TargetChoice::Targets(vec![target])))
                    .unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            _ => break,
        }
    }
    run(&mut driver, false, &[]);
    let seat = |i| mtg_core::ids::PlayerId::from_index(i);
    let in_exile = names(&driver, Zone::Exile).contains(&"Victim".to_string());
    let in_graveyard = names(&driver, Zone::Graveyard(seat(0))).contains(&"Victim".to_string());
    assert_eq!((in_exile, in_graveyard), (exiled, !exiled));
}

/// The Aura a Forge script gives.
fn aura(text: &str) -> Arc<CardDefinition> {
    let base = card(text, &[CardType::Enchantment], "{2}{W}", None);
    Arc::new((*base).clone().with_subtype("Aura"))
}

const ARREST: &str = "Name:Arrest\nManaCost:2 W\nTypes:Enchantment Aura\nK:Enchant:Creature\nS:Mode$ CantAttack,CantBlock,CantBeActivated | ValidCard$ Creature.EnchantedBy | Description$ Enchanted creature can't attack or block and its activated abilities can't be activated.\n";

fn llanowar() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Llanowar Elves")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(ManaCost::parse("{0}").unwrap())
            .with_rules(
                serde_json::from_str::<Ability>(
                    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#,
                )
                .unwrap(),
            ),
    )
}

#[rstest::rstest]
#[case::enchanted_creature_cannot_tap_for_mana(true, false)]
#[case::other_creature_still_taps(false, true)]
fn a_plain_ban_on_activation_includes_mana_abilities(#[case] enchanted: bool, #[case] taps: bool) {
    use mtg_testkit::scenario::attached_to;
    let host = ready(llanowar());
    let aura = if enchanted {
        attached_to(ready(aura(ARREST)), 0, 0)
    } else {
        ready(aura(ARREST))
    };
    let scenario = main_phase().battlefield(0, [host, aura]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    assert_eq!(driver.tap_for_mana("Llanowar Elves").is_ok(), taps);
}

const LATULLA: &str = "Name:Latulla's Orders\nManaCost:1 R\nTypes:Enchantment Aura\nK:Flash\nK:Enchant:Creature\nT:Mode$ DamageDone | ValidSource$ Card.AttachedBy | ValidTarget$ Player.TriggeredDefendingPlayer | CombatDamage$ True | Execute$ TrigDestroy | OptionalDecider$ You | TriggerDescription$ Whenever enchanted creature deals combat damage to defending player, you may destroy target artifact that player controls.\nSVar:TrigDestroy:DB$ Destroy | ValidTgts$ Artifact.ControlledBy TriggeredTarget | TgtPrompt$ Select target artifact damaged player controls\n";

#[rstest::rstest]
#[case::unblocked_attacker_destroys_an_artifact(false, false)]
#[case::blocked_attacker_deals_no_damage_to_the_player(true, true)]
fn an_aura_trigger_on_damage_to_the_defending_player(
    #[case] blocked: bool,
    #[case] survives: bool,
) {
    use mtg_testkit::cards::mountain;
    use mtg_testkit::scenario::attached_to;
    let art = Arc::new(
        CardDefinition::new("Their Rock")
            .with_type(CardType::Artifact)
            .with_mana_cost(ManaCost::parse("{0}").unwrap()),
    );
    let mut theirs = vec![ready(art)];
    if blocked {
        theirs.push(ready(free_creature("Wall", 0, 4)));
    }
    let scenario = main_phase()
        .battlefield(
            0,
            [
                ready(free_creature("Attacker", 2, 2)),
                attached_to(ready(aura(LATULLA)), 0, 0),
                ready(mountain()),
            ],
        )
        .battlefield(1, theirs);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver
        .pass_until(mtg_engine::turn::Step::DeclareAttackers)
        .unwrap();
    driver.attack(&[("Attacker", 1)]).unwrap();
    for _ in 0..60 {
        if driver.state().turn().step() == mtg_engine::turn::Step::Main2 {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseTargets(o) => {
                let rock = driver.target_named("Their Rock").unwrap();
                assert!(o.legal.contains(&rock));
                driver
                    .answer(Answer::ChooseTargets(TargetChoice::Targets(vec![rock])))
                    .unwrap();
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true))).unwrap();
            }
            DecisionKind::DeclareBlockers(_) if blocked => {
                driver.block(&[("Wall", "Attacker")]).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    let rock_stays = names(&driver, Zone::Battlefield).contains(&"Their Rock".to_string());
    assert_eq!(rock_stays, survives);
}

const EXTRACT_POWER: &str = "Name:Extract Power\nManaCost:5 U\nTypes:Sorcery\nA:SP$ Dig | Defined$ Player | DigNum$ 1 | ChangeNum$ All | DestinationZone$ Exile | ExileFaceDown$ True | WithMayLook$ True | RememberChanged$ True | SubAbility$ DBEffect | SpellDescription$ Look at the top card of each player's library, then exile those cards face down. You may play them without paying their mana costs for as long as they remain exiled.\nSVar:DBEffect:DB$ Effect | RememberObjects$ Remembered | StaticAbilities$ STPlay | Duration$ Permanent | ForgetOnMoved$ Exile | SubAbility$ DBCleanup\nSVar:STPlay:Mode$ Continuous | MayPlay$ True | Affected$ Card.IsRemembered | AffectedZone$ Exile | MayPlayWithoutManaCost$ True | Description$ You may play them without paying their mana costs for as long as they remain exiled.\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\n";

fn library_of(names: &[&str]) -> Vec<Arc<CardDefinition>> {
    names
        .iter()
        .map(|n| {
            Arc::new(
                CardDefinition::new(*n)
                    .with_type(CardType::Creature)
                    .with_power_toughness(2, 2)
                    .with_mana_cost(ManaCost::parse("{2}").unwrap()),
            )
        })
        .collect()
}

/// Whether each seat may look at the exiled card of this name.
fn looks(driver: &Driver, name: &str) -> (bool, bool) {
    let id = *driver
        .state()
        .zone(Zone::Exile)
        .iter()
        .find(|i| driver.state().object(**i).unwrap().card().name == name)
        .unwrap();
    let seat = mtg_core::ids::PlayerId::from_index;
    (
        driver.state().may_look_at(seat(0), id),
        driver.state().may_look_at(seat(1), id),
    )
}

#[test]
fn cards_exiled_face_down_by_a_spell_are_seen_by_its_caster_only() {
    let spell = card(EXTRACT_POWER, &[CardType::Sorcery], "{0}", None);
    let scenario = main_phase()
        .library(0, library_of(&["Mine", "Mine2"]))
        .library(1, library_of(&["Theirs", "Theirs2"]))
        .hand(0, [spell]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    cast_and_run(&mut driver, "Extract Power", true, &[]);
    // The caster looks at both; the owner of the opponent's card does not.
    assert_eq!(looks(&driver, "Mine"), (true, false));
    assert_eq!(looks(&driver, "Theirs"), (true, false));
}

#[test]
fn a_card_hidden_by_a_permanent_stays_seen_when_the_permanent_leaves() {
    let hider = Arc::new(
        CardDefinition::new("Hider")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(ManaCost::parse("{0}").unwrap())
            .with_rules(
                serde_json::from_str::<Ability>(
                    r#"{"Activated":{"cost":"{0}","effects":[{"LookAtTop":{"as":"top","who":"Opponents"}},{"ExileHidden":{"as":"exiled4","what":{"Bound":"top"}}},{"Rule":{"duration":"Forever","rule":{"MayCast":{"from":"Exile","only":{"Bound":"exiled4"},"play":true}}}}]}}"#,
                )
                .unwrap(),
            ),
    );
    let kill = Arc::new(
        CardDefinition::new("Kill")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse("{0}").unwrap())
            .with_rules(
                serde_json::from_str::<Ability>(
                    r#"{"Spell":{"effects":[{"Destroy":{"what":{"Target":0}}}],"targets":[{"what":{"Object":[]}}]}}"#,
                )
                .unwrap(),
            ),
    );
    let scenario = main_phase()
        .library(1, library_of(&["Theirs", "Theirs2"]))
        .battlefield(0, [ready(hider)])
        .hand(0, [kill]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Hider").unwrap();
    run(&mut driver, true, &[]);
    assert_eq!(looks(&driver, "Theirs"), (true, false));
    cast_and_run(&mut driver, "Kill", true, &["Hider"]);
    assert!(!names(&driver, Zone::Battlefield).contains(&"Hider".to_string()));
    assert_eq!(looks(&driver, "Theirs"), (true, false));
}

const FIERY_ENCORE: &str = "Name:Fiery Encore\nManaCost:4 R\nTypes:Sorcery\nA:SP$ Discard | Mode$ TgtChoose | SubAbility$ DBDraw | RememberDiscarded$ True | SpellDescription$ Discard a card, then draw a card.\nSVar:DBDraw:DB$ Draw | SubAbility$ DBImmediateTrigger\nSVar:DBImmediateTrigger:DB$ ImmediateTrigger | ConditionDefined$ Remembered | ConditionPresent$ Card.nonLand | Execute$ TrigDealDamage | RememberObjects$ RememberedCard | SubAbility$ DBCleanup | SpellDescription$ When you discard a nonland card this way, CARDNAME deals damage equal to that card's mana value to target creature or planeswalker.\nSVar:TrigDealDamage:DB$ DealDamage | ValidTgts$ Creature,Planeswalker | TgtPrompt$ Select target creature or planeswalker | NumDmg$ X\nSVar:X:TriggerRemembered$CardManaCost\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\nK:Storm\n";

#[test]
fn a_storm_copy_of_a_spell_with_a_reflexive_trigger_plays_through() {
    let encore = card(FIERY_ENCORE, &[CardType::Sorcery], "{0}", None);
    let filler = json_sorcery(
        "Filler",
        r#"{"Spell":{"effects":[{"GainLife":{"amount":1}}]}}"#,
    );
    let discard = |name: &str, cost: &str| {
        Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Creature)
                .with_power_toughness(1, 1)
                .with_mana_cost(ManaCost::parse(cost).unwrap()),
        )
    };
    let scenario = main_phase()
        .hand(
            0,
            [
                filler,
                encore,
                discard("Disc", "{3}"),
                discard("Disc2", "{2}"),
            ],
        )
        .library(0, library_of(&["Draw1", "Draw2", "Draw3"]))
        .battlefield(1, [ready(free_creature("Vic", 9, 9))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    cast_and_run(&mut driver, "Filler", true, &[]);
    cast_and_run(&mut driver, "Fiery Encore", true, &["Vic", "Disc", "Disc2"]);
    let id = driver.id_of("Vic").unwrap();
    // The copy and the spell each discard a card and deal its mana value (3 and 2) as damage.
    let damage = driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .damage();
    assert_eq!(damage, 5);
}

const FATAL_FISSURE: &str = "Name:Fatal Fissure\nManaCost:1 B\nTypes:Instant\nA:SP$ DelayedTrigger | ValidTgts$ Creature | Mode$ ChangesZone | RememberObjects$ Targeted | ValidCard$ Card.IsTriggerRemembered | Origin$ Battlefield | Destination$ Graveyard | ThisTurn$ True | Execute$ TrigEarthbend | TriggerDescription$ When that creature dies this turn, you earthbend 4. | SpellDescription$ Choose target creature. When that creature dies this turn, you earthbend 4.\nSVar:TrigEarthbend:DB$ Earthbend | Num$ 4\n";

#[rstest::rstest]
#[case::the_chosen_creature_dies(true, 4)]
#[case::another_creature_dies(false, 0)]
fn a_delayed_trigger_chooses_its_land_when_it_triggers(
    #[case] chosen_dies: bool,
    #[case] counters: u32,
) {
    use mtg_testkit::cards::forest;
    let fissure = card(FATAL_FISSURE, &[CardType::Instant], "{0}", None);
    let kill = json_sorcery(
        "Kill",
        r#"{"Spell":{"effects":[{"Destroy":{"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#,
    );
    let scenario = main_phase().hand(0, [fissure, kill]).battlefield(
        0,
        [
            ready(free_creature("Chosen", 1, 1)),
            ready(free_creature("Other", 1, 1)),
            ready(forest()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    // With no land the spell can still be cast: its only target is the creature.
    cast_and_run(&mut driver, "Fatal Fissure", true, &["Chosen"]);
    let victim = if chosen_dies { "Chosen" } else { "Other" };
    cast_and_run(&mut driver, "Kill", true, &[victim, "Forest"]);
    let forest = driver.id_of("Forest").unwrap();
    let characteristics =
        mtg_engine::characteristics::characteristics(driver.state(), forest).unwrap();
    assert_eq!(
        characteristics.types.contains(CardType::Creature),
        chosen_dies
    );
    assert_eq!(characteristics.power.unwrap_or(0), counters as i32);
}

const RESONANCE: &str = "Name:Resonance Technician\nManaCost:3 UR UR\nTypes:Creature Weird Detective\nPT:4/4\nA:AB$ CopySpellAbility | Cost$ T tapXType<X/Artifact/artifacts you control> | ValidTgts$ Instant.YouCtrl+cmcEQX,Sorcery.YouCtrl+cmcEQX | TargetType$ Spell | AILogic$ Once | MayChooseTarget$ True | SpellDescription$ Copy target instant or sorcery spell you control with mana value X. You may choose new targets for the copy.\nSVar:X:Count$xPaid\n";

#[rstest::rstest]
#[case::two_artifacts_copy_a_spell_of_value_two(2, true)]
#[case::one_artifact_cannot_copy_a_spell_of_value_two(1, false)]
fn a_cost_that_taps_x_artifacts_is_offered_when_some_x_has_a_target(
    #[case] artifacts: usize,
    #[case] offered: bool,
) {
    use mtg_testkit::cards::forest;
    let tech = card(RESONANCE, &[CardType::Creature], "{0}", Some((4, 4)));
    let rock = |n: usize| {
        ready(Arc::new(
            CardDefinition::new(format!("Rock{n}"))
                .with_type(CardType::Artifact)
                .with_mana_cost(ManaCost::parse("{0}").unwrap()),
        ))
    };
    let two = Arc::new(
        CardDefinition::new("Two")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse("{2}").unwrap()),
    );
    let mut field = vec![ready(tech), ready(forest()), ready(forest())];
    field.extend((0..artifacts).map(rock));
    let scenario = main_phase().hand(0, [two]).battlefield(0, field);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_cast("Two").unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    assert_eq!(
        !driver
            .offered_abilities("Resonance Technician")
            .unwrap()
            .is_empty(),
        offered
    );
}

const BRAIN_PRY: &str = "Name:Brain Pry\nManaCost:1 B\nTypes:Sorcery\nA:SP$ NameCard | Defined$ You | ValidCards$ Card.nonLand | ValidDescription$ nonland | SubAbility$ RevealHand | SpellDescription$ Choose a nonland card name. Target player reveals their hand. That player discards a card with that name. If they can't, you draw a card.\nSVar:RevealHand:DB$ RevealHand | RememberRevealed$ True | ValidTgts$ Player | SubAbility$ DBDiscard\nSVar:DBDiscard:DB$ Discard | Defined$ Targeted | NumCards$ 1 | Mode$ TgtChoose | DiscardValid$ Card.NamedCard | SubAbility$ DBDraw\nSVar:DBDraw:DB$ Draw | Defined$ You | NumCards$ 1 | ConditionDefined$ Remembered | ConditionPresent$ Card.NamedCard | ConditionCompare$ EQ0 | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True | ClearNamedCard$ True\n";

#[rstest::rstest]
#[case::the_named_card_is_discarded_and_nothing_is_drawn(&["Named", "Other"], 0)]
#[case::without_the_card_the_caster_draws(&["Other"], 1)]
fn brain_pry_draws_only_when_no_card_with_the_name_was_discarded(
    #[case] hand: &[&str],
    #[case] drawn: usize,
) {
    let pry = card(BRAIN_PRY, &[CardType::Sorcery], "{0}", None);
    let scenario = main_phase()
        .library(0, library_of(&["Top1", "Top2"]))
        .hand(0, [pry])
        .hand(1, library_of(hand));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    cast_and_run(&mut driver, "Brain Pry", true, &["Named", "p1"]);
    let seat = mtg_core::ids::PlayerId::from_index;
    assert_eq!(names(&driver, Zone::Hand(seat(0))).len(), drawn);
    assert_eq!(
        names(&driver, Zone::Graveyard(seat(1))).contains(&"Named".to_string()),
        hand.contains(&"Named")
    );
}
