//! Combat damage: dividing an attacker's damage among its blockers and the player it attacks, and dealing it
//! simultaneously in the first-strike step and the regular one.

use mtg_core::ids::{ObjectId, PlayerId};

use crate::answers::Outcome;
use crate::characteristics::{characteristics, controller_of};
use crate::combat::{AttackTarget, Attacker, DamageTarget};
use crate::decision::DamageOptions;
use crate::error::IllegalAnswer;
use crate::events::Event;
use crate::keywords::{self, DamageRules};
use crate::object::GameObject;
use crate::snapshot::Snapshot;
use crate::state::GameState;
use crate::turn::Step;
use crate::zones::Zone;

/// Which combat damage step is being dealt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DamagePass {
    /// The first-strike step (creatures with first strike or double strike).
    FirstStrike,
    /// The regular step.
    Regular,
}

/// Whether a first-strike damage step occurs this combat: an attacker or a blocker has first strike or double strike (CR 510.4).
pub(crate) fn first_strike_step_needed(state: &GameState) -> bool {
    state.combat().attackers().iter().any(|attacker| {
        std::iter::once(attacker.object())
            .chain(attacker.blockers().iter().copied())
            .any(|id| keywords::damage_rules(state, id).strikes_first())
    })
}

/// The damage steps still ahead in the combat in progress, in order.
pub(crate) fn passes_ahead(state: &GameState) -> &'static [DamagePass] {
    const BOTH: [DamagePass; 2] = [DamagePass::FirstStrike, DamagePass::Regular];
    const REGULAR: [DamagePass; 1] = [DamagePass::Regular];
    let first_strike_over = match state.turn().step() {
        Step::FirstStrikeDamage => state.priority().is_some(),
        Step::CombatDamage => true,
        _ => !first_strike_step_needed(state),
    };
    if first_strike_over { &REGULAR } else { &BOTH }
}

/// Damage `id` deals in `pass`; `None` if it is not on the battlefield or does not strike in this pass.
///
/// A creature with first strike alone strikes in the first-strike step only, one with double strike in both,
/// and every other creature in the regular step (CR 510.4).
pub(crate) fn strike_power(state: &GameState, id: ObjectId, pass: DamagePass) -> Option<u32> {
    let on_battlefield = state
        .object(id)
        .is_some_and(|o| o.zone() == Zone::Battlefield);
    if !on_battlefield {
        return None;
    }
    let rules = keywords::damage_rules(state, id);
    let strikes = match pass {
        DamagePass::FirstStrike => rules.strikes_first(),
        DamagePass::Regular => rules.strikes_regularly(),
    };
    if !strikes {
        return None;
    }
    let power = characteristics(state, id)?.power.unwrap_or(0).max(0);
    Some(u32::try_from(power).unwrap_or(0))
}

/// Damage that is lethal to `id` from a source that does or does not have deathtouch (CR 120.6, 702.2c).
///
/// It is what the toughness leaves after the damage marked, and 1 at most from deathtouch: any damage from such
/// a source is lethal, but a creature already holding lethal damage needs none.
pub(crate) fn lethal_damage(state: &GameState, id: ObjectId, deathtouch: bool) -> u32 {
    let toughness = characteristics(state, id)
        .and_then(|c| c.toughness)
        .unwrap_or(0);
    let marked = state
        .object(id)
        .and_then(GameObject::permanent)
        .map_or(0, crate::object::PermanentState::damage);
    let left = u32::try_from((i64::from(toughness) - i64::from(marked)).max(0)).unwrap_or(u32::MAX);
    if deathtouch { left.min(1) } else { left }
}

/// What an attacker's damage amounts to in one step: dealt as it stands or waiting for its controller's division.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Division {
    /// Nothing is left to choose: these shares are dealt, in order.
    Fixed(Vec<(DamageTarget, u32)>),
    /// The attacker's controller divides the damage.
    Choose(DamageOptions),
}

/// What the attacker attacks, as a recipient of damage.
fn defender_of(attacker: &Attacker) -> DamageTarget {
    match attacker.target() {
        AttackTarget::Player(p) => DamageTarget::Player(p),
        AttackTarget::Planeswalker(o) => DamageTarget::Object(o),
    }
}

/// How the attacker's `power` is shared among its blockers and the player or planeswalker it attacks.
///
/// An unblocked attacker deals all of it to what it attacks. A blocked one deals it to its blockers: all to
/// the only one, as its controller divided it among several, or as the default division has it when the
/// controller has not divided yet and has a choice to make. Trample (CR 702.19) adds what it attacks as a
/// recipient of the damage that goes beyond lethal to every blocker, and of all of it when no blocker is left.
pub(crate) fn division(
    state: &GameState,
    attacker: &Attacker,
    power: u32,
    rules: DamageRules,
) -> Division {
    let defender = defender_of(attacker);
    if !attacker.blocked() {
        return Division::Fixed(vec![(defender, power)]);
    }
    let blockers = attacker.blockers();
    if blockers.is_empty() {
        return Division::Fixed(if rules.trample {
            vec![(defender, power)]
        } else {
            Vec::new()
        });
    }
    if let Some(recorded) = &attacker.assignment {
        let mut shares: Vec<(DamageTarget, u32)> = blockers
            .iter()
            .map(|blocker| {
                let share = recorded
                    .iter()
                    .filter(|(b, _)| b == blocker)
                    .fold(0u32, |sum, (_, n)| sum.saturating_add(*n));
                (DamageTarget::Object(*blocker), share)
            })
            .collect();
        let assigned = shares
            .iter()
            .fold(0u32, |sum, (_, n)| sum.saturating_add(*n));
        if rules.trample && power > assigned {
            shares.push((defender, power - assigned));
        }
        return Division::Fixed(shares);
    }
    let options = DamageOptions {
        attacker: attacker.object(),
        power,
        blockers: blockers.to_vec(),
        excess: rules.trample.then_some(defender),
        lethal: blockers
            .iter()
            .map(|blocker| lethal_damage(state, *blocker, rules.deathtouch))
            .collect(),
    };
    if has_choice(&options) {
        Division::Choose(options)
    } else {
        Division::Fixed(default_shares(&options))
    }
}

/// Whether the attacker's controller has more than one way to divide the damage.
fn has_choice(options: &DamageOptions) -> bool {
    if options.power == 0 {
        return false;
    }
    match (options.excess.is_some(), options.blockers.as_slice()) {
        (_, []) | (false, [_]) => false,
        (true, [_]) => options
            .lethal
            .first()
            .is_some_and(|lethal| options.power > *lethal),
        _ => true,
    }
}

/// The division a controller most often wants: each blocker in order gets lethal damage while power lasts,
/// and what is left goes to the player or planeswalker attacked with trample and else onto the last blocker.
pub(crate) fn default_shares(options: &DamageOptions) -> Vec<(DamageTarget, u32)> {
    let mut left = options.power;
    let mut shares = Vec::with_capacity(options.blockers.len() + 1);
    for (at, blocker) in options.blockers.iter().enumerate() {
        let last = at + 1 == options.blockers.len();
        let lethal = options.lethal.get(at).copied().unwrap_or(0);
        let share = if last && options.excess.is_none() {
            left
        } else {
            left.min(lethal)
        };
        left -= share;
        shares.push((DamageTarget::Object(*blocker), share));
    }
    if let Some(excess) = options.excess {
        shares.push((excess, left));
    }
    shares
}

/// The first attacker whose controller must divide damage in `pass`, and the choice it faces.
pub(crate) fn pending_assignment(
    state: &GameState,
    pass: DamagePass,
) -> Option<(PlayerId, DamageOptions)> {
    state.combat().attackers().iter().find_map(|attacker| {
        let power = strike_power(state, attacker.object(), pass)?;
        let rules = keywords::damage_rules(state, attacker.object());
        match division(state, attacker, power, rules) {
            Division::Choose(options) => {
                let controller = controller_of(state, attacker.object())?;
                Some((controller, options))
            }
            Division::Fixed(_) => None,
        }
    })
}

/// Validates how an attacker divided its damage among its blockers and records it.
///
/// With trample the damage not given to a blocker goes to the player or planeswalker attacked, which needs
/// lethal damage on every blocker first (CR 702.19b). Without it all the damage goes to the blockers.
pub(crate) fn assignment_answer(
    options: &DamageOptions,
    assignment: Vec<(ObjectId, u32)>,
) -> Result<Outcome, IllegalAnswer> {
    for (i, (blocker, _)) in assignment.iter().enumerate() {
        if assignment[..i].iter().any(|(b, _)| b == blocker) {
            return Err(IllegalAnswer::Duplicate("blocker"));
        }
        if !options.blockers.contains(blocker) {
            return Err(IllegalAnswer::NotOffered("blocker"));
        }
    }
    let assigned = assignment
        .iter()
        .fold(0u32, |sum, (_, n)| sum.saturating_add(*n));
    let split_error = IllegalAnswer::DamageSplit {
        power: options.power,
        assigned,
    };
    if options.excess.is_none() {
        if assigned != options.power {
            return Err(split_error);
        }
    } else if assigned > options.power {
        return Err(split_error);
    } else if assigned < options.power {
        let lethal_everywhere = options
            .blockers
            .iter()
            .zip(&options.lethal)
            .all(|(b, lethal)| {
                assignment
                    .iter()
                    .find(|(given, _)| given == b)
                    .map_or(0, |(_, n)| *n)
                    >= *lethal
            });
        if !lethal_everywhere {
            return Err(IllegalAnswer::ExcessBeforeLethal);
        }
    }
    Ok(Outcome::Events(vec![Event::DamageAssigned {
        attacker: options.attacker,
        assignment,
    }]))
}

/// What deals damage and what its keywords make of the damage: the source as it is, or as it was when it has since
/// left the place it was named in (last known information, CR 608.2h).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Dealer {
    id: ObjectId,
    /// Who controls the source, as the lifelink gain reads it.
    controller: Option<PlayerId>,
    /// What its keywords do to the damage.
    keywords: DamageRules,
}

impl Dealer {
    /// The source `id` as it is now.
    pub(crate) fn live(state: &GameState, id: ObjectId) -> Dealer {
        Dealer {
            id,
            controller: controller_of(state, id),
            keywords: keywords::damage_rules(state, id),
        }
    }

    /// The source as its snapshot holds it: under the controller and with the keywords it had.
    pub(crate) fn known(shot: &Snapshot) -> Dealer {
        Dealer {
            id: shot.id,
            controller: Some(shot.controller),
            keywords: keywords::damage_rules_of(&shot.keywords),
        }
    }
}

/// The events of `source` dealing `amount` damage to `target`, with the source read as it is now.
pub(crate) fn dealt(
    state: &GameState,
    source: ObjectId,
    target: DamageTarget,
    amount: u32,
) -> Vec<Event> {
    dealt_by(state, &Dealer::live(state, source), target, amount)
}

/// The events of `dealer` dealing `amount` damage to `target`: the damage, then the life its controller gains
/// when it has lifelink (CR 702.15), in the same batch. Damage of nothing is no event.
pub(crate) fn dealt_by(
    state: &GameState,
    dealer: &Dealer,
    target: DamageTarget,
    amount: u32,
) -> Vec<Event> {
    if amount == 0 {
        return Vec::new();
    }
    let mut events = vec![Event::DamageDealt {
        source: dealer.id,
        target,
        amount,
        deathtouch: dealer.keywords.deathtouch,
    }];
    let controller = dealer
        .controller
        .filter(|controller| state.is_alive(*controller));
    if let Some(player) = controller
        && dealer.keywords.lifelink
    {
        events.push(Event::LifeGained { player, amount });
    }
    events
}

/// All combat damage for `pass`, dealt simultaneously from the same pre-damage state.
///
/// An attacker whose controller has not divided its damage yet deals the default division, which is what
/// `forecast::combat_forecast` runs this function for: the real step asks the controllers first.
pub(crate) fn damage_events(state: &GameState, pass: DamagePass) -> Vec<Event> {
    let mut events = Vec::new();
    for attacker in state.combat().attackers() {
        if let Some(power) = strike_power(state, attacker.object(), pass) {
            let rules = keywords::damage_rules(state, attacker.object());
            let shares = match division(state, attacker, power, rules) {
                Division::Fixed(shares) => shares,
                Division::Choose(options) => default_shares(&options),
            };
            for (target, amount) in shares {
                events.extend(dealt(state, attacker.object(), target, amount));
            }
        }
        for &blocker in attacker.blockers() {
            if let Some(power) = strike_power(state, blocker, pass) {
                events.extend(dealt(
                    state,
                    blocker,
                    DamageTarget::Object(attacker.object()),
                    power,
                ));
            }
        }
    }
    events
}

#[cfg(test)]
mod tests {
    //! Goal: the default division gives each blocker lethal damage in order and puts the rest on the last blocker or, with trample, on the player attacked; a controller has a choice exactly when there is more than one way to divide; and a division is accepted exactly when it respects lethal damage before the player, for any power and toughness mix.
    use super::*;
    use mtg_core::ids::PlayerId;
    use proptest::prelude::*;
    use rstest::rstest;

    const PLAYER: DamageTarget = DamageTarget::Player(PlayerId(1));

    fn options(power: u32, lethal: &[u32], trample: bool) -> DamageOptions {
        DamageOptions {
            attacker: ObjectId(1),
            power,
            blockers: (0..lethal.len())
                .map(|at| ObjectId(10 + at as u64))
                .collect(),
            excess: trample.then_some(PLAYER),
            lethal: lethal.to_vec(),
        }
    }

    /// The blockers' shares and what is left for the player, from a division.
    fn split(shares: &[(DamageTarget, u32)]) -> (Vec<u32>, u32) {
        let to_blockers = shares
            .iter()
            .filter(|(target, _)| matches!(target, DamageTarget::Object(_)))
            .map(|(_, amount)| *amount)
            .collect();
        let to_the_player = shares
            .iter()
            .filter(|(target, _)| *target == PLAYER)
            .map(|(_, amount)| *amount)
            .sum();
        (to_blockers, to_the_player)
    }

    #[rstest]
    #[case::one_blocker_takes_it_all(5, &[3], false, &[5], 0)]
    #[case::exact_fit(5, &[2, 3], false, &[2, 3], 0)]
    #[case::leftover_on_the_last(7, &[2, 3], false, &[2, 5], 0)]
    #[case::short_of_the_first(1, &[2, 3], false, &[1, 0], 0)]
    #[case::three_way(9, &[2, 2, 2], false, &[2, 2, 5], 0)]
    #[case::already_dead_first(3, &[0, 3], false, &[0, 3], 0)]
    #[case::no_power(0, &[2, 2], false, &[0, 0], 0)]
    #[case::trample_leftover_to_the_player(7, &[2, 3], true, &[2, 3], 2)]
    #[case::trample_short_of_lethal(4, &[2, 3], true, &[2, 2], 0)]
    #[case::trample_exact_lethal(5, &[2, 3], true, &[2, 3], 0)]
    #[case::trample_over_a_blocker_with_lethal_damage_marked(3, &[0], true, &[0], 3)]
    #[case::deathtouch_trample(4, &[1, 1, 1], true, &[1, 1, 1], 1)]
    fn the_default_division(
        #[case] power: u32,
        #[case] lethal: &[u32],
        #[case] trample: bool,
        #[case] blockers: &[u32],
        #[case] player: u32,
    ) {
        let shares = default_shares(&options(power, lethal, trample));
        assert_eq!(split(&shares), (blockers.to_vec(), player));
    }

    #[rstest]
    #[case::no_power(0, &[2, 2], false, false)]
    #[case::one_blocker(5, &[3], false, false)]
    #[case::two_blockers(5, &[3, 3], false, true)]
    #[case::two_blockers_with_one_point(1, &[3, 3], false, true)]
    #[case::trampler_that_can_go_past(5, &[3], true, true)]
    #[case::trampler_with_exactly_lethal(3, &[3], true, false)]
    #[case::trampler_short_of_lethal(2, &[3], true, false)]
    #[case::trampler_against_a_blocker_needing_nothing(1, &[0], true, true)]
    #[case::trampler_with_two_blockers_and_no_power(0, &[3, 3], true, false)]
    #[case::trampler_with_two_blockers(1, &[3, 3], true, true)]
    fn a_controller_has_a_choice_only_when_there_is_more_than_one_division(
        #[case] power: u32,
        #[case] lethal: &[u32],
        #[case] trample: bool,
        #[case] choice: bool,
    ) {
        assert_eq!(has_choice(&options(power, lethal, trample)), choice);
    }

    #[rstest]
    #[case::everything_on_blockers_without_trample(5, &[2, 2], false, &[(0, 3), (1, 2)], true)]
    #[case::short_without_trample(5, &[2, 2], false, &[(0, 2), (1, 2)], false)]
    #[case::too_much(5, &[2, 2], false, &[(0, 3), (1, 3)], false)]
    #[case::the_rest_past_lethal_blockers(5, &[2, 2], true, &[(0, 2), (1, 2)], true)]
    #[case::nothing_assigned_is_all_past(5, &[2, 2], true, &[], false)]
    #[case::the_rest_past_a_short_blocker(5, &[2, 2], true, &[(0, 1), (1, 2)], false)]
    #[case::everything_on_one_blocker(5, &[2, 2], true, &[(0, 5)], true)]
    #[case::more_than_the_power(5, &[2, 2], true, &[(0, 3), (1, 3)], false)]
    #[case::no_lethal_needed_past_a_blocker_that_needs_none(3, &[0], true, &[], true)]
    fn a_division_is_accepted_when_it_keeps_to_the_rules(
        #[case] power: u32,
        #[case] lethal: &[u32],
        #[case] trample: bool,
        #[case] given: &[(usize, u32)],
        #[case] accepted: bool,
    ) {
        let options = options(power, lethal, trample);
        let assignment = given
            .iter()
            .map(|(at, amount)| (options.blockers[*at], *amount))
            .collect();
        assert_eq!(assignment_answer(&options, assignment).is_ok(), accepted);
    }

    #[test]
    fn a_blocker_that_is_not_offered_or_named_twice_is_refused() {
        let options = options(4, &[2, 2], true);
        let stranger = vec![(ObjectId(99), 4)];
        assert_eq!(
            assignment_answer(&options, stranger).err(),
            Some(IllegalAnswer::NotOffered("blocker"))
        );
        let twice = vec![(options.blockers[0], 2), (options.blockers[0], 2)];
        assert_eq!(
            assignment_answer(&options, twice).err(),
            Some(IllegalAnswer::Duplicate("blocker"))
        );
        let huge = vec![
            (options.blockers[0], u32::MAX),
            (options.blockers[1], u32::MAX),
        ];
        assert_eq!(
            assignment_answer(&options, huge).err(),
            Some(IllegalAnswer::DamageSplit {
                power: 4,
                assigned: u32::MAX
            })
        );
    }

    proptest! {
        #[test]
        fn the_default_division_conserves_power_and_is_always_accepted(
            power in 0u32..40,
            lethal in prop::collection::vec(0u32..10, 1..6),
            trample in any::<bool>(),
        ) {
            let options = options(power, &lethal, trample);
            let shares = default_shares(&options);
            let (to_blockers, to_the_player) = split(&shares);
            prop_assert_eq!(to_blockers.len(), lethal.len());
            prop_assert_eq!(to_blockers.iter().sum::<u32>() + to_the_player, power);
            if !trample {
                prop_assert_eq!(to_the_player, 0);
            }
            // Past a blocker that is not the last, or past any with trample, only lethal damage is given.
            let early = if trample { lethal.len() } else { lethal.len() - 1 };
            for at in 0..early {
                prop_assert!(to_blockers[at] <= lethal[at]);
            }
            if to_the_player > 0 {
                for (share, needed) in to_blockers.iter().zip(&lethal) {
                    prop_assert!(share >= needed);
                }
            }
            // The blockers' part is what the answer lists, and the engine takes it as it is.
            let assignment = options
                .blockers
                .iter()
                .copied()
                .zip(to_blockers)
                .collect();
            prop_assert!(assignment_answer(&options, assignment).is_ok());
        }

        #[test]
        fn an_answer_that_sends_damage_past_a_short_blocker_is_never_accepted(
            lethal in prop::collection::vec(1u32..10, 1..5),
            short in 0usize..5,
            extra in 1u32..5,
        ) {
            let short = short % lethal.len();
            let options = options(lethal.iter().sum::<u32>() + extra, &lethal, true);
            let assignment: Vec<(ObjectId, u32)> = options
                .blockers
                .iter()
                .zip(&lethal)
                .enumerate()
                .map(|(at, (blocker, needed))| (*blocker, if at == short { needed - 1 } else { *needed }))
                .collect();
            prop_assert_eq!(
                assignment_answer(&options, assignment).err(),
                Some(IllegalAnswer::ExcessBeforeLethal)
            );
        }
    }

    mod leaving {
        //! Goal: damage from a source that has left is dealt as the source was: the keywords of its snapshot decide the deathtouch of the event and the life its controller of then gains, not the card it has become.
        use std::sync::Arc;

        use mtg_cards::CardDefinition;
        use mtg_cards::ability::Ability;
        use mtg_cards::keyword::Keyword;
        use mtg_core::card::CardType;
        use mtg_core::ids::EffectId;

        use super::*;
        use crate::effects::{ContinuousEffect, Duration, Modification, Selector};
        use crate::events::ZoneCause;
        use crate::fixtures::state;
        use crate::fold::apply_event;
        use crate::snapshot::snapshot;

        const FOE: PlayerId = PlayerId(1);

        fn printing(keywords: &[Keyword]) -> Arc<CardDefinition> {
            let mut card = CardDefinition::new("Source")
                .with_type(CardType::Creature)
                .with_power_toughness(3, 3);
            for keyword in keywords {
                card = card.with_rules(Ability::Keyword(keyword.clone()));
            }
            Arc::new(card)
        }

        /// Seat 0 controls a creature that prints `keywords`.
        fn game(keywords: &[Keyword]) -> (GameState, ObjectId) {
            let game = state(vec![vec![], vec![]], vec![vec![printing(keywords)], vec![]]);
            let id = game.zone(Zone::Battlefield)[0];
            (game, id)
        }

        fn change(game: &mut GameState, id: ObjectId, modification: Modification, n: u64) {
            let effect = ContinuousEffect::new(
                EffectId(n),
                Selector::objects(game, [id]),
                modification,
                Duration::UntilEndOfTurn,
                n,
            );
            apply_event(game, &Event::EffectCreated { effect }).unwrap();
        }

        /// Sacrifices the creature, so the card in the graveyard is a new object.
        fn sacrifice(game: &mut GameState, id: ObjectId) {
            apply_event(
                game,
                &Event::ZoneChanged {
                    object: id,
                    from: Zone::Battlefield,
                    to: Zone::Graveyard(PlayerId(0)),
                    cause: ZoneCause::Sacrificed,
                },
            )
            .unwrap();
        }

        /// What the damage of 3 to the foe is: whether it deathtouches, and who gains how much life.
        fn outcome(events: &[Event]) -> (bool, Vec<(PlayerId, u32)>) {
            let touched = events.iter().any(|event| {
                matches!(
                    event,
                    Event::DamageDealt {
                        deathtouch: true,
                        amount: 3,
                        ..
                    }
                )
            });
            let gains = events
                .iter()
                .filter_map(|event| match event {
                    Event::LifeGained { player, amount } => Some((*player, *amount)),
                    _ => None,
                })
                .collect();
            (touched, gains)
        }

        #[rstest]
        #[case::given_nothing(&[], false, false)]
        #[case::lifelink(&[Keyword::Lifelink], false, true)]
        #[case::deathtouch(&[Keyword::Deathtouch], true, false)]
        #[case::both(&[Keyword::Lifelink, Keyword::Deathtouch], true, true)]
        fn a_source_that_left_deals_damage_with_the_keywords_an_effect_gave_it(
            #[case] given: &[Keyword],
            #[case] deathtouch: bool,
            #[case] lifelink: bool,
        ) {
            let (mut game, id) = game(&[]);
            for (n, keyword) in given.iter().enumerate() {
                change(
                    &mut game,
                    id,
                    Modification::AddKeyword(keyword.clone()),
                    n as u64 + 1,
                );
            }
            let shot = snapshot(&game, id).unwrap();
            sacrifice(&mut game, id);
            let target = DamageTarget::Player(FOE);
            let known = dealt_by(&game, &Dealer::known(&shot), target, 3);
            assert_eq!(
                outcome(&known),
                (
                    deathtouch,
                    if lifelink {
                        vec![(PlayerId(0), 3)]
                    } else {
                        vec![]
                    }
                )
            );
            // Read as the card it has become, the source has none of them: the card prints no keyword.
            assert_eq!(outcome(&dealt(&game, id, target, 3)), (false, vec![]));
        }

        #[rstest]
        #[case::lost_every_ability(Modification::LoseAbilities)]
        #[case::lost_both_one_by_one(Modification::RemoveKeyword(Keyword::Lifelink))]
        fn a_source_that_left_without_its_printed_keywords_deals_plain_damage(
            #[case] taken: Modification,
        ) {
            let (mut game, id) = game(&[Keyword::Lifelink, Keyword::Deathtouch]);
            change(&mut game, id, taken.clone(), 1);
            if matches!(taken, Modification::RemoveKeyword(_)) {
                change(
                    &mut game,
                    id,
                    Modification::RemoveKeyword(Keyword::Deathtouch),
                    2,
                );
            }
            let shot = snapshot(&game, id).unwrap();
            sacrifice(&mut game, id);
            let target = DamageTarget::Player(FOE);
            assert_eq!(
                outcome(&dealt_by(&game, &Dealer::known(&shot), target, 3)),
                (false, vec![])
            );
            // The card in the graveyard prints them again, which is what the snapshot keeps from being read.
            assert_eq!(
                outcome(&dealt(&game, id, target, 3)),
                (true, vec![(PlayerId(0), 3)])
            );
        }

        #[test]
        fn the_life_goes_to_the_player_who_controlled_the_source_then() {
            let (mut game, id) = game(&[Keyword::Lifelink]);
            change(&mut game, id, Modification::SetController(FOE), 1);
            let shot = snapshot(&game, id).unwrap();
            assert_eq!(shot.controller, FOE);
            sacrifice(&mut game, id);
            let events = dealt_by(
                &game,
                &Dealer::known(&shot),
                DamageTarget::Player(PlayerId(0)),
                3,
            );
            assert_eq!(outcome(&events).1, [(FOE, 3)], "its owner is seat 0");
        }

        #[rstest]
        #[case::the_event_says_deathtouch_of_a_source_without_it(&[], true, true)]
        #[case::the_event_says_none_of_a_source_with_it(&[Keyword::Deathtouch], false, false)]
        #[case::both_agree_on_deathtouch(&[Keyword::Deathtouch], true, true)]
        #[case::both_agree_on_none(&[], false, false)]
        fn the_event_decides_the_deathtouch_mark_not_the_keywords_the_source_has_when_it_is_folded(
            #[case] source_has: &[Keyword],
            #[case] event_says: bool,
            #[case] marked: bool,
        ) {
            let mut game = state(
                vec![vec![], vec![]],
                vec![
                    vec![printing(source_has)],
                    vec![crate::fixtures::creature("Victim", 5, 5)],
                ],
            );
            let [source, victim] = [
                game.zone(Zone::Battlefield)[0],
                game.zone(Zone::Battlefield)[1],
            ];
            apply_event(
                &mut game,
                &Event::DamageDealt {
                    source,
                    target: DamageTarget::Object(victim),
                    amount: 1,
                    deathtouch: event_says,
                },
            )
            .unwrap();
            let permanent = game.object(victim).unwrap().permanent().unwrap();
            assert_eq!(permanent.deathtouched(), marked);
            assert_eq!(permanent.damage(), 1);
        }

        #[test]
        fn damage_of_nothing_is_no_event_for_a_source_that_left_either() {
            let (mut game, id) = game(&[Keyword::Lifelink]);
            let shot = snapshot(&game, id).unwrap();
            sacrifice(&mut game, id);
            let events = dealt_by(&game, &Dealer::known(&shot), DamageTarget::Player(FOE), 0);
            assert_eq!(events.len(), 0);
        }
    }
}
