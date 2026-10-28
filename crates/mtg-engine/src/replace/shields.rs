//! Goal: prevention and regeneration shields act through the replacement step as the rules say: a shield takes up to its amount of the damage of the events in a batch, in the order the affected player picks when the events differ and share what is left (and with no question when it has enough for all of them or the events are one event), and is used up or remembers what is left; a shield covers the recipients, the sources and the kind of damage it was made for and nothing else; each shield applies to an event once, so several shields apply one after another in the order the affected player picks, and a question is asked only when the order can matter; damage that can't be prevented passes every shield and leaves it unused; lifelink follows the damage that is left; a regeneration shield replaces the destruction of its permanent (a destroy effect, lethal damage, deathtouch) and nothing else (not a burial, zero toughness or a sacrifice), before any other effect applies to the move, and is used by one destruction; at tables of two, three and four seats.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::Duration;
use mtg_core::card::CardType;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};
use rstest::rstest;

use super::{Processed, process};
use crate::choose::{Choice, ChoiceOption, Target};
use crate::combat::DamageTarget;
use crate::decision::DecisionKind;
use crate::events::{DeathCause, Event, ZoneCause};
use crate::fixtures::{creature, state};
use crate::fold::apply_event;
use crate::rules::{DamageShield, Rule, RuleEffect};
use crate::state::GameState;
use crate::turn::Step;
use crate::zones::Zone;

fn seat(n: usize) -> PlayerId {
    PlayerId(u64::try_from(n).unwrap())
}

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// A creature with these abilities.
fn with(name: &str, abilities: &[Ability]) -> Arc<CardDefinition> {
    let card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_power_toughness(2, 2);
    Arc::new(
        abilities
            .iter()
            .cloned()
            .fold(card, CardDefinition::with_rules),
    )
}

/// Every seat controls a Bear of its number; seat 1 also controls an Imp, an Elf and these cards.
fn game(seats: usize, extra: Vec<Arc<CardDefinition>>) -> GameState {
    let hands = vec![Vec::new(); seats];
    let mut fields: Vec<Vec<Arc<CardDefinition>>> = (0..seats)
        .map(|n| vec![creature(&format!("Bear{n}"), 2, 2)])
        .collect();
    fields[1].extend([creature("Imp", 2, 2), creature("Elf", 2, 2)]);
    fields[1].extend(extra);
    state(hands, fields)
}

fn id(game: &GameState, name: &str) -> ObjectId {
    game.objects()
        .find(|o| o.card().name == name)
        .map(crate::object::GameObject::id)
        .unwrap()
}

fn target(game: &GameState, name: &str) -> Target {
    match name.strip_prefix("player") {
        Some(n) => Target::Player(seat(n.parse().unwrap())),
        None => Target::Object(game.object(id(game, name)).unwrap().reference()),
    }
}

fn damage_target(game: &GameState, name: &str) -> DamageTarget {
    match target(game, name) {
        Target::Player(p) => DamageTarget::Player(p),
        Target::Object(o) => DamageTarget::Object(o.id),
    }
}

/// The damage `source` deals to `to` (a card's name, or `player` and a seat).
fn hit(game: &GameState, source: &str, to: &str, amount: u32) -> Event {
    Event::DamageDealt {
        source: id(game, source),
        target: damage_target(game, to),
        amount,
        deathtouch: false,
        form: crate::combat::DamageForm::Normal,
        toxic: 0,
    }
}

/// Puts a rule in force as a spell of `source`'s controller made it, until end of turn.
fn begin(game: &mut GameState, source: &str, rule: Rule) -> EffectId {
    let shield = EffectId(game.next_effect);
    let object = game.object(id(game, source)).unwrap();
    let effect = RuleEffect {
        id: shield,
        rule,
        duration: Duration::UntilEndOfTurn,
        source: Some(object.reference()),
        controller: object.controller(),
        turn: 1,
    };
    apply_event(game, &Event::RuleCreated { effect }).unwrap();
    shield
}

/// A prevention shield for damage to `to` (anything when empty) from the sources `from` selects.
fn shield_for(
    game: &GameState,
    to: &[&str],
    from: &str,
    combat: Option<bool>,
    left: Option<u32>,
) -> Rule {
    Rule::PreventDamage(DamageShield {
        to: (!to.is_empty()).then(|| to.iter().map(|name| target(game, name)).collect()),
        source: serde_json::from_str(from).unwrap(),
        from: None,
        combat,
        left,
        then: Vec::new(),
    })
}

fn anything(game: &GameState, left: Option<u32>) -> Rule {
    shield_for(game, &[], "[]", None, left)
}

fn events(result: Processed) -> Vec<Event> {
    match result {
        Processed::Events(events) => events,
        other @ Processed::Ask { .. } => panic!("events were expected, got {other:?}"),
    }
}

fn amounts(events: &[Event]) -> Vec<u32> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::DamageDealt { amount, .. } => Some(*amount),
            _ => None,
        })
        .collect()
}

fn run(game: &GameState, batch: &[Event]) -> Vec<Event> {
    events(process(game, batch.to_vec(), &[]).unwrap())
}

/// What a batch did to a shield.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum After {
    Untouched,
    Used(u32),
    Ended,
}

fn shield_events(events: &[Event], shield: EffectId) -> After {
    events
        .iter()
        .find_map(|event| match event {
            Event::ShieldUsed { id, left } if *id == shield => Some(After::Used(*left)),
            Event::RuleExpired { id } if *id == shield => Some(After::Ended),
            _ => None,
        })
        .unwrap_or(After::Untouched)
}

// Amounts

#[rstest]
#[case::part_of_one(Some(3), &[2], &[], After::Used(1))]
#[case::all_of_one(Some(3), &[3], &[], After::Ended)]
#[case::less_than_the_damage(Some(3), &[5], &[2], After::Ended)]
#[case::two_events_share_it(Some(3), &[2, 2], &[1], After::Ended)]
#[case::four_events_share_it(Some(3), &[1, 1, 1, 1], &[1], After::Ended)]
#[case::enough_for_both(Some(7), &[4, 2], &[], After::Used(1))]
#[case::all_damage(None, &[4, 2], &[], After::Untouched)]
fn a_shield_takes_damage_from_the_events_of_a_batch_without_a_question_when_they_are_one_event(
    #[values(2, 3, 4)] seats: usize,
    #[case] left: Option<u32>,
    #[case] hits: &[u32],
    #[case] remaining: &[u32],
    #[case] after: After,
) {
    let mut table = game(seats, Vec::new());
    let rule = anything(&table, left);
    let shield = begin(&mut table, "Imp", rule);
    let batch: Vec<Event> = hits
        .iter()
        .map(|amount| hit(&table, "Imp", "Bear0", *amount))
        .collect();
    let result = run(&table, &batch);
    assert_eq!(amounts(&result), remaining);
    assert_eq!(shield_events(&result, shield), after);
}

#[test]
fn a_shield_that_remembers_what_is_left_is_the_state_the_next_batch_starts_from() {
    let mut table = game(2, Vec::new());
    let rule = anything(&table, Some(3));
    let shield = begin(&mut table, "Imp", rule);
    let first = run(&table, &[hit(&table, "Imp", "Bear0", 2)]);
    assert_eq!(shield_events(&first, shield), After::Used(1));
    for event in &first {
        apply_event(&mut table, event).unwrap();
    }
    let second = run(&table, &[hit(&table, "Imp", "Bear0", 2)]);
    assert_eq!(amounts(&second), [1]);
    assert_eq!(shield_events(&second, shield), After::Ended);
}

// What a shield covers

#[rstest]
#[case::a_creature(&["Bear0"], "[]", None, &[("Imp", "Bear0", 2), ("Imp", "Bear1", 2), ("Imp", "player0", 2)], &[2, 2])]
#[case::a_player(&["player0"], "[]", None, &[("Imp", "Bear0", 2), ("Imp", "player0", 2), ("Imp", "player1", 2)], &[2, 2])]
#[case::two_recipients(&["Bear0", "player1"], "[]", None, &[("Imp", "Bear0", 1), ("Imp", "player1", 1), ("Imp", "player0", 1)], &[1])]
#[case::one_source(&[], r#"[{"Named":"Imp"}]"#, None, &[("Imp", "player0", 2), ("Elf", "player0", 3)], &[3])]
#[case::every_other_source(&[], r#"[{"Not":{"Named":"Imp"}}]"#, None, &[("Imp", "player0", 2), ("Elf", "player0", 3)], &[2])]
#[case::source_and_recipient(&["Bear0"], r#"[{"Named":"Imp"}]"#, None, &[("Imp", "Bear0", 2), ("Elf", "Bear0", 3), ("Imp", "Bear1", 4)], &[3, 4])]
#[case::damage_that_is_not_combat_damage(&[], "[]", Some(false), &[("Imp", "player0", 2)], &[])]
#[case::combat_damage_outside_combat(&[], "[]", Some(true), &[("Imp", "player0", 2)], &[2])]
fn a_shield_covers_the_recipients_the_sources_and_the_kind_of_damage_it_was_made_for(
    #[values(2, 3, 4)] seats: usize,
    #[case] to: &[&str],
    #[case] from: &str,
    #[case] combat: Option<bool>,
    #[case] hits: &[(&str, &str, u32)],
    #[case] remaining: &[u32],
) {
    let mut table = game(seats, Vec::new());
    let rule = shield_for(&table, to, from, combat, None);
    begin(&mut table, "Elf", rule);
    let batch: Vec<Event> = hits
        .iter()
        .map(|(source, to, amount)| hit(&table, source, to, *amount))
        .collect();
    assert_eq!(amounts(&run(&table, &batch)), remaining);
}

#[test]
fn a_shield_for_combat_damage_covers_the_damage_steps_and_not_the_rest_of_the_turn() {
    let mut table = game(2, Vec::new());
    let rule = shield_for(&table, &[], "[]", Some(true), None);
    begin(&mut table, "Elf", rule);
    let batch = [hit(&table, "Imp", "player0", 2)];
    assert_eq!(amounts(&run(&table, &batch)), [2]);
    apply_event(
        &mut table,
        &Event::StepBegan {
            step: Step::CombatDamage,
        },
    )
    .unwrap();
    assert_eq!(amounts(&run(&table, &batch)), Vec::<u32>::new());
}

#[test]
fn a_shield_for_a_creature_that_became_another_object_ends() {
    let mut table = game(2, Vec::new());
    let rule = shield_for(&table, &["Bear0"], "[]", None, None);
    begin(&mut table, "Imp", rule);
    let bear = id(&table, "Bear0");
    let batch = [hit(&table, "Imp", "Bear0", 2)];
    assert_eq!(amounts(&run(&table, &batch)), Vec::<u32>::new());
    let left = Event::ZoneChanged {
        object: bear,
        from: Zone::Battlefield,
        to: Zone::Graveyard(seat(0)),
        cause: ZoneCause::Moved,
    };
    apply_event(&mut table, &left).unwrap();
    assert!(!table.rule_effects()[0].is_active(&table));
}

// Several shields

fn two_shields(table: &mut GameState, first: Option<u32>, second: Option<u32>) -> [EffectId; 2] {
    let one = anything(table, first);
    let two = anything(table, second);
    [begin(table, "Imp", one), begin(table, "Elf", two)]
}

fn ask(table: &GameState, batch: &[Event]) -> (PlayerId, Vec<Option<EffectId>>) {
    let Processed::Ask { decision, proposed } = process(table, batch.to_vec(), &[]).unwrap() else {
        panic!("two shields apply, so the player is asked");
    };
    assert_eq!(proposed, batch);
    let DecisionKind::ChooseReplacement(options) = decision.kind else {
        panic!("an order of replacement effects was expected");
    };
    (
        decision.player,
        options.candidates.iter().map(|c| c.shield).collect(),
    )
}

#[rstest]
#[case::the_small_one_first(0)]
#[case::the_large_one_first(1)]
fn two_shields_on_one_event_ask_for_the_order_and_each_applies_once(
    #[values(2, 3, 4)] seats: usize,
    #[case] pick: usize,
) {
    let mut table = game(seats, Vec::new());
    let [small, large] = two_shields(&mut table, Some(3), Some(4));
    let batch = [hit(&table, "Imp", "Bear0", 5)];
    let (asked, candidates) = ask(&table, &batch);
    assert_eq!(
        asked,
        seat(0),
        "the controller of the creature that is hurt orders them"
    );
    assert_eq!(candidates, [Some(small), Some(large)]);
    let result = events(
        process(
            &table,
            batch.to_vec(),
            &[(seat(0), Choice::Replacement(pick))],
        )
        .unwrap(),
    );
    assert_eq!(amounts(&result), Vec::<u32>::new());
    // Against 5 damage the first takes its fill (3 or 4) and the second the rest, so each ends with 2 or 3 minus that.
    let (first, second) = if pick == 0 {
        (small, large)
    } else {
        (large, small)
    };
    assert_eq!(shield_events(&result, first), After::Ended);
    assert_eq!(shield_events(&result, second), After::Used(2));
}

#[test]
fn damage_to_a_player_asks_that_player() {
    let mut table = game(4, Vec::new());
    two_shields(&mut table, Some(3), Some(4));
    for n in 0..4 {
        let batch = [hit(&table, "Imp", &format!("player{n}"), 5)];
        assert_eq!(ask(&table, &batch).0, seat(n));
    }
}

#[test]
fn the_unlimited_shield_first_leaves_the_limited_one_unused() {
    let mut table = game(3, Vec::new());
    let [limited, all] = two_shields(&mut table, Some(3), None);
    let batch = [hit(&table, "Imp", "Bear0", 5)];
    let (_, candidates) = ask(&table, &batch);
    let pick = candidates.iter().position(|c| *c == Some(all)).unwrap();
    let result = events(
        process(
            &table,
            batch.to_vec(),
            &[(seat(0), Choice::Replacement(pick))],
        )
        .unwrap(),
    );
    assert_eq!(amounts(&result), Vec::<u32>::new());
    assert_eq!(shield_events(&result, limited), After::Untouched);
}

#[test]
fn shields_that_prevent_everything_are_not_asked_about() {
    let mut table = game(3, Vec::new());
    let [one, two] = two_shields(&mut table, None, None);
    let result = run(&table, &[hit(&table, "Imp", "Bear0", 5)]);
    assert_eq!(amounts(&result), Vec::<u32>::new());
    assert_eq!(shield_events(&result, one), After::Untouched);
    assert_eq!(shield_events(&result, two), After::Untouched);
}

#[test]
fn a_shield_that_runs_out_leaves_the_rest_of_the_damage_of_the_event() {
    let mut table = game(2, Vec::new());
    let rule = anything(&table, Some(2));
    let shield = begin(&mut table, "Imp", rule);
    let result = run(&table, &[hit(&table, "Imp", "Bear0", 5)]);
    assert_eq!(amounts(&result), [3]);
    assert_eq!(shield_events(&result, shield), After::Ended);
}

#[test]
fn an_answer_that_is_not_the_order_of_shields_is_refused() {
    let mut table = game(2, Vec::new());
    two_shields(&mut table, Some(3), Some(4));
    let batch = [hit(&table, "Imp", "Bear0", 5)];
    for answers in [
        vec![(seat(1), Choice::Replacement(0))],
        vec![(seat(0), Choice::Replacement(2))],
        vec![(seat(0), Choice::YesNo(true))],
    ] {
        assert!(process(&table, batch.to_vec(), &answers).is_err());
    }
}

// The order a limited shield takes the events of a batch in

/// A question the step asked while a batch was played.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Question {
    player: PlayerId,
    /// `option` or `replacement`.
    kind: &'static str,
    options: Vec<String>,
    source: Option<ObjectId>,
    text: String,
}

/// Plays a batch to its end, answering each question with the next of `picks`; the questions come back with the events.
fn play(table: &GameState, batch: &[Event], picks: &[usize]) -> (Vec<Event>, Vec<Question>) {
    let mut journal = Vec::new();
    let mut asked = Vec::new();
    loop {
        match process(table, batch.to_vec(), &journal).unwrap() {
            Processed::Events(events) => {
                assert_eq!(asked.len(), picks.len(), "every pick answers a question");
                return (events, asked);
            }
            Processed::Ask { decision, proposed } => {
                assert_eq!(proposed, batch);
                let pick = *picks
                    .get(asked.len())
                    .unwrap_or_else(|| panic!("a question was not expected: {decision:?}"));
                let (question, choice) = match decision.kind {
                    DecisionKind::ChooseOption(options) => (
                        Question {
                            player: decision.player,
                            kind: "option",
                            options: options
                                .options
                                .iter()
                                .map(|option| match option {
                                    ChoiceOption::Label(label) => label.clone(),
                                    other => panic!("a label was expected, got {other:?}"),
                                })
                                .collect(),
                            source: Some(options.source),
                            text: options.text,
                        },
                        Choice::Option(pick),
                    ),
                    DecisionKind::ChooseReplacement(options) => (
                        Question {
                            player: decision.player,
                            kind: "replacement",
                            options: Vec::new(),
                            source: None,
                            text: options.text,
                        },
                        Choice::Replacement(pick),
                    ),
                    other => panic!("an order was expected, got {other:?}"),
                };
                journal.push((question.player, choice));
                asked.push(question);
            }
        }
    }
}

/// The damage left in the events as `source>target:amount`.
fn dealt(game: &GameState, events: &[Event]) -> Vec<String> {
    let name = |object: ObjectId| game.object(object).unwrap().card().name.clone();
    events
        .iter()
        .filter_map(|event| match event {
            Event::DamageDealt {
                source,
                target,
                amount,
                ..
            } => {
                let to = match target {
                    DamageTarget::Player(player) => format!("player{}", player.get()),
                    DamageTarget::Object(object) => name(*object),
                };
                Some(format!("{}>{to}:{amount}", name(*source)))
            }
            _ => None,
        })
        .collect()
}

fn hits(game: &GameState, list: &[(&str, &str, u32)]) -> Vec<Event> {
    list.iter()
        .map(|(source, to, amount)| hit(game, source, to, *amount))
        .collect()
}

const IMP2: (&str, &str, u32) = ("Imp", "Bear0", 2);
const ELF2: (&str, &str, u32) = ("Elf", "Bear0", 2);

#[rstest]
#[case::identical_events_are_a_tie(3, &[IMP2, IMP2], &[], &[], &["Imp>Bear0:1"], After::Ended)]
#[case::enough_for_all(9, &[IMP2, ("Elf", "Bear0", 5)], &[], &[], &[], After::Used(2))]
#[case::exactly_enough(7, &[IMP2, ("Elf", "Bear0", 5)], &[], &[], &[], After::Ended)]
#[case::one_event_larger_than_the_shield(2, &[("Imp", "Bear0", 5)], &[], &[], &["Imp>Bear0:3"], After::Ended)]
#[case::the_first_listed_first(3, &[IMP2, ELF2], &[0], &[0], &["Elf>Bear0:1"], After::Ended)]
#[case::the_second_listed_first(3, &[IMP2, ELF2], &[1], &[0], &["Imp>Bear0:1"], After::Ended)]
#[case::the_large_one_takes_it_all(1, &[("Imp", "Bear0", 4), ("Elf", "Bear0", 1)], &[0], &[0], &["Imp>Bear0:3", "Elf>Bear0:1"], After::Ended)]
#[case::the_small_one_first_is_prevented_whole(1, &[("Imp", "Bear0", 4), ("Elf", "Bear0", 1)], &[1], &[0], &["Imp>Bear0:4"], After::Ended)]
#[case::three_events_last_then_middle(4, &[IMP2, ("Elf", "Bear0", 3), ("Imp", "Bear0", 1)], &[2, 1], &[0, 0], &["Imp>Bear0:2"], After::Ended)]
#[case::three_events_middle_then_first(4, &[IMP2, ("Elf", "Bear0", 3), ("Imp", "Bear0", 1)], &[1, 0], &[0, 0], &["Imp>Bear0:1", "Imp>Bear0:1"], After::Ended)]
#[case::three_events_in_batch_order(4, &[IMP2, ("Elf", "Bear0", 3), ("Imp", "Bear0", 1)], &[0, 0], &[0, 0], &["Elf>Bear0:1", "Imp>Bear0:1"], After::Ended)]
#[case::a_creature_and_its_controller(3, &[IMP2, ("Imp", "player0", 2)], &[1], &[0], &["Imp>Bear0:1"], After::Ended)]
#[case::each_player_picks_among_their_own_events_only(3, &[("Imp", "Bear0", 3), ("Imp", "Bear1", 3)], &[], &[], &["Imp>Bear1:3"], After::Ended)]
#[case::the_player_of_a_later_event_picks_among_theirs(3, &[IMP2, ("Imp", "Bear1", 5), ("Elf", "Bear1", 2)], &[1], &[1], &["Imp>Bear1:5", "Elf>Bear1:1"], After::Ended)]
#[case::and_the_other_pick(3, &[IMP2, ("Imp", "Bear1", 5), ("Elf", "Bear1", 2)], &[0], &[1], &["Imp>Bear1:4", "Elf>Bear1:2"], After::Ended)]
fn a_limited_shield_takes_the_events_in_the_order_the_affected_player_picks(
    #[values(2, 3, 4)] seats: usize,
    #[case] left: u32,
    #[case] batch: &[(&str, &str, u32)],
    #[case] picks: &[usize],
    #[case] asked: &[usize],
    #[case] remaining: &[&str],
    #[case] after: After,
) {
    let mut table = game(seats, Vec::new());
    let rule = anything(&table, Some(left));
    let shield = begin(&mut table, "Imp", rule);
    let (result, questions) = play(&table, &hits(&table, batch), picks);
    assert_eq!(dealt(&table, &result), remaining);
    assert_eq!(shield_events(&result, shield), after);
    let who: Vec<PlayerId> = questions.iter().map(|q| q.player).collect();
    assert_eq!(who, asked.iter().map(|n| seat(*n)).collect::<Vec<_>>());
    assert!(questions.iter().all(|q| q.kind == "option"));
}

#[rstest]
fn the_options_name_each_distinct_event_once_and_the_question_names_the_shield(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut table = game(seats, Vec::new());
    let rule = anything(&table, Some(3));
    let shield = begin(&mut table, "Imp", rule);
    let batch = hits(&table, &[IMP2, IMP2, ELF2]);
    let (result, questions) = play(&table, &batch, &[1]);
    let [question] = questions.as_slice() else {
        panic!("one question was expected, got {questions:?}");
    };
    assert_eq!(
        question.options,
        ["2 damage from Imp to Bear0", "2 damage from Elf to Bear0"]
    );
    assert_eq!(question.source, Some(id(&table, "Imp")));
    assert_eq!(question.text, "Choose the damage Imp prevents first.");
    // The Elf's event is taken whole, and the two events of the Imp are one tie for what is left.
    assert_eq!(dealt(&table, &result), ["Imp>Bear0:1", "Imp>Bear0:2"]);
    assert_eq!(shield_events(&result, shield), After::Ended);
}

#[rstest]
fn a_pick_names_the_first_of_the_events_that_are_the_same_event(#[values(2, 3, 4)] seats: usize) {
    let mut table = game(seats, Vec::new());
    let rule = anything(&table, Some(2));
    begin(&mut table, "Imp", rule);
    let batch = hits(&table, &[ELF2, IMP2, IMP2]);
    let (result, _) = play(&table, &batch, &[1]);
    assert_eq!(dealt(&table, &result), ["Elf>Bear0:2", "Imp>Bear0:2"]);
}

#[rstest]
fn two_limited_shields_ask_for_the_order_of_the_shields_and_then_for_the_damage(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut table = game(seats, Vec::new());
    let [first, second] = two_shields(&mut table, Some(2), Some(2));
    let batch = hits(&table, &[IMP2, ELF2]);
    let (result, questions) = play(&table, &batch, &[0, 1]);
    let kinds: Vec<&str> = questions.iter().map(|q| q.kind).collect();
    assert_eq!(kinds, ["replacement", "option"]);
    assert_eq!(dealt(&table, &result), Vec::<String>::new());
    assert_eq!(shield_events(&result, first), After::Ended);
    assert_eq!(shield_events(&result, second), After::Ended);
}

#[rstest]
#[case::another_player(seat(1), Choice::Option(0))]
#[case::an_option_that_is_not_offered(seat(0), Choice::Option(2))]
#[case::the_order_of_effects(seat(0), Choice::Replacement(0))]
#[case::a_yes(seat(0), Choice::YesNo(true))]
fn an_answer_that_is_not_a_pick_of_the_damage_is_refused(
    #[values(2, 3, 4)] seats: usize,
    #[case] who: PlayerId,
    #[case] choice: Choice,
) {
    let mut table = game(seats, Vec::new());
    let rule = anything(&table, Some(3));
    begin(&mut table, "Imp", rule);
    let batch = hits(&table, &[IMP2, ELF2]);
    assert!(process(&table, batch.clone(), &[(who, choice)]).is_err());
    let two = [(seat(0), Choice::Option(0)), (seat(0), Choice::Option(0))];
    assert!(
        process(&table, batch, &two).is_err(),
        "an answer to a question that is not asked"
    );
}

#[rstest]
fn the_step_is_a_pure_function_of_the_state_the_batch_and_the_answers(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut table = game(seats, Vec::new());
    let rule = anything(&table, Some(3));
    begin(&mut table, "Imp", rule);
    let batch = hits(&table, &[IMP2, ELF2]);
    let journal = [(seat(0), Choice::Option(1))];
    let once = process(&table, batch.clone(), &journal).unwrap();
    assert_eq!(once, process(&table, batch.clone(), &journal).unwrap());
    let Processed::Ask { decision, .. } = process(&table, batch, &[]).unwrap() else {
        panic!("the player is asked");
    };
    assert_eq!(decision.player, seat(0));
}

// Damage that can't be prevented

fn rule_card(rule: &str) -> Arc<CardDefinition> {
    with(
        "Hex",
        &[ability(&format!(
            r#"{{"Rule":{{"rule":{{"DamageCantBePrevented":{rule}}}}}}}"#
        ))],
    )
}

#[rstest]
#[case::every_damage("{}", &[("Imp", "player0", 2), ("Elf", "Bear0", 3)], &[2, 3])]
#[case::damage_of_one_source(r#"{"source":[{"Named":"Imp"}]}"#, &[("Imp", "player0", 2), ("Elf", "player0", 3)], &[2])]
#[case::combat_damage_outside_combat(r#"{"combat":true}"#, &[("Imp", "player0", 2)], &[])]
#[case::damage_that_is_not_combat_damage(r#"{"combat":false}"#, &[("Imp", "player0", 2)], &[2])]
fn damage_that_cant_be_prevented_passes_the_shield_and_leaves_it_unused(
    #[values(2, 3, 4)] seats: usize,
    #[case] rule: &str,
    #[case] hits: &[(&str, &str, u32)],
    #[case] remaining: &[u32],
) {
    let mut table = game(seats, vec![rule_card(rule)]);
    let shield_rule = anything(&table, Some(9));
    let shield = begin(&mut table, "Imp", shield_rule);
    let batch: Vec<Event> = hits
        .iter()
        .map(|(source, to, amount)| hit(&table, source, to, *amount))
        .collect();
    let result = run(&table, &batch);
    assert_eq!(amounts(&result), remaining);
    let used = hits.len() - remaining.len();
    assert_eq!(
        shield_events(&result, shield) == After::Untouched,
        used == 0
    );
}

#[test]
fn a_rule_of_a_spell_makes_damage_unpreventable_until_it_ends() {
    let mut table = game(2, Vec::new());
    let rule = anything(&table, None);
    begin(&mut table, "Imp", rule);
    let batch = [hit(&table, "Imp", "player0", 2)];
    assert_eq!(amounts(&run(&table, &batch)), Vec::<u32>::new());
    let unpreventable = begin(
        &mut table,
        "Elf",
        Rule::DamageCantBePrevented {
            source: mtg_cards::refs::Selector::default(),
            combat: None,
        },
    );
    assert_eq!(amounts(&run(&table, &batch)), [2]);
    apply_event(&mut table, &Event::RuleExpired { id: unpreventable }).unwrap();
    assert_eq!(amounts(&run(&table, &batch)), Vec::<u32>::new());
}

// Lifelink

#[rstest]
#[case::part_is_prevented(Some(1), &[1], Some(1))]
#[case::all_is_prevented(Some(5), &[], None)]
#[case::none_is_prevented(Some(0), &[2], Some(2))]
fn lifelink_gains_the_damage_that_is_dealt(
    #[case] left: Option<u32>,
    #[case] dealt: &[u32],
    #[case] gained: Option<u32>,
) {
    let linker = with("Linker", &[Ability::Keyword(Keyword::Lifelink)]);
    let mut table = game(2, vec![linker]);
    if left != Some(0) {
        let rule = anything(&table, left);
        begin(&mut table, "Elf", rule);
    }
    let batch = [
        hit(&table, "Linker", "Bear0", 2),
        Event::LifeGained {
            player: seat(1),
            amount: 2,
        },
    ];
    let result = run(&table, &batch);
    assert_eq!(amounts(&result), dealt);
    let life: Vec<u32> = result
        .iter()
        .filter_map(|event| match event {
            Event::LifeGained { amount, .. } => Some(*amount),
            _ => None,
        })
        .collect();
    assert_eq!(life, gained.into_iter().collect::<Vec<_>>());
}

#[rstest]
#[case::the_linkers_damage_first(1, &["Imp>Bear0:2"], None)]
#[case::the_other_damage_first(0, &["Linker>Bear0:2"], Some(2))]
fn lifelink_follows_the_event_the_player_picked(
    #[values(2, 3, 4)] seats: usize,
    #[case] pick: usize,
    #[case] remaining: &[&str],
    #[case] gained: Option<u32>,
) {
    let linker = with("Linker", &[Ability::Keyword(Keyword::Lifelink)]);
    let mut table = game(seats, vec![linker]);
    let rule = anything(&table, Some(2));
    begin(&mut table, "Elf", rule);
    let batch = [
        hit(&table, "Imp", "Bear0", 2),
        hit(&table, "Linker", "Bear0", 2),
        Event::LifeGained {
            player: seat(1),
            amount: 2,
        },
    ];
    let (result, _) = play(&table, &batch, &[pick]);
    assert_eq!(dealt(&table, &result), remaining);
    let life: Vec<u32> = result
        .iter()
        .filter_map(|event| match event {
            Event::LifeGained { amount, .. } => Some(*amount),
            _ => None,
        })
        .collect();
    assert_eq!(life, gained.into_iter().collect::<Vec<_>>());
}

// Regeneration

fn destroy(game: &GameState, name: &str, cause: ZoneCause) -> Event {
    Event::ZoneChanged {
        object: id(game, name),
        from: Zone::Battlefield,
        to: Zone::Graveyard(seat(0)),
        cause,
    }
}

fn dies(game: &GameState, name: &str, cause: DeathCause) -> Event {
    Event::CreatureDied {
        object: id(game, name),
        cause,
    }
}

fn regenerates(game: &GameState, name: &str) -> Rule {
    Rule::Regenerate(game.object(id(game, name)).unwrap().reference())
}

#[test]
fn a_destruction_is_replaced_by_regeneration_and_uses_the_shield_up() {
    let mut table = game(3, Vec::new());
    let rule = regenerates(&table, "Bear0");
    let shield = begin(&mut table, "Bear0", rule);
    let bear = id(&table, "Bear0");
    let result = run(&table, &[destroy(&table, "Bear0", ZoneCause::Destroyed)]);
    assert_eq!(
        result,
        [
            Event::Regenerated { object: bear },
            Event::RuleExpired { id: shield }
        ]
    );
}

#[rstest]
#[case::destroyed(ZoneCause::Destroyed, true)]
#[case::buried(ZoneCause::Buried, false)]
#[case::sacrificed(ZoneCause::Sacrificed, false)]
#[case::exiled(ZoneCause::Exiled, false)]
#[case::state_based(ZoneCause::StateBased, false)]
fn a_move_is_regenerated_when_it_is_a_destruction_that_can_be_regenerated(
    #[values(2, 3, 4)] seats: usize,
    #[case] cause: ZoneCause,
    #[case] replaced: bool,
) {
    let mut table = game(seats, Vec::new());
    let rule = regenerates(&table, "Bear0");
    begin(&mut table, "Bear0", rule);
    let moved = destroy(&table, "Bear0", cause);
    let result = run(&table, std::slice::from_ref(&moved));
    assert_eq!(result.first() != Some(&moved), replaced);
}

#[rstest]
#[case::lethal_damage(DeathCause::LethalDamage, true)]
#[case::deathtouch(DeathCause::Deathtouch, true)]
#[case::zero_toughness(DeathCause::ZeroToughness, false)]
fn a_death_by_state_based_actions_is_regenerated_when_it_is_a_destruction(
    #[values(2, 3, 4)] seats: usize,
    #[case] cause: DeathCause,
    #[case] replaced: bool,
) {
    let mut table = game(seats, Vec::new());
    let rule = regenerates(&table, "Bear0");
    begin(&mut table, "Bear0", rule);
    let died = dies(&table, "Bear0", cause);
    let result = run(&table, std::slice::from_ref(&died));
    assert_eq!(result.first() != Some(&died), replaced);
}

#[test]
fn one_shield_is_used_by_one_destruction_and_two_by_two() {
    let mut table = game(2, Vec::new());
    let rule = regenerates(&table, "Bear0");
    let first = begin(&mut table, "Bear0", rule.clone());
    let second = begin(&mut table, "Bear0", rule);
    let bear = id(&table, "Bear0");
    let batch = [
        destroy(&table, "Bear0", ZoneCause::Destroyed),
        dies(&table, "Bear0", DeathCause::LethalDamage),
        destroy(&table, "Bear0", ZoneCause::Destroyed),
    ];
    let result = run(&table, &batch);
    assert_eq!(
        result,
        [
            Event::Regenerated { object: bear },
            Event::Regenerated { object: bear },
            batch[2].clone(),
            Event::RuleExpired { id: first },
            Event::RuleExpired { id: second },
        ]
    );
}

#[test]
fn a_shield_of_another_permanent_replaces_nothing() {
    let mut table = game(2, Vec::new());
    let rule = regenerates(&table, "Bear1");
    begin(&mut table, "Bear1", rule);
    let batch = [destroy(&table, "Bear0", ZoneCause::Destroyed)];
    assert_eq!(run(&table, &batch), batch);
}

#[test]
fn regeneration_comes_before_the_effects_that_replace_the_move() {
    let rest_in_peace = with(
        "Peace",
        &[ability(
            r#"{"Replacement":{"when":{"ZoneChange":{"what":[],"to":"Graveyard"}},"by":{"MoveTo":"Exile"}}}"#,
        )],
    );
    let mut table = game(3, vec![rest_in_peace]);
    let rule = regenerates(&table, "Bear0");
    let shield = begin(&mut table, "Bear0", rule);
    let bear = id(&table, "Bear0");
    let moved = destroy(&table, "Bear0", ZoneCause::Destroyed);
    assert_eq!(
        run(&table, std::slice::from_ref(&moved)),
        [
            Event::Regenerated { object: bear },
            Event::RuleExpired { id: shield }
        ]
    );
    // Without the shield the move is exiled instead.
    let other = destroy(&table, "Bear2", ZoneCause::Destroyed);
    let result = run(&table, &[other]);
    assert!(matches!(
        result.as_slice(),
        [Event::ZoneChanged {
            to: Zone::Exile,
            ..
        }]
    ));
}

#[test]
fn regeneration_does_not_stop_a_destruction_of_something_it_is_not_for() {
    let mut table = game(2, Vec::new());
    let rule = regenerates(&table, "Bear0");
    begin(&mut table, "Bear0", rule);
    let batch = [
        hit(&table, "Imp", "Bear0", 2),
        Event::PriorityPassed { player: seat(0) },
    ];
    assert_eq!(run(&table, &batch), batch);
}
