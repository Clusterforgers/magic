//! Goal: whole bot games with decks of Izzet Charm, Rhystic Study, spells that ask a player to pay and abilities that cost mana finish and keep every invariant (zones, life, mana, the journal, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, and the games really use the payments: players are asked whether to pay and pay, with one source or several, on a turn that is not theirs, and activate abilities by paying their mana; a player who cannot pay is asked all the same and a Yes from them is refused without a change (the bots say No); an engine rebuilt from the log at each decision asks the same question, payments included, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{Choice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event};
use mtg_testkit::{RandomBot, pay_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30)
}

#[derive(Default)]
struct Seen {
    /// A yes followed at once by a yes of the same player: a payment an effect asked for was made.
    payments: u64,
    /// Payments made with two sources or more.
    with_several_sources: u64,
    /// Payments made on another seat's turn.
    out_of_turn: u64,
    declined_or_unable: u64,
    activations: u64,
}

impl Seen {
    fn add(&mut self, events: &[Event]) {
        let mut active = 0usize;
        for (at, event) in events.iter().enumerate() {
            match event {
                Event::TurnBegan { active: player, .. } => active = player.index(),
                Event::ChoiceMade {
                    player,
                    choice: Choice::YesNo(true),
                } if matches!(
                    events.get(at + 1),
                    Some(Event::ChoiceMade { player: again, choice: Choice::YesNo(true) }) if again == player
                ) =>
                {
                    self.payments += 1;
                    self.out_of_turn += u64::from(player.index() != active);
                    let taps = events[at + 2..]
                        .iter()
                        .take_while(|e| {
                            matches!(e, Event::PermanentTapped { .. } | Event::ManaAdded { .. })
                        })
                        .filter(|e| matches!(e, Event::PermanentTapped { .. }))
                        .count();
                    self.with_several_sources += u64::from(taps > 1);
                }
                Event::ChoiceMade {
                    choice: Choice::YesNo(false),
                    ..
                } => self.declined_or_unable += 1,
                Event::AbilityActivated { .. } => self.activations += 1,
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_payment_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = pay_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        seen.add(&outcome.events);
    }
    let campaign = format!("{} games of {players} seats", games());
    assert!(seen.payments > 0, "nobody paid for an effect in {campaign}");
    assert!(
        seen.with_several_sources > 0,
        "no payment used several sources in {campaign}"
    );
    assert!(
        seen.out_of_turn > 0,
        "no seat paid on another seat's turn in {campaign}"
    );
    assert!(seen.declined_or_unable > 0, "nobody declined in {campaign}");
    assert!(
        seen.activations > 0,
        "no ability that costs mana was activated in {campaign}"
    );
}

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_including_every_payment(
    #[values(2, 3, 4)] players: usize,
) -> Result<(), EngineError> {
    let (mut questions, mut payments, mut unable) = (0, 0, 0);
    for seed in 0..8 {
        let setup = pay_fuzz_setup(players, seed).unwrap();
        let mut engine = Engine::new(setup.clone())?;
        let mut bots: Vec<RandomBot> = (0..players as u64)
            .map(|seat| RandomBot::new(seed, seat))
            .collect();
        for step in 0..1500 {
            let Some(decision) = engine.decision().cloned() else {
                break;
            };
            let resumed = Engine::resume(setup.clone(), engine.events().to_vec())?;
            assert_eq!(
                resumed.state_hash(),
                engine.state_hash(),
                "seed {seed} step {step}"
            );
            // A cast or activation that is not paid for yet, and a land played that waits for the life its replacement asks
            // for, live outside the state; what an effect asks does not.
            let resolving = engine.state().resolving().is_some();
            let outside = engine.replacing()
                || !resolving
                    && matches!(
                        &decision.kind,
                        DecisionKind::PayMana(_)
                            | DecisionKind::ChooseTargets(_)
                            | DecisionKind::ChooseMode(_)
                    );
            if let DecisionKind::YesNo(options) = &decision.kind {
                questions += u64::from(options.text.starts_with("Pay "));
                if options.only_no {
                    // A payment that cannot be made is asked all the same; a Yes is refused and changes nothing.
                    assert!(options.text.starts_with("Pay "), "seed {seed} step {step}");
                    unable += 1;
                    let (log, hash) = (engine.events().len(), engine.state_hash());
                    let refused = engine.apply(Answer::YesNo(YesNoChoice(true)));
                    assert!(
                        matches!(
                            refused,
                            Err(EngineError::Illegal(IllegalAnswer::NotOffered("yes")))
                        ),
                        "seed {seed} step {step}: {refused:?}"
                    );
                    assert_eq!(engine.events().len(), log, "seed {seed} step {step}");
                    assert_eq!(engine.state_hash(), hash, "seed {seed} step {step}");
                    assert_eq!(engine.decision(), Some(&decision));
                }
            }
            if let DecisionKind::PayMana(options) = &decision.kind
                && resolving
            {
                payments += 1;
                assert!(!options.can_cancel, "seed {seed} step {step}");
                assert!(
                    engine.state().zone(Zone::Stack).contains(&options.spell)
                        || engine
                            .state()
                            .zone(Zone::Battlefield)
                            .contains(&options.spell),
                    "the payment is for the object that asks"
                );
            }
            if !outside {
                assert_eq!(
                    resumed.decision(),
                    engine.decision(),
                    "seed {seed} step {step}"
                );
            }
            let answer = bots[decision.player.index()].choose(&decision);
            engine.apply(answer)?;
        }
    }
    assert!(questions > 0, "no payment was offered");
    assert!(unable > 0, "nobody was asked to pay what they cannot");
    assert!(payments > 0, "no payment was asked for");
    Ok(())
}

#[test]
fn a_seeded_payment_game_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(pay_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        let second = play_game(pay_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
