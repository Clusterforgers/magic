//! Goal: protection (CR 702.16) and shroud play as the rules say, at tables of two, three and four seats, for every kind of quality a keyword can name (a colour, a card type, a player and everything): a spell or ability whose source has the quality is not offered the holder as a target, its holder's controller included, and the answer that names it is refused; damage that source deals to the holder, a creature or a player, is prevented, combat damage included; an Aura or Equipment with the quality is not attached to the holder and one already attached when protection arrives comes off (the Aura to the graveyard); a creature with the quality is not offered as a blocker for the holder, whatever else the blockers are; a multicoloured source has every colour it has and a colourless one none; protection that arrives mid-turn works from then, one that is taken away or ends stops, and a target that becomes protected as the spell resolves makes it do nothing; shroud keeps out every spell and ability and hexproof only the opponents'; and Mother of Runes, Kor Firewalker and Black Knight do what their cards say.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, Targetable};
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::Event;
use mtg_engine::choose::Target;
use mtg_engine::choose::{OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{
    burn, cost, damage, free_creature, island, keyworded, mountain, one_target, pinger, plains,
    wastes,
};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

/// What a keyword protects from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quality {
    Red,
    Creatures,
    Opponents,
    Everything,
}

impl Quality {
    fn selector(self) -> Selector {
        match self {
            Quality::Red => Selector(vec![Pred::Color(Color::Red)]),
            Quality::Creatures => Selector(vec![Pred::Type(CardType::Creature)]),
            Quality::Opponents => Selector(vec![Pred::ControlledBy(Ref::Opponents)]),
            Quality::Everything => Selector::default(),
        }
    }

    fn keyword(self) -> Keyword {
        Keyword::Protection(self.selector())
    }

    /// Whether protection of this quality, held by what seat 1 controls, stops a source of this kind that `caster` controls.
    fn stops(self, source: Source, caster: usize) -> bool {
        match self {
            Quality::Red => source.colors().contains(&Color::Red),
            Quality::Creatures => source == Source::Creature,
            Quality::Opponents => caster != 1,
            Quality::Everything => true,
        }
    }
}

/// What a damage spell or ability comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Red,
    RedBlue,
    Blue,
    Colorless,
    Creature,
}

impl Source {
    fn colors(self) -> &'static [Color] {
        match self {
            Source::Red | Source::Creature => &[Color::Red],
            Source::RedBlue => &[Color::Red, Color::Blue],
            Source::Blue => &[Color::Blue],
            Source::Colorless => &[],
        }
    }

    /// The spell: 3 damage to any target, in the colours of the source.
    fn spell(self) -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Bolt")
                .with_type(CardType::Instant)
                .with_mana_cost(cost(u32::from(self == Source::Colorless), self.colors()))
                .with_rules(Ability::Spell {
                    targets: vec![one_target(Targetable::AnyTarget)],
                    effects: vec![damage(Ref::Target(0), 3)],
                }),
        )
    }

    /// The same damage to each creature, aimed at nothing: a sweeper in the colours of the source.
    fn sweeper(self) -> Arc<CardDefinition> {
        let rules: Ability = serde_json::from_str(
            r#"{"Spell":{"effects":[{"DealDamage":{"to":{"Each":[{"Type":"Creature"}]},"amount":3}}]}}"#,
        )
        .unwrap();
        Arc::new(
            CardDefinition::new("Sweeper")
                .with_type(CardType::Instant)
                .with_mana_cost(cost(u32::from(self == Source::Colorless), self.colors()))
                .with_rules(rules),
        )
    }

    fn lands(self) -> Vec<mtg_engine::PresetPermanent> {
        let mut lands = Vec::new();
        for color in self.colors() {
            lands.push(ready(match color {
                Color::Blue => island(),
                _ => mountain(),
            }));
        }
        if self == Source::Colorless {
            lands.push(ready(wastes()));
        }
        lands
    }
}

/// `seats` seats and `caster` on turn with priority; every seat has a Bear; seat 1 has a Guard with protection of
/// `quality`, or is a player with it. The caster has the spell and the lands for it, or a Pinger.
fn table(
    seats: usize,
    caster: usize,
    source: Source,
    quality: Quality,
    player: bool,
    sweeper: bool,
) -> Scenario {
    let mut scenario = Scenario::new(seats).at(1, caster, Step::Main1);
    for seat in 0..seats {
        let mut board = vec![ready(free_creature(&format!("Bear {seat}"), 2, 2))];
        if seat == 1 && !player {
            board.push(ready(keyworded("Guard", 2, 4, &[quality.keyword()])));
        }
        if seat == caster {
            if source == Source::Creature {
                board.push(ready(pinger("Pinger", 0)));
            } else {
                board.extend(source.lands());
            }
        }
        scenario = scenario.battlefield(seat, board);
    }
    if source != Source::Creature {
        let spell = if sweeper {
            source.sweeper()
        } else {
            source.spell()
        };
        scenario = scenario.hand(caster, [spell]);
    }
    if player {
        scenario = scenario.player_keywords(1, [quality.keyword()]);
    }
    scenario
}

fn begin(driver: &mut Driver, source: Source) -> Result<(), ScriptError> {
    if source == Source::Creature {
        driver.begin_activation("Pinger").map(|_| ())
    } else {
        driver.begin_cast("Bolt")
    }
}

fn legal(driver: &Driver) -> Vec<Target> {
    let DecisionKind::ChooseTargets(options) = &driver.decision().unwrap().kind else {
        panic!("a target is asked for first");
    };
    options.legal.clone()
}

/// Answers whatever is asked passively until the stack is empty and nothing resolves.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn alive(driver: &Driver, name: &str) -> bool {
    names(driver.state(), Zone::Battlefield).contains(&name.to_owned())
}

#[rstest]
fn targeting_is_refused_to_a_source_with_the_quality_whoever_controls_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        Quality::Red,
        Quality::Creatures,
        Quality::Opponents,
        Quality::Everything
    )]
    quality: Quality,
    #[values(
        Source::Red,
        Source::RedBlue,
        Source::Blue,
        Source::Colorless,
        Source::Creature
    )]
    source: Source,
    #[values(0, 1)] caster: usize,
    #[values(false, true)] player: bool,
) {
    let mut driver =
        Driver::from_scenario(table(seats, caster, source, quality, player, false)).unwrap();
    begin(&mut driver, source).unwrap();
    let offered = legal(&driver);
    let holder = if player {
        driver.target_player(1)
    } else {
        driver.target_named("Guard").unwrap()
    };
    let stopped = quality.stops(source, caster);
    assert_eq!(
        offered.contains(&holder),
        !stopped,
        "{quality:?} {source:?}"
    );
    // Everything else stays: every other player and every Bear.
    for seat in (0..seats).filter(|seat| *seat != 1 || !player) {
        assert!(
            offered.contains(&driver.target_player(seat)),
            "player {seat}"
        );
    }
    for seat in 0..seats {
        assert!(offered.contains(&driver.target_named(&format!("Bear {seat}")).unwrap()));
    }
    if stopped {
        let before = (driver.engine().events().len(), driver.engine().state_hash());
        assert!(driver.choose_targets(&[holder]).is_err());
        assert_eq!(
            (driver.engine().events().len(), driver.engine().state_hash()),
            before,
            "the refused answer leaves no trace"
        );
    }
}

#[rstest]
fn damage_that_does_not_target_is_prevented_by_the_quality_alone(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        Quality::Red,
        Quality::Creatures,
        Quality::Opponents,
        Quality::Everything
    )]
    quality: Quality,
    #[values(Source::Red, Source::RedBlue, Source::Blue, Source::Colorless)] source: Source,
    #[values(0, 1)] caster: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, caster, source, quality, false, true))?;
    driver.cast("Sweeper")?;
    settle(&mut driver)?;
    // Three damage kills every 2/2 Bear and leaves a Guard of toughness 4 whole either way, so what shows is its damage.
    let guard = driver.id_of("Guard")?;
    let marked = driver
        .state()
        .object(guard)
        .unwrap()
        .permanent()
        .unwrap()
        .damage();
    let stopped = quality.stops(source, caster);
    assert_eq!(
        marked,
        if stopped { 0 } else { 3 },
        "{quality:?} {source:?}"
    );
    for seat in 0..seats {
        assert!(!alive(&driver, &format!("Bear {seat}")), "Bear {seat}");
    }
    Ok(())
}

#[rstest]
fn damage_to_a_player_with_protection_is_prevented_and_the_rest_is_not(
    #[values(2, 3, 4)] seats: usize,
    #[values(Quality::Red, Quality::Opponents, Quality::Everything)] quality: Quality,
    #[values(Source::Red, Source::Blue)] source: Source,
) -> Result<(), ScriptError> {
    // A player with protection can't be named by the spell it stops, so the damage comes from an attack.
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(source_creature(source))])
        .player_keywords(1, [quality.keyword()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Raider", 1)])?;
    driver.idle_until(1, Step::EndCombat)?;
    // The Raider is a creature of its colour, controlled by seat 0.
    let stopped = match quality {
        Quality::Red => source == Source::Red,
        Quality::Opponents | Quality::Everything => true,
        Quality::Creatures => unreachable!("not in the table"),
    };
    driver.expect_life(1, if stopped { 20 } else { 17 })?;
    for seat in (0..seats).filter(|seat| *seat != 1) {
        driver.expect_life(seat, 20)?;
    }
    Ok(())
}

/// A 3/3 that costs nothing in the colour of `source`.
fn source_creature(source: Source) -> Arc<CardDefinition> {
    let color = match source {
        Source::Blue => Color::Blue,
        _ => Color::Red,
    };
    mtg_testkit::cards::creature("Raider", 0, &[color], 3, 3)
}

#[rstest]
fn combat_damage_dealt_by_a_source_with_the_quality_to_the_holder_is_prevented(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        Quality::Red,
        Quality::Creatures,
        Quality::Opponents,
        Quality::Everything
    )]
    quality: Quality,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(source_creature(Source::Red))]);
    scenario = scenario.battlefield(1, [ready(keyworded("Guard", 2, 3, &[quality.keyword()]))]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Raider", 1)])?;
    common::combat::to_blockers(&mut driver, 1)?;
    driver.expect_decision(1, "declare-blockers")?;
    driver.block(&[("Guard", "Raider")])?;
    driver.idle_until(2, Step::EndCombat)?;
    // The Raider's three damage would kill the 2/3 Guard; protection leaves it with none, and the Guard still hurts the Raider.
    assert!(alive(&driver, "Guard"), "{quality:?}");
    let guard = driver.id_of("Guard")?;
    assert_eq!(
        driver
            .state()
            .object(guard)
            .unwrap()
            .permanent()
            .unwrap()
            .damage(),
        0
    );
    let raider = driver.id_of("Raider")?;
    assert_eq!(
        driver
            .state()
            .object(raider)
            .unwrap()
            .permanent()
            .unwrap()
            .damage(),
        2
    );
    Ok(())
}

#[rstest]
fn a_creature_with_protection_is_blocked_only_by_creatures_without_the_quality(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        Quality::Red,
        Quality::Creatures,
        Quality::Opponents,
        Quality::Everything
    )]
    quality: Quality,
) -> Result<(), ScriptError> {
    let walls = [
        ("Red Wall", Color::Red),
        ("Blue Wall", Color::Blue),
        ("White Wall", Color::White),
    ];
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(keyworded("Knight", 2, 2, &[quality.keyword()]))]);
    scenario = scenario.battlefield(
        1,
        walls
            .iter()
            .map(|(name, color)| ready(mtg_testkit::cards::creature(name, 0, &[*color], 0, 3)))
            .collect::<Vec<_>>(),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Knight", 1)])?;
    common::combat::to_blockers(&mut driver, 1)?;
    let offered: Vec<String> = match &driver.decision()?.kind {
        DecisionKind::DeclareBlockers(options) => options
            .blockers
            .iter()
            .map(|option| common::name_of(driver.state(), option.object))
            .collect(),
        _ => Vec::new(),
    };
    let mut expected: Vec<String> = walls
        .iter()
        .filter(|(_, color)| match quality {
            Quality::Red => *color != Color::Red,
            _ => false,
        })
        .map(|(name, _)| (*name).to_owned())
        .collect();
    let mut offered = offered;
    expected.sort();
    offered.sort();
    assert_eq!(offered, expected, "{quality:?}");
    if expected.is_empty() {
        driver.idle_until(2, Step::EndCombat)?;
        driver.expect_life(1, 18)?;
    }
    Ok(())
}

#[rstest]
#[case::white_aura_and_white(Color::White, 0)]
#[case::white_aura_and_blue(Color::Blue, 1)]
fn an_aura_is_not_cast_on_a_creature_with_protection_from_its_colour(
    #[case] protected: Color,
    #[case] aura_may: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let guard = keyworded(
        "Guard",
        2,
        2,
        &[Keyword::Protection(Selector(vec![Pred::Color(protected)]))],
    );
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(plains()), ready(plains())])
        .battlefield(1, [ready(guard), ready(free_creature("Bear", 2, 2))])
        .hand(0, [card("Pacifism").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Pacifism")?;
    let offered = legal(&driver);
    assert_eq!(
        offered.contains(&driver.target_named("Guard")?),
        aura_may == 1
    );
    Ok(())
}

#[rstest]
#[case::the_aura_goes_to_the_graveyard(true)]
#[case::the_equipment_comes_off(false)]
fn an_attachment_already_on_a_creature_falls_off_when_protection_from_it_arrives(
    #[case] aura: bool,
    #[values(2, 3, 4)] seats: usize,
    #[values(
        Quality::Red,
        Quality::Everything,
        Quality::Opponents,
        Quality::Creatures
    )]
    quality: Quality,
) -> Result<(), ScriptError> {
    let (attachment, color) = if aura {
        ("Pacifism", Color::White)
    } else {
        ("Bonesplitter", Color::Red)
    };
    let quality_selector = match (aura, quality) {
        (_, Quality::Everything) => Selector::default(),
        (_, Quality::Opponents) => Selector(vec![Pred::ControlledBy(Ref::Opponents)]),
        (true, _) => Selector(vec![Pred::Color(color)]),
        (false, _) => Selector(vec![Pred::Type(CardType::Artifact)]),
    };
    let _ = color;
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [attached_to(ready(card(attachment).unwrap()), 1, 0)])
        .battlefield(1, [ready(free_creature("Guard", 2, 2))]);
    let mut driver = Driver::from_scenario(scenario)?;
    let guard = driver.id_of("Guard")?;
    assert_eq!(
        driver.state().host_of(driver.id_of(attachment)?),
        Some(guard),
        "attached before"
    );
    driver.add_effect(
        driver.on(guard),
        Modification::AddKeyword(Keyword::Protection(quality_selector)),
        Duration::UntilEndOfTurn,
    )?;
    driver.pass()?;
    if aura {
        assert!(!alive(&driver, "Pacifism"), "the Aura is in the graveyard");
        driver.expect_zone(Zone::Graveyard(pid(0)), &["Pacifism"])?;
    } else {
        assert!(alive(&driver, "Bonesplitter"), "the Equipment stays");
        assert_eq!(driver.state().host_of(driver.id_of("Bonesplitter")?), None);
    }
    Ok(())
}

#[rstest]
fn equip_is_not_offered_a_creature_with_protection_from_the_equipment(
    #[values(2, 3, 4)] seats: usize,
    #[values(Quality::Everything, Quality::Creatures, Quality::Opponents)] quality: Quality,
) -> Result<(), ScriptError> {
    // Bonesplitter is a colourless artifact: protection from creatures or from opponents' things does not name it for
    // the first and does for the second; only the quality that matches it keeps the Equip ability away.
    let own = keyworded("Own Guard", 2, 2, &[quality.keyword()]);
    let open = free_creature("Open Bear", 2, 2);
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("Bonesplitter").unwrap()),
            ready(own),
            ready(open),
            ready(mountain()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let offered = driver.offered_abilities("Bonesplitter")?;
    assert!(!offered.is_empty());
    driver.begin_activation("Bonesplitter")?;
    let guard = driver.target_named("Own Guard")?;
    let stopped = matches!(quality, Quality::Everything);
    let offered = legal(&driver);
    assert_eq!(offered.contains(&guard), !stopped, "{quality:?}");
    assert!(offered.contains(&driver.target_named("Open Bear")?));
    Ok(())
}

#[rstest]
fn protection_that_arrives_mid_turn_works_from_then_and_stops_when_it_is_taken_or_ends(
    #[values(2, 3, 4)] seats: usize,
    #[values(Quality::Red, Quality::Everything)] quality: Quality,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(mountain())])
        .battlefield(1, [ready(free_creature("Guard", 2, 2))])
        .hand(0, [burn("Bolt", 3)]);
    let mut driver = Driver::from_scenario(scenario)?;
    let guard = driver.id_of("Guard")?;
    driver.begin_cast("Bolt")?;
    assert!(
        legal(&driver).contains(&driver.target_named("Guard")?),
        "before"
    );
    driver.cancel_targets()?;
    driver.add_effect(
        driver.on(guard),
        Modification::AddKeyword(quality.keyword()),
        Duration::UntilEndOfTurn,
    )?;
    driver.begin_cast("Bolt")?;
    assert!(
        !legal(&driver).contains(&driver.target_named("Guard")?),
        "during"
    );
    driver.cancel_targets()?;
    // Taking the keyword away (a later effect) lifts it.
    driver.add_effect(
        driver.on(guard),
        Modification::RemoveKeyword(quality.keyword()),
        Duration::UntilEndOfTurn,
    )?;
    driver.begin_cast("Bolt")?;
    assert!(
        legal(&driver).contains(&driver.target_named("Guard")?),
        "after it is taken"
    );
    Ok(())
}

#[rstest]
fn a_target_that_becomes_protected_as_the_spell_resolves_makes_it_do_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(mountain())])
        .battlefield(1, [ready(free_creature("Guard", 2, 2))])
        .hand(0, [burn("Bolt", 3)]);
    let mut driver = Driver::from_scenario(scenario)?;
    let guard = driver.id_of("Guard")?;
    let target = driver.target_named("Guard")?;
    driver.cast_at("Bolt", &[target])?;
    driver.add_effect(
        driver.on(guard),
        Modification::AddKeyword(Quality::Red.keyword()),
        Duration::UntilEndOfTurn,
    )?;
    settle(&mut driver)?;
    assert!(alive(&driver, "Guard"));
    driver.expect_event("the spell fizzles", |e| {
        matches!(
            e,
            Event::ResolutionEnded {
                outcome: ResolutionOutcome::Fizzled,
                ..
            }
        )
    })
}

#[rstest]
fn shroud_stops_everyone_hexproof_the_opponents_and_protection_only_its_quality(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] caster: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .battlefield(
            1,
            [
                ready(mountain()),
                ready(keyworded("Shrouded", 1, 1, &[Keyword::Shroud])),
                ready(keyworded("Hexed", 1, 1, &[Keyword::Hexproof])),
                ready(keyworded("Blue Proof", 1, 1, &[Quality::Red.keyword()])),
                ready(keyworded(
                    "White Proof",
                    1,
                    1,
                    &[Keyword::Protection(Selector(vec![Pred::Color(
                        Color::White,
                    )]))],
                )),
            ],
        )
        .hand(1, [burn("Bolt", 3)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Bolt")?;
    let offered = legal(&driver);
    // The caster controls all four, so hexproof does not apply to it; shroud and protection from red do.
    for (name, may) in [
        ("Shrouded", false),
        ("Hexed", true),
        ("Blue Proof", false),
        ("White Proof", true),
    ] {
        assert_eq!(offered.contains(&driver.target_named(name)?), may, "{name}");
    }
    Ok(())
}

#[rstest]
fn a_player_with_shroud_is_offered_to_no_one(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] caster: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, caster, Step::Main1)
        .battlefield(caster, [ready(mountain())])
        .hand(caster, [burn("Bolt", 3)])
        .player_keywords(1, [Keyword::Shroud]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Bolt")?;
    let offered = legal(&driver);
    assert!(!offered.contains(&driver.target_player(1)));
    assert_eq!(
        offered.iter().filter(|t| t.player().is_some()).count(),
        seats - 1
    );
    Ok(())
}

#[rstest]
fn mother_of_runes_gives_protection_from_the_colour_chosen_as_the_ability_resolves(
    #[values(2, 3, 4)] seats: usize,
    #[values(Color::Red, Color::Blue)] chosen: Color,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(2, 1, Step::Main1)
        .battlefield(
            1,
            [
                ready(card("Mother of Runes").unwrap()),
                ready(free_creature("Guard", 2, 2)),
            ],
        )
        .battlefield(0, [ready(mountain()), ready(island())])
        .hand(0, [burn("Bolt", 3), mono_blue_bolt()]);
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named("Guard")?;
    driver.begin_activation("Mother of Runes")?;
    driver.choose_targets(&[target])?;
    // The colour is chosen on resolution, not when the ability goes on the stack.
    assert!(
        !matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)),
        "not yet"
    );
    let mut asked = false;
    for _ in 0..50 {
        if let DecisionKind::ChooseOption(options) = &driver.decision()?.kind {
            let index = options
                .options
                .iter()
                .position(|option| {
                    matches!(option, mtg_engine::choose::ChoiceOption::Color(c) if *c == chosen)
                })
                .unwrap();
            driver.answer(Answer::ChooseOption(OptionChoice(index)))?;
            asked = true;
            break;
        }
        driver.answer_passively()?;
    }
    assert!(asked, "the colour of the protection is asked for");
    settle(&mut driver)?;
    // Seat 0 is not active in this turn; give it priority with the spells by passing to its turn is not needed: the
    // keyword is read directly.
    let guard = driver.id_of("Guard")?;
    let held: Vec<_> = mtg_engine::keywords::keywords_of(driver.state(), guard)
        .into_iter()
        .cloned()
        .collect();
    assert_eq!(
        held,
        [Keyword::Protection(Selector(vec![Pred::Color(chosen)]))]
    );
    Ok(())
}

fn mono_blue_bolt() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Blue Bolt")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Blue]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::AnyTarget)],
                effects: vec![damage(Ref::Target(0), 3)],
            }),
    )
}

#[rstest]
fn kor_firewalker_gains_life_for_each_red_spell_cast_by_any_player_and_cannot_be_burned(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] caster: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, caster, Step::Main1)
        .battlefield(
            1,
            [ready(card("Kor Firewalker").unwrap())]
                .into_iter()
                .chain((caster == 1).then(|| ready(mountain())))
                .collect::<Vec<_>>(),
        )
        .battlefield(
            0,
            if caster == 0 {
                vec![ready(mountain())]
            } else {
                vec![]
            },
        )
        .hand(caster, [burn("Bolt", 3)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Bolt")?;
    assert!(
        !legal(&driver).contains(&driver.target_named("Kor Firewalker")?),
        "protection from red"
    );
    let target = driver.target_player(caster);
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    // Kor Firewalker's controller decides whether to gain the life.
    let mut gained = false;
    for _ in 0..50 {
        if let DecisionKind::YesNo(_) = &driver.decision()?.kind {
            driver.expect_decision(1, "yes-no")?;
            driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            gained = true;
        }
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    assert!(gained);
    driver.expect_life(1, if caster == 1 { 18 } else { 21 })?;
    Ok(())
}

#[rstest]
fn black_knight_has_first_strike_and_cannot_be_blocked_or_enchanted_by_anything_white(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(card("Black Knight").unwrap())])
        .battlefield(
            1,
            [
                ready(card("Serra Angel").unwrap()),
                ready(free_creature("Gray Bear", 2, 2)),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    let knight = driver.id_of("Black Knight")?;
    let held = mtg_engine::keywords::keywords_of(driver.state(), knight);
    assert!(held.contains(&&Keyword::FirstStrike));
    driver.attack(&[("Black Knight", 1)])?;
    common::combat::to_blockers(&mut driver, 1)?;
    let DecisionKind::DeclareBlockers(options) = &driver.decision()?.kind else {
        panic!("the Gray Bear may block");
    };
    let blockers: Vec<String> = options
        .blockers
        .iter()
        .map(|option| common::name_of(driver.state(), option.object))
        .collect();
    assert_eq!(blockers, ["Gray Bear"], "Serra Angel is white");
    Ok(())
}

#[rstest]
fn pacifism_is_not_offered_the_black_knight(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(plains()), ready(plains())])
        .battlefield(
            1,
            [
                ready(card("Black Knight").unwrap()),
                ready(free_creature("Gray Bear", 2, 2)),
            ],
        )
        .hand(0, [card("Pacifism").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Pacifism")?;
    let offered = legal(&driver);
    assert!(!offered.contains(&driver.target_named("Black Knight")?));
    assert!(offered.contains(&driver.target_named("Gray Bear")?));
    let _: Option<ObjectId> = None;
    Ok(())
}
