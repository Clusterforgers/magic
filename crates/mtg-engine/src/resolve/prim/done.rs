//! "If you do" after an effect that is not a "may": whether the effect took something away.

use mtg_cards::effect::Effect;

use super::may::performed_name;
use crate::choose::Target;
use crate::resolve::{Cx, Next, Run};

/// How an effect tells a following "if you do" (`Reflexive`, `If` of `Performed`) whether it happened.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Reports {
    /// It did something when it proposed an event and nothing when it did not.
    Events,
    /// It did something when its cost was paid.
    Paid,
    /// It binds the answer itself (`may`, `cast_from_zone`).
    Itself,
    /// It is structure or bookkeeping with no action of its own to carry out; what follows reads the steps inside it.
    Nothing,
}

/// The way each effect reports. The match names every variant, so a new effect does not build until it is placed
/// here: an effect counts as done without acting only by being listed under `Nothing` on purpose.
#[allow(clippy::too_many_lines)] // one arm for every effect
fn reports(effect: &Effect) -> Reports {
    match effect {
        Effect::Draw { .. }
        | Effect::Discard { .. }
        | Effect::Mill { .. }
        | Effect::Scry { .. }
        | Effect::Reveal { .. }
        | Effect::LookAtHand { .. }
        | Effect::Search { .. }
        | Effect::Shuffle { .. }
        | Effect::Move { .. }
        | Effect::Destroy { .. }
        | Effect::Exile { .. }
        | Effect::ExileHidden { .. }
        | Effect::Manifest { .. }
        | Effect::Cloak { .. }
        | Effect::Sacrifice { .. }
        | Effect::SacrificeUpTo { .. }
        | Effect::Counter { .. }
        | Effect::DealDamage { .. }
        | Effect::GainLife { .. }
        | Effect::LoseLife { .. }
        | Effect::AddMana { .. }
        | Effect::PutCounters { .. }
        | Effect::RemoveCounters { .. }
        | Effect::RemoveAllCounters { .. }
        | Effect::MoveCounters { .. }
        | Effect::MultiplyCounters { .. }
        | Effect::Tap { .. }
        | Effect::Untap { .. }
        | Effect::TapOrUntap { .. }
        | Effect::PhaseOut { .. }
        | Effect::Attach { .. }
        | Effect::Unattach { .. }
        | Effect::JoinAttack { .. }
        | Effect::MyriadCopies { .. }
        | Effect::Transform { .. }
        | Effect::Flip { .. }
        | Effect::TurnFaceUp { .. }
        | Effect::Prepare { .. }
        | Effect::Surveil { .. }
        | Effect::Proliferate { .. }
        | Effect::Goad { .. }
        | Effect::Fight { .. }
        | Effect::BecomeMonarch { .. }
        | Effect::RingTempts { .. }
        | Effect::CreateEmblem { .. }
        | Effect::BecomeDayNight { .. }
        | Effect::Amass { .. }
        | Effect::Incubate { .. }
        | Effect::Conjure { .. }
        | Effect::Seek { .. }
        | Effect::Venture { .. }
        | Effect::UnlockDoor { .. }
        | Effect::TakeInitiative { .. }
        | Effect::Monstrosity { .. }
        | Effect::Renown { .. }
        | Effect::Exploit { .. }
        | Effect::Pair { .. }
        | Effect::BecomeLevel { .. }
        | Effect::Adapt { .. }
        | Effect::Bolster { .. }
        | Effect::Fabricate { .. }
        | Effect::Learn { .. }
        | Effect::Crewed { .. }
        | Effect::Explore { .. }
        | Effect::Connive { .. }
        | Effect::ExileUntil { .. }
        | Effect::RevealUntil { .. }
        | Effect::ExileTop { .. }
        | Effect::TimeTravel { .. }
        | Effect::ExtraTurn { .. }
        | Effect::AddPhase { .. }
        | Effect::WinGame { .. }
        | Effect::ChangeTargets { .. }
        | Effect::CopySpell { .. }
        | Effect::CopyAbility { .. }
        | Effect::CopyCard { .. }
        | Effect::BecomeCopy { .. }
        | Effect::CreateToken { .. }
        | Effect::Continuous { .. }
        | Effect::ExchangeControl { .. }
        | Effect::Rule { .. }
        | Effect::Regenerate { .. }
        | Effect::RemoveFromCombat { .. }
        | Effect::Shield { .. }
        | Effect::EndTurn
        | Effect::LoseGame { .. }
        | Effect::DrawGame {}
        | Effect::SetLife { .. }
        | Effect::ExchangeLife { .. }
        | Effect::RemoveDamage { .. }
        | Effect::Planechase { .. } => Reports::Events,
        Effect::Pay { .. } => Reports::Paid,
        Effect::May { .. } | Effect::CastFromZone { .. } => Reports::Itself,
        #[rustfmt::skip]
        Effect::LookAtTop { .. } | Effect::Choose { .. } | Effect::ChooseAtRandom { .. } | Effect::Encode { .. }
        | Effect::Remember { .. } | Effect::Forget { .. } | Effect::RememberValue { .. } | Effect::DelayedTrigger { .. } | Effect::Reflexive { .. }
        | Effect::If { .. } | Effect::Modal { .. } | Effect::RepeatEach { .. } | Effect::Together { .. } | Effect::RollDice { .. }
        | Effect::FlipCoin { .. } | Effect::Clash { .. } | Effect::Vote { .. } | Effect::VillainousChoice { .. }
        | Effect::ChooseValue { .. } | Effect::Let { .. } | Effect::Bind { .. } | Effect::RepeatWhile { .. } | Effect::SeparatePiles { .. }
        | Effect::Either { .. } | Effect::OneOf { .. } => Reports::Nothing,
    }
}

/// An effect that takes an action binds its controller for a following "if you do" when it did something and nothing
/// when it did not (`reports`); the effects that bind themselves, the structural ones, an effect that asks first and a
/// step that runs again pass through unchanged.
pub(super) fn report(cx: &Cx<'_>, effect: &Effect, mut run: Run) -> Run {
    let kind = reports(effect);
    if matches!(kind, Reports::Itself | Reports::Nothing) {
        return run;
    }
    if let Run::Done {
        events,
        bound,
        next,
        ..
    } = &mut run
    {
        if matches!(next, Next::Again) {
            return run;
        }
        let did = match kind {
            Reports::Paid => matches!(next, Next::Enter(0)),
            _ => !events.is_empty(),
        };
        let who = if did {
            vec![Target::Player(cx.scope.controller)]
        } else {
            Vec::new()
        };
        bound.push((performed_name(cx.resolving.position()), who));
    }
    run
}

#[cfg(test)]
mod tests {
    //! Goal: a sacrifice or a removal of counters binds its controller under the name an "if you do" reads when it took something and nothing when it did not, and a structural effect binds no such name.
    use mtg_cards::value::CounterKind;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Resolving, Scope, prim};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    /// Seat 0 controls the Source with two charge counters; the table has `seats` seats.
    fn game(seats: usize) -> GameState {
        let mut battlefield = vec![vec![creature("Source", 1, 1)]];
        battlefield.extend((1..seats).map(|_| Vec::new()));
        let mut game = setup(vec![Vec::new(); seats], battlefield);
        game.players[0].battlefield[0].counters = vec![(CounterKind::Charge, 2)];
        GameState::from_setup(&game).unwrap()
    }

    /// What the effect bound under the name of position 0, or `None` when it bound nothing of the kind or asked.
    fn performed(game: &GameState, json: &str) -> Option<bool> {
        let effect: Effect = serde_json::from_str(json).unwrap();
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        let Run::Done { bound, .. } = prim::run(&cx, &effect).unwrap() else {
            return None;
        };
        bound
            .iter()
            .find(|(name, _)| *name == performed_name(&[0]))
            .map(|(_, who)| !who.is_empty())
    }

    #[rstest]
    #[case::counters_taken(r#"{"RemoveCounters":{"from":"This","counter":"charge"}}"#, Some(true))]
    #[case::all_counters_taken(
        r#"{"RemoveCounters":{"from":"This","counter":"charge","count":5}}"#,
        Some(true)
    )]
    #[case::no_counter_of_the_kind(
        r#"{"RemoveCounters":{"from":"This","counter":"+1/+1"}}"#,
        Some(false)
    )]
    #[case::a_count_of_nothing(
        r#"{"RemoveCounters":{"from":"This","counter":"charge","count":0}}"#,
        Some(false)
    )]
    #[case::nothing_to_sacrifice(r#"{"Sacrifice":{"filter":[{"Type":"Land"}]}}"#, Some(false))]
    #[case::the_card_sacrificed_itself(r#"{"Sacrifice":{"filter":["This"]}}"#, Some(true))]
    #[case::life_gained(r#"{"GainLife":{"amount":1}}"#, Some(true))]
    #[case::nothing_to_discard(r#"{"Discard":{}}"#, Some(false))]
    #[case::a_structural_effect_binds_nothing(r#"{"If":{"condition":"YourTurn","then":[]}}"#, None)]
    fn an_effect_that_takes_something_reports_whether_it_did(
        #[case] json: &str,
        #[case] expected: Option<bool>,
        #[values(2, 3, 4)] seats: usize,
    ) {
        assert_eq!(performed(&game(seats), json), expected);
    }
}
