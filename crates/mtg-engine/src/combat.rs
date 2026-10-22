//! Combat state and the legality of attacks and blocks.

use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::attack_tax;
use crate::characteristics::{
    cant_attack, cant_be_blocked_by, cant_block, characteristics, controller_of,
};
use crate::decision::{AttackOptions, AttackerOption, BlockOptions, BlockerOption};
use crate::error::IllegalAnswer;
use crate::events::Event;
use crate::keywords;
use crate::rules::{must_attack, must_attack_players, must_block};
use crate::state::GameState;
use crate::zones::Zone;

/// What an attacking creature is attacking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum AttackTarget {
    /// A player.
    Player(PlayerId),
    /// A planeswalker.
    Planeswalker(ObjectId),
}

/// Who or what receives damage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum DamageTarget {
    /// A player's life total.
    Player(PlayerId),
    /// A permanent on the battlefield.
    Object(ObjectId),
}

/// What a damage source's keywords make of the damage it deals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum DamageForm {
    /// Damage marked on a creature, life lost by a player.
    #[default]
    Normal,
    /// Wither (CR 702.80): -1/-1 counters on a creature; a player loses life as usual.
    Wither,
    /// Infect (CR 702.90): -1/-1 counters on a creature, poison counters for a player.
    Infect,
}

impl DamageForm {
    /// Whether the damage is plain damage (serialization leaves it out).
    #[must_use]
    pub const fn is_normal(&self) -> bool {
        matches!(self, DamageForm::Normal)
    }

    /// Whether a creature it hurts gets -1/-1 counters and no marked damage.
    #[must_use]
    pub const fn makes_counters(self) -> bool {
        !self.is_normal()
    }
}

/// An attacking creature and its blockers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Attacker {
    pub(crate) object: ObjectId,
    pub(crate) target: AttackTarget,
    pub(crate) blocked: bool,
    pub(crate) blockers: Vec<ObjectId>,
    pub(crate) assignment: Option<Vec<(ObjectId, u32)>>,
    /// The planeswalker it attacks has left the battlefield: it goes on attacking, aimed at nothing (CR 506.4c).
    pub(crate) target_left: bool,
}

impl Attacker {
    /// Whether the planeswalker it attacks has left the battlefield: it is still an attacking creature, deals no combat
    /// damage to the planeswalker, is not redirected and has no defending player to block it (CR 506.4c).
    pub const fn attacks_nothing(&self) -> bool {
        self.target_left
    }

    /// The player who may block it: whoever it attacks, or controls the planeswalker it attacks; nobody once that
    /// planeswalker has left.
    pub fn defender(&self, state: &GameState) -> Option<PlayerId> {
        if self.target_left {
            None
        } else {
            defending_player(state, self.target)
        }
    }

    /// The attacking creature.
    pub const fn object(&self) -> ObjectId {
        self.object
    }

    /// What it attacks.
    pub const fn target(&self) -> AttackTarget {
        self.target
    }

    /// Whether it was blocked (it stays blocked even if every blocker leaves).
    pub const fn blocked(&self) -> bool {
        self.blocked
    }

    /// Creatures currently blocking it.
    pub fn blockers(&self) -> &[ObjectId] {
        &self.blockers
    }
}

/// The combat in progress this turn.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct CombatState {
    pub(crate) attackers: Vec<Attacker>,
    pub(crate) attackers_declared: bool,
    pub(crate) blockers_declared: Vec<PlayerId>,
    /// The creatures that left combat this turn and what each was attacking, for an ability that takes the place of one.
    pub(crate) left: Vec<(ObjectId, AttackTarget)>,
}

impl CombatState {
    /// Declared attackers.
    pub fn attackers(&self) -> &[Attacker] {
        &self.attackers
    }

    /// Whether the declare-attackers choice has been made this turn.
    pub const fn attackers_declared(&self) -> bool {
        self.attackers_declared
    }

    /// Defending players who have already declared blockers.
    pub fn blockers_declared(&self) -> &[PlayerId] {
        &self.blockers_declared
    }

    /// What the creature `id` was attacking when it left combat this turn.
    pub(crate) fn left_attacking(&self, id: ObjectId) -> Option<AttackTarget> {
        self.left
            .iter()
            .rev()
            .find_map(|(object, target)| (*object == id).then_some(*target))
    }

    /// Whether `id` is an attacking creature no creature was declared to block, once its defending player has declared
    /// blockers (CR 509.1h).
    pub(crate) fn is_unblocked(&self, state: &GameState, id: ObjectId) -> bool {
        self.attackers.iter().any(|a| {
            a.object == id
                && !a.blocked
                && a.defender(state)
                    .map_or(!self.blockers_declared.is_empty(), |defender| {
                        self.blockers_declared.contains(&defender)
                    })
        })
    }

    /// Removes a creature that left the battlefield from combat (CR 506.4).
    pub(crate) fn remove_object(&mut self, id: ObjectId) {
        if let Some(attacker) = self.attackers.iter().find(|a| a.object == id) {
            let target = attacker.target;
            self.left.push((id, target));
        }
        self.attackers.retain(|a| a.object != id);
        for attacker in &mut self.attackers {
            attacker.blockers.retain(|b| *b != id);
            if let Some(assignment) = &mut attacker.assignment {
                assignment.retain(|(b, _)| *b != id);
            }
        }
    }

    /// Removes attackers aimed at a player that left the game.
    pub(crate) fn remove_attackers_targeting(&mut self, target: AttackTarget) {
        self.attackers.retain(|a| a.target != target);
    }

    /// The planeswalker `id` left the battlefield: the creatures attacking it go on attacking nothing (CR 506.4c).
    pub(crate) fn planeswalker_left(&mut self, id: ObjectId) {
        for attacker in &mut self.attackers {
            if attacker.target == AttackTarget::Planeswalker(id) {
                attacker.target_left = true;
            }
        }
    }

    /// Forgets how attackers divided their damage: each damage step starts with every attacker undivided.
    pub(crate) fn clear_assignments(&mut self) {
        for attacker in &mut self.attackers {
            attacker.assignment = None;
        }
    }
}

/// The player an attack is aimed at, i.e. who may block it.
pub fn defending_player(state: &GameState, target: AttackTarget) -> Option<PlayerId> {
    match target {
        AttackTarget::Player(p) => Some(p),
        AttackTarget::Planeswalker(id) => controller_of(state, id),
    }
}

fn is_untapped_creature(state: &GameState, id: ObjectId) -> bool {
    let Some(object) = state.object(id) else {
        return false;
    };
    object.zone() == Zone::Battlefield
        && object.permanent().is_some_and(|p| !p.tapped())
        && characteristics(state, id).is_some_and(|c| c.is_creature())
}

/// Whether `id` may be declared as an attacker by `player` now.
pub(crate) fn can_attack(state: &GameState, id: ObjectId, player: PlayerId) -> bool {
    controller_of(state, id) == Some(player)
        && !cant_attack(state, id)
        && !keywords::summoning_sick(state, id)
        && !keywords::forbids_attacking(state, id)
        && is_untapped_creature(state, id)
}

/// What `player` may attack: the players the table's attack range allows and the planeswalkers they control.
pub(crate) fn defenders_for(state: &GameState, player: PlayerId) -> Vec<AttackTarget> {
    let seats = state.ruleset().defending_seats(state, player);
    let mut targets: Vec<AttackTarget> = seats.iter().copied().map(AttackTarget::Player).collect();
    for &id in state.zone(Zone::Battlefield) {
        let is_walker =
            characteristics(state, id).is_some_and(|c| c.types.contains(CardType::Planeswalker));
        let hostile = controller_of(state, id).is_some_and(|c| seats.contains(&c));
        if is_walker && hostile {
            targets.push(AttackTarget::Planeswalker(id));
        }
    }
    targets
}

/// The player a rule pairs the creature `object` with (Encore, CR 702.141) when that player is among the `defenders`: the
/// creature attacks that player if able, so it may attack no one else. A rule that pairs it with a player who cannot be
/// attacked asks nothing of it.
fn paired_defender(
    state: &GameState,
    object: ObjectId,
    defenders: &[AttackTarget],
) -> Option<AttackTarget> {
    must_attack_players(state, object)
        .into_iter()
        .map(AttackTarget::Player)
        .find(|target| defenders.contains(target))
}

/// Every creature `player` may attack with and what each may attack.
pub(crate) fn attack_options(state: &GameState, player: PlayerId) -> AttackOptions {
    let defenders = defenders_for(state, player);
    let attackers = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| can_attack(state, *id, player))
        .map(|object| {
            AttackerOption::free(
                object,
                paired_defender(state, object, &defenders)
                    .map_or_else(|| defenders.clone(), |only| vec![only]),
            )
        })
        .collect::<Vec<_>>();
    let mut options = AttackOptions {
        attackers,
        required: Vec::new(),
        mana: 0,
    };
    attack_tax::describe(state, player, &mut options);
    // A creature that can't attack any defender is not required to (it is "able" only with one to attack), nor is one
    // that can attack only by paying a cost: a requirement never makes a player pay (CR 508.1d).
    options.required = options
        .attackers
        .iter()
        .filter(|option| {
            option.free_defender().is_some()
                && (must_attack(state, option.object)
                    || paired_defender(state, option.object, &defenders).is_some())
        })
        .map(|option| option.object)
        .collect();
    options
}

/// What declaring `attackers` costs in mana (CR 508.1h): the taxes in force added up, each attacker for what it attacks.
pub fn attack_cost(
    state: &GameState,
    attackers: &[(ObjectId, AttackTarget)],
) -> mtg_core::mana::ManaCost {
    attack_tax::total(state, attackers)
}

/// Players being attacked, in turn order after the active player.
pub(crate) fn defending_players(state: &GameState) -> Vec<PlayerId> {
    let active = state.turn().active();
    state
        .turn_order_from(active)
        .into_iter()
        .filter(|p| *p != active)
        .filter(|p| {
            state
                .combat()
                .attackers()
                .iter()
                .any(|a| a.defender(state) == Some(*p))
        })
        .collect()
}

/// Every creature `player` may block with and what each may block.
///
/// An attacker that needs several blockers (menace) is offered only while enough of `player`'s creatures could
/// block it, so a creature that could only join a block that cannot be made is not offered for it.
pub(crate) fn block_options(state: &GameState, player: PlayerId) -> BlockOptions {
    let blockable: Vec<ObjectId> = state
        .combat()
        .attackers()
        .iter()
        .filter(|a| a.defender(state) == Some(player))
        .map(Attacker::object)
        .collect();
    let candidates: Vec<(ObjectId, Vec<ObjectId>)> = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            controller_of(state, *id) == Some(player)
                && is_untapped_creature(state, *id)
                && !cant_block(state, *id)
        })
        .map(|object| {
            let can_block: Vec<ObjectId> = blockable
                .iter()
                .copied()
                .filter(|attacker| {
                    !keywords::stops_block(state, *attacker, object)
                        && !cant_be_blocked_by(state, *attacker, object)
                })
                .collect();
            (object, can_block)
        })
        .collect();
    let blockable_together = |attacker: ObjectId| {
        let able = candidates
            .iter()
            .filter(|(_, can_block)| can_block.contains(&attacker))
            .count();
        able >= keywords::min_blockers(state, attacker)
    };
    let blockers: Vec<BlockerOption> = candidates
        .iter()
        .filter_map(|(object, can_block)| {
            let can_block: Vec<ObjectId> = can_block
                .iter()
                .copied()
                .filter(|attacker| blockable_together(*attacker))
                .collect();
            (!can_block.is_empty()).then_some(BlockerOption {
                object: *object,
                can_block,
            })
        })
        .collect();
    let min_blockers = blockable
        .iter()
        .copied()
        .filter(|attacker| blockable_together(*attacker))
        .map(|attacker| (attacker, keywords::min_blockers(state, attacker)))
        .filter(|(_, needed)| *needed > 1)
        .collect();
    let required = blockers
        .iter()
        .map(|option| option.object)
        .filter(|object| must_block(state, *object))
        .collect();
    BlockOptions {
        blockers,
        min_blockers,
        required,
    }
}

/// Checks a declared attack against the options.
pub(crate) fn check_attackers(
    options: &AttackOptions,
    answer: &[(ObjectId, AttackTarget)],
) -> Result<(), IllegalAnswer> {
    for (i, (object, target)) in answer.iter().enumerate() {
        if answer[..i].iter().any(|(o, _)| o == object) {
            return Err(IllegalAnswer::Duplicate("attacker"));
        }
        let allowed = options
            .attackers
            .iter()
            .find(|a| a.object == *object)
            .ok_or(IllegalAnswer::NotOffered("attacker"))?;
        if !allowed.defenders.contains(target) {
            return Err(IllegalAnswer::NotOffered("defender"));
        }
    }
    // CR 508.1d: every requirement that can be obeyed is; a creature that is able to attack must.
    if options
        .required
        .iter()
        .any(|required| !answer.iter().any(|(object, _)| object == required))
    {
        return Err(IllegalAnswer::Required("attacker"));
    }
    Ok(())
}

/// Checks declared blocks against the options, and that an attacker that needs several blockers gets them.
pub(crate) fn check_blockers(
    state: &GameState,
    options: &BlockOptions,
    answer: &[(ObjectId, ObjectId)],
) -> Result<(), IllegalAnswer> {
    for (i, (blocker, attacker)) in answer.iter().enumerate() {
        if answer[..i].iter().any(|(b, _)| b == blocker) {
            return Err(IllegalAnswer::Duplicate("blocker"));
        }
        let allowed = options
            .blockers
            .iter()
            .find(|b| b.object == *blocker)
            .ok_or(IllegalAnswer::NotOffered("blocker"))?;
        if !allowed.can_block.contains(attacker) {
            return Err(IllegalAnswer::NotOffered("attacker to block"));
        }
    }
    for (_, attacker) in answer {
        let got = answer.iter().filter(|(_, a)| a == attacker).count();
        let needed = keywords::min_blockers(state, *attacker);
        if got < needed {
            return Err(IllegalAnswer::TooFewBlockers { needed, got });
        }
    }
    // CR 509.1c: every requirement that can be obeyed is; a creature that must block does, unless every block it could
    // make would break a restriction (menace) that no creature free to join could mend.
    if options.required.iter().any(|required| {
        !answer.iter().any(|(blocker, _)| blocker == required)
            && options.could_join(*required, answer)
    }) {
        return Err(IllegalAnswer::Required("blocker"));
    }
    Ok(())
}

/// Events for a validated attack declaration: the declaration and tapping the attackers that tap to attack.
pub(crate) fn attack_events(
    state: &GameState,
    player: PlayerId,
    attackers: &[(ObjectId, AttackTarget)],
) -> Vec<Event> {
    let mut events = vec![Event::AttackersDeclared {
        player,
        attackers: attackers.to_vec(),
    }];
    events.extend(
        attackers
            .iter()
            .filter(|(object, _)| !keywords::attacks_untapped(state, *object))
            .map(|(object, _)| Event::PermanentTapped { object: *object }),
    );
    events
}

#[cfg(test)]
mod tests {
    //! Goal: a creature a rule requires to attack or block is named by the options only while it is able to, a creature a rule pairs with a player attacks that player only and must while one paired with a player who cannot be attacked is free, a declaration that leaves it out is refused by name and one that names it is accepted, a creature that cannot attack or block is not required, a blocker is required only while it could still be added to the declaration (menace included), and a game with no such rule offers no requirement.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_cards::keyword::Keyword;
    use mtg_core::card::CardType;
    use mtg_core::ids::EffectId;
    use rstest::rstest;

    use super::*;
    use crate::effects::Duration;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::rules::{Rule, RuleEffect};
    use crate::turn::Step;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn menacing(name: &str) -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Creature)
                .with_power_toughness(2, 2)
                .with_rules(Ability::Keyword(Keyword::Menace)),
        )
    }

    /// Seat 0 controls a Wolf and a Fox, seat 1 a Bear, a Cub and a menacing Brute; it is seat 0's declare attackers step.
    fn game() -> GameState {
        let mut game = state(
            vec![vec![], vec![]],
            vec![
                vec![creature("Wolf", 2, 2), creature("Fox", 1, 1)],
                vec![
                    creature("Bear", 2, 2),
                    creature("Cub", 1, 1),
                    menacing("Brute"),
                ],
            ],
        );
        game.turn.step = Step::DeclareAttackers;
        game
    }

    fn id(game: &GameState, name: &str) -> ObjectId {
        game.objects()
            .find(|o| o.card().name == name)
            .map(crate::object::GameObject::id)
            .unwrap()
    }

    fn require(
        game: &mut GameState,
        rule: fn(Vec<crate::object::ObjectRef>) -> Rule,
        cards: &[&str],
    ) {
        let refs = cards
            .iter()
            .map(|name| game.object(id(game, name)).unwrap().reference())
            .collect();
        let effect = RuleEffect {
            id: EffectId(game.next_effect),
            rule: rule(refs),
            duration: Duration::Forever,
            source: None,
            controller: ME,
            turn: 1,
        };
        apply_event(game, &Event::RuleCreated { effect }).unwrap();
    }

    fn attacks(game: &GameState, names: &[&str]) -> Vec<(ObjectId, AttackTarget)> {
        names
            .iter()
            .map(|name| (id(game, name), AttackTarget::Player(FOE)))
            .collect()
    }

    #[rstest]
    #[case::the_one_named(&["Wolf"], &["Wolf"])]
    #[case::both(&["Wolf", "Fox"], &["Wolf", "Fox"])]
    #[case::one_that_is_not_ours(&["Bear"], &[])]
    fn attackers_that_must_attack_are_named_and_the_declaration_names_them(
        #[case] ruled: &[&str],
        #[case] required: &[&str],
    ) {
        let mut game = game();
        require(&mut game, Rule::MustAttack, ruled);
        let options = attack_options(&game, ME);
        let mut named: Vec<ObjectId> = options.required.clone();
        named.sort();
        let mut expected: Vec<ObjectId> = required.iter().map(|name| id(&game, name)).collect();
        expected.sort();
        assert_eq!(named, expected);
        if !required.is_empty() {
            assert_eq!(
                check_attackers(&options, &[]),
                Err(IllegalAnswer::Required("attacker"))
            );
            assert_eq!(check_attackers(&options, &attacks(&game, required)), Ok(()));
            assert_eq!(
                check_attackers(&options, &attacks(&game, &["Wolf", "Fox"])),
                Ok(())
            );
        }
    }

    #[test]
    fn a_creature_that_cannot_attack_is_not_required_to() {
        let mut game = game();
        require(&mut game, Rule::MustAttack, &["Wolf", "Fox"]);
        let fox = id(&game, "Fox");
        apply_event(&mut game, &Event::PermanentTapped { object: fox }).unwrap();
        let options = attack_options(&game, ME);
        assert_eq!(options.required, [id(&game, "Wolf")]);
        assert_eq!(
            check_attackers(&options, &attacks(&game, &["Fox"])),
            Err(IllegalAnswer::NotOffered("attacker"))
        );
    }

    /// `seats` players: seat 0 controls a Wolf and a Fox, every other seat a Bear; it is seat 0's declare attackers step.
    fn table(seats: usize) -> GameState {
        let mut fields = vec![vec![creature("Wolf", 2, 2), creature("Fox", 1, 1)]];
        fields.extend((1..seats).map(|seat| vec![creature(&format!("Bear {seat}"), 2, 2)]));
        let mut game = state(vec![vec![]; seats], fields);
        game.turn.step = Step::DeclareAttackers;
        game
    }

    /// A rule that pairs each creature with the seat after it (Encore, CR 702.141).
    fn pair(game: &mut GameState, pairs: &[(&str, usize)]) {
        let named = pairs
            .iter()
            .map(|(name, seat)| {
                let object = game.object(id(game, name)).unwrap().reference();
                (object, PlayerId::from_index(*seat))
            })
            .collect();
        let effect = RuleEffect {
            id: EffectId(game.next_effect),
            rule: Rule::MustAttackPlayers(named),
            duration: Duration::UntilEndOfTurn,
            source: None,
            controller: ME,
            turn: 1,
        };
        apply_event(game, &Event::RuleCreated { effect }).unwrap();
    }

    fn defenders_of(options: &AttackOptions, game: &GameState, name: &str) -> Vec<AttackTarget> {
        let wanted = id(game, name);
        options
            .attackers
            .iter()
            .find(|a| a.object == wanted)
            .map(|a| a.defenders.clone())
            .unwrap()
    }

    #[rstest]
    #[case::two_seats(2)]
    #[case::three_seats(3)]
    #[case::four_seats(4)]
    fn a_creature_paired_with_a_player_attacks_that_player_only_and_must(#[case] seats: usize) {
        let mut game = table(seats);
        let last = seats - 1;
        pair(&mut game, &[("Wolf", 1), ("Fox", last)]);
        let options = attack_options(&game, ME);
        let player = |seat: usize| vec![AttackTarget::Player(PlayerId::from_index(seat))];
        assert_eq!(defenders_of(&options, &game, "Wolf"), player(1));
        assert_eq!(defenders_of(&options, &game, "Fox"), player(last));
        let mut named = options.required.clone();
        named.sort();
        let mut expected = vec![id(&game, "Wolf"), id(&game, "Fox")];
        expected.sort();
        assert_eq!(named, expected);
        let attack = |wolf: usize, fox: usize| {
            vec![
                (
                    id(&game, "Wolf"),
                    AttackTarget::Player(PlayerId::from_index(wolf)),
                ),
                (
                    id(&game, "Fox"),
                    AttackTarget::Player(PlayerId::from_index(fox)),
                ),
            ]
        };
        assert_eq!(check_attackers(&options, &attack(1, last)), Ok(()));
        assert_eq!(
            check_attackers(&options, &attack(1, last)[..1]),
            Err(IllegalAnswer::Required("attacker"))
        );
        if seats > 2 {
            assert_eq!(
                check_attackers(&options, &attack(last, last)),
                Err(IllegalAnswer::NotOffered("defender"))
            );
        }
        assert_eq!(options.required_attack(), attack(1, last));
    }

    #[rstest]
    #[case::one_that_left(3, &[2], &[1])]
    #[case::the_only_one(2, &[1], &[])]
    #[case::one_of_three_left(4, &[3], &[1, 2])]
    fn a_creature_paired_with_a_player_who_cannot_be_attacked_is_free(
        #[case] seats: usize,
        #[case] gone: &[usize],
        #[case] left: &[usize],
    ) {
        let mut game = table(seats);
        pair(&mut game, &[("Wolf", seats - 1)]);
        for seat in gone {
            let event = Event::PlayerLost {
                player: PlayerId::from_index(*seat),
                reason: crate::player::LossReason::Conceded,
            };
            apply_event(&mut game, &event).unwrap();
        }
        let options = attack_options(&game, ME);
        assert_eq!(options.required, []);
        let defenders = defenders_of(&options, &game, "Wolf");
        assert_eq!(defenders_of(&options, &game, "Fox"), defenders);
        let seats_left: Vec<AttackTarget> = left
            .iter()
            .map(|seat| AttackTarget::Player(PlayerId::from_index(*seat)))
            .collect();
        assert_eq!(defenders, seats_left);
    }

    #[test]
    fn a_creature_that_must_attack_with_no_player_to_attack_is_not_forced() {
        let mut game = game();
        require(&mut game, Rule::MustAttack, &["Wolf", "Fox"]);
        apply_event(
            &mut game,
            &Event::PlayerLost {
                player: FOE,
                reason: crate::player::LossReason::Conceded,
            },
        )
        .unwrap();
        let options = attack_options(&game, ME);
        assert!(options.attackers.iter().all(|a| a.defenders.is_empty()));
        assert_eq!(options.required, []);
        assert_eq!(check_attackers(&options, &[]), Ok(()));
    }

    #[test]
    fn a_game_without_the_rule_requires_nothing() {
        let game = game();
        let options = attack_options(&game, ME);
        assert_eq!(options.required, []);
        assert_eq!(check_attackers(&options, &[]), Ok(()));
    }

    /// The Wolf and the Fox attack seat 1; seat 1 may block with the Bear, the Cub and the Brute.
    fn attacked() -> GameState {
        let mut game = game();
        let attackers = attacks(&game, &["Wolf", "Fox"]);
        apply_event(
            &mut game,
            &Event::AttackersDeclared {
                player: ME,
                attackers,
            },
        )
        .unwrap();
        game.turn.step = Step::DeclareBlockers;
        game
    }

    fn blocks(game: &GameState, pairs: &[(&str, &str)]) -> Vec<(ObjectId, ObjectId)> {
        pairs
            .iter()
            .map(|(blocker, attacker)| (id(game, blocker), id(game, attacker)))
            .collect()
    }

    #[rstest]
    #[case::nothing(&[], false)]
    #[case::the_wrong_creature(&[("Cub", "Fox")], false)]
    #[case::the_creature(&[("Bear", "Wolf")], true)]
    #[case::the_creature_at_the_other_attacker(&[("Bear", "Fox")], true)]
    #[case::with_others(&[("Bear", "Wolf"), ("Cub", "Fox")], true)]
    fn a_blocker_that_must_block_is_in_every_declaration(
        #[case] declared: &[(&str, &str)],
        #[case] legal: bool,
    ) {
        let mut game = attacked();
        require(&mut game, Rule::MustBlock, &["Bear"]);
        let options = block_options(&game, FOE);
        assert_eq!(options.required, [id(&game, "Bear")]);
        let result = check_blockers(&game, &options, &blocks(&game, declared));
        assert_eq!(result.is_ok(), legal, "{result:?}");
        if !legal {
            assert!(matches!(
                result,
                Err(IllegalAnswer::Required("blocker") | IllegalAnswer::NotOffered(_))
            ));
        }
    }

    #[test]
    fn a_blocker_that_cannot_block_what_attacks_is_not_required_to() {
        let mut game = attacked();
        require(&mut game, Rule::MustBlock, &["Bear"]);
        let wolf = id(&game, "Wolf");
        let fox = id(&game, "Fox");
        // Nothing attacks them any more: the declaration is made against an empty attack.
        game.combat.remove_object(wolf);
        game.combat.remove_object(fox);
        let options = block_options(&game, FOE);
        assert_eq!(options.required, []);
        assert_eq!(check_blockers(&game, &options, &[]), Ok(()));
    }

    /// Seat 1's Brute (menace) attacks seat 0, whose Wolf must block and whose Fox is tapped or not.
    fn brute_attacks(fox_tapped: bool) -> GameState {
        let mut game = game();
        if fox_tapped {
            let fox = id(&game, "Fox");
            apply_event(&mut game, &Event::PermanentTapped { object: fox }).unwrap();
        }
        game.turn.active = FOE;
        let declared = vec![(id(&game, "Brute"), AttackTarget::Player(ME))];
        let event = Event::AttackersDeclared {
            player: FOE,
            attackers: declared,
        };
        apply_event(&mut game, &event).unwrap();
        game.turn.step = Step::DeclareBlockers;
        require(&mut game, Rule::MustBlock, &["Wolf"]);
        game
    }

    #[test]
    fn a_creature_that_must_block_is_not_required_to_break_a_restriction() {
        // The Wolf alone may not block a menace attacker, so nothing is required of it.
        let game = brute_attacks(true);
        let options = block_options(&game, ME);
        assert_eq!(options.required, []);
        assert_eq!(check_blockers(&game, &options, &[]), Ok(()));
    }

    #[test]
    fn a_creature_that_must_block_a_menace_attacker_is_joined_by_a_free_creature() {
        let game = brute_attacks(false);
        let options = block_options(&game, ME);
        assert_eq!(options.required, [id(&game, "Wolf")]);
        assert_eq!(
            check_blockers(&game, &options, &[]),
            Err(IllegalAnswer::Required("blocker"))
        );
        assert_eq!(
            check_blockers(&game, &options, &blocks(&game, &[("Wolf", "Brute")])),
            Err(IllegalAnswer::TooFewBlockers { needed: 2, got: 1 })
        );
        let pair = blocks(&game, &[("Wolf", "Brute"), ("Fox", "Brute")]);
        assert_eq!(check_blockers(&game, &options, &pair), Ok(()));
        assert_eq!(
            options.required_blocks().len(),
            2,
            "the least the rules ask is the pair"
        );
    }
}
