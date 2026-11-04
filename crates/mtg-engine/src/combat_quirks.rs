//! Combat rules that name creatures: Provoke ("blocks it if able"), "must be blocked", "can't attack or block alone", the
//! permissions to attack despite defender and to block while tapped, and targeting as though hexproof or shroud were absent.

use std::collections::BTreeMap;

use mtg_cards::keyword::Keyword;
use mtg_cards::modify::{Blocking, CombatQuirk};
use mtg_core::ids::{ObjectId, PlayerId};

use crate::decision::{BlockOptions, BlockerOption};
use crate::error::IllegalAnswer;
use crate::rules::Rule;
use crate::state::GameState;

/// Hexproof or shroud the players' spells and abilities ignore on the permanents and players named.
#[derive(Debug, Clone)]
struct Ignored {
    by: Vec<PlayerId>,
    what: Vec<ObjectId>,
    players: Vec<PlayerId>,
    shroud: bool,
}

/// The combat permissions and requirements in force.
#[derive(Debug, Default)]
pub(crate) struct QuirkRules {
    quirks: BTreeMap<ObjectId, Vec<CombatQuirk>>,
    provoked: Vec<(ObjectId, ObjectId)>,
    ignored: Vec<Ignored>,
}

impl QuirkRules {
    /// No rule in force.
    pub(crate) const NONE: QuirkRules = QuirkRules {
        quirks: BTreeMap::new(),
        provoked: Vec::new(),
        ignored: Vec::new(),
    };

    /// Puts a rule of this file in force; any other rule changes nothing.
    pub(crate) fn put(&mut self, rule: &Rule) {
        match rule {
            Rule::MustBlockAttacker { blockers, attacker } => self
                .provoked
                .extend(blockers.iter().map(|blocker| (blocker.id, attacker.id))),
            Rule::Combat { quirk, creatures } => {
                for creature in creatures {
                    let held = self.quirks.entry(creature.id).or_default();
                    if !held.contains(quirk) {
                        held.push(*quirk);
                    }
                }
            }
            Rule::IgnoreHexproof {
                by,
                what,
                players,
                shroud,
            } => self.ignored.push(Ignored {
                by: by.clone(),
                what: what.iter().map(|named| named.id).collect(),
                players: players.clone(),
                shroud: *shroud,
            }),
            _ => {}
        }
    }

    fn has(&self, id: ObjectId, wanted: impl Fn(&CombatQuirk) -> bool) -> bool {
        self.quirks
            .get(&id)
            .is_some_and(|held| held.iter().any(wanted))
    }
}

/// Whether the creature may attack as though it did not have defender.
pub(crate) fn attacks_despite_defender(state: &GameState, id: ObjectId) -> bool {
    state
        .rules()
        .quirks
        .has(id, |q| matches!(q, CombatQuirk::AttackDespiteDefender))
}

/// Whether the creature may block as though it were untapped.
pub(crate) fn blocks_while_tapped(state: &GameState, id: ObjectId) -> bool {
    state
        .rules()
        .quirks
        .has(id, |q| matches!(q, CombatQuirk::BlockWhileTapped))
}

/// Whether the creature can't attack unless another creature attacks too.
pub(crate) fn cant_attack_alone(state: &GameState, id: ObjectId) -> bool {
    state
        .rules()
        .quirks
        .has(id, |q| matches!(q, CombatQuirk::CantAttackAlone))
}

/// Whether the creature can't block unless another creature blocks too.
pub(crate) fn cant_block_alone(state: &GameState, id: ObjectId) -> bool {
    state
        .rules()
        .quirks
        .has(id, |q| matches!(q, CombatQuirk::CantBlockAlone))
}

/// How the attacker must be blocked, if a rule says it must be.
pub(crate) fn must_be_blocked(state: &GameState, attacker: ObjectId) -> Option<Blocking> {
    let held = state.rules().quirks.quirks.get(&attacker)?;
    held.iter()
        .filter_map(|quirk| match quirk {
            CombatQuirk::MustBeBlocked(how) => Some(*how),
            _ => None,
        })
        .max_by_key(|how| matches!(how, Blocking::Every))
}

/// The attackers the creature must block if able: those a Provoke names and those that must be blocked by every creature
/// able to.
pub(crate) fn provoked_attackers(state: &GameState, blocker: ObjectId) -> Vec<ObjectId> {
    state
        .rules()
        .quirks
        .provoked
        .iter()
        .filter(|(held, _)| *held == blocker)
        .map(|(_, attacker)| *attacker)
        .collect()
}

/// Whether a hexproof or shroud keyword of the permanent `holder` (or the player, for `Holder::Player`) does not keep the
/// spell or ability of `controller` from targeting it.
pub(crate) fn ignores_protection(
    state: &GameState,
    holder: Holder,
    keyword: &Keyword,
    controller: PlayerId,
) -> bool {
    let shroud = matches!(keyword, Keyword::Shroud);
    if !shroud && !matches!(keyword, Keyword::Hexproof) {
        return false;
    }
    state.rules().quirks.ignored.iter().any(|rule| {
        (!shroud || rule.shroud)
            && rule.by.contains(&controller)
            && match holder {
                Holder::Object(id) => rule.what.contains(&id),
                Holder::Player(id) => rule.players.contains(&id),
            }
    })
}

/// What a protection keyword belongs to.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Holder {
    /// A permanent.
    Object(ObjectId),
    /// A player.
    Player(PlayerId),
}

/// Refuses a declaration whose only attacker can't attack alone.
pub(crate) fn check_attack_alone(
    state: &GameState,
    attackers: &[ObjectId],
) -> Result<(), IllegalAnswer> {
    match attackers {
        [only] if cant_attack_alone(state, *only) => Err(IllegalAnswer::NotAlone("attacker")),
        _ => Ok(()),
    }
}

/// Refuses a declaration whose only blocker can't block alone.
pub(crate) fn check_block_alone(
    state: &GameState,
    blocks: &[(ObjectId, ObjectId)],
) -> Result<(), IllegalAnswer> {
    let mut blockers: Vec<ObjectId> = blocks.iter().map(|(blocker, _)| *blocker).collect();
    blockers.dedup();
    blockers.sort_unstable();
    blockers.dedup();
    match blockers.as_slice() {
        [only] if cant_block_alone(state, *only) => Err(IllegalAnswer::NotAlone("blocker")),
        _ => Ok(()),
    }
}

impl BlockOptions {
    /// The attackers among `blockers` that something could still block: they are blocked by none in `blocks`, and a
    /// creature could be added to or moved into their block without leaving another requirement unmet.
    pub fn unmet_must_be_blocked(&self, blocks: &[(ObjectId, ObjectId)]) -> bool {
        self.must_be_blocked.iter().any(|attacker| {
            !blocks.iter().any(|(_, held)| held == attacker)
                && self
                    .blockers
                    .iter()
                    .any(|option| self.could_serve(option, *attacker, blocks))
        })
    }

    /// Whether `option` could block `attacker`, which nothing blocks, without breaking a rule or giving up a requirement
    /// of its own.
    fn could_serve(
        &self,
        option: &BlockerOption,
        attacker: ObjectId,
        blocks: &[(ObjectId, ObjectId)],
    ) -> bool {
        if !option.can_block.contains(&attacker) {
            return false;
        }
        let needed = self.needed_for(attacker);
        let busy: Vec<ObjectId> = blocks
            .iter()
            .filter(|(blocker, _)| *blocker == option.object)
            .map(|(_, held)| *held)
            .collect();
        if busy.is_empty() {
            let blocking = crate::w14c_block_capacity::w14c_distinct_blockers(blocks).len();
            if self.most.is_some_and(|most| blocking >= most) {
                return false;
            }
            let free = self
                .blockers
                .iter()
                .filter(|other| {
                    other.object != option.object
                        && other.can_block.contains(&attacker)
                        && !blocks.iter().any(|(held, _)| *held == other.object)
                })
                .count();
            return free + 1 >= needed;
        }
        // Moving a creature that blocks only because it must, or the only blocker of an attacker that must be blocked,
        // trades one requirement for another.
        needed <= 1
            && !self.required.contains(&option.object)
            && busy.iter().all(|held| {
                !self.must_be_blocked.contains(held)
                    || blocks
                        .iter()
                        .any(|(blocker, at)| at == held && *blocker != option.object)
            })
    }

    /// Adds to `blocks` a creature for each attacker that must be blocked and has none, if one is free.
    pub(crate) fn serve_must_be_blocked(&self, blocks: &mut Vec<(ObjectId, ObjectId)>) {
        for attacker in &self.must_be_blocked {
            if blocks.iter().any(|(_, held)| held == attacker) {
                continue;
            }
            let free = self.blockers.iter().find(|option| {
                option.can_block.contains(attacker)
                    && !blocks.iter().any(|(blocker, _)| *blocker == option.object)
            });
            if let Some(option) = free {
                blocks.push((option.object, *attacker));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a creature a rule makes block an attacker (Provoke, "all creatures able to block it") is offered that attacker
    //! and is required to block it, an attacker that must be blocked is blocked by some creature unless every creature that
    //! could is busy with a requirement of its own, no rule leaves the declaration without a legal answer and the default
    //! answer is always legal, a creature that can't attack or block alone is never declared alone, a creature attacks
    //! despite defender and blocks while tapped when a rule says so, and a hexproof or shroud permanent or player is
    //! targetable by the players a rule names.
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_cards::modify::GameRule;
    use mtg_cards::refs::Ref;
    use mtg_core::card::CardType;
    use mtg_core::ids::EffectId;
    use rstest::rstest;

    use super::*;
    use crate::combat::{
        AttackTarget, attack_options, block_options, check_attackers, check_blockers,
    };
    use crate::effects::Duration;
    use crate::events::Event;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::resolve::Scope;
    use crate::rules::{RuleEffect, lower_rule};
    use crate::turn::Step;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn keyworded(name: &str, keyword: Keyword) -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Creature)
                .with_power_toughness(2, 2)
                .with_rules(Ability::Keyword(keyword)),
        )
    }

    fn id(game: &GameState, name: &str) -> ObjectId {
        game.objects()
            .find(|o| o.card().name == name)
            .map(crate::object::GameObject::id)
            .unwrap()
    }

    fn name_of(game: &GameState, object: ObjectId) -> String {
        game.object(object).unwrap().card().name.clone()
    }

    /// Seat 0 controls `me`, seat 1 `foe`; seat 0 declares attackers.
    fn game_with(me: Vec<Arc<CardDefinition>>, foe: Vec<Arc<CardDefinition>>) -> GameState {
        let mut game = state(vec![vec![], vec![]], vec![me, foe]);
        game.turn.step = Step::DeclareAttackers;
        game
    }

    fn game() -> GameState {
        game_with(
            vec![creature("Wolf", 2, 2), creature("Fox", 1, 1)],
            vec![
                creature("Bear", 2, 2),
                creature("Cub", 1, 1),
                creature("Boar", 3, 3),
            ],
        )
    }

    fn rule(game: &mut GameState, rule: Rule) {
        let effect = RuleEffect {
            id: EffectId(game.next_effect),
            rule,
            duration: Duration::Forever,
            source: None,
            controller: ME,
            turn: 1,
        };
        let event = Event::RuleCreated {
            effect: effect.into(),
        };
        apply_event(game, &event).unwrap();
    }

    fn refs(game: &GameState, names: &[&str]) -> Vec<crate::object::ObjectRef> {
        names
            .iter()
            .map(|name| game.object(id(game, name)).unwrap().reference())
            .collect()
    }

    fn provoke(game: &mut GameState, blockers: &[&str], attacker: &str) {
        let wanted = Rule::MustBlockAttacker {
            blockers: refs(game, blockers),
            attacker: refs(game, &[attacker])[0],
        };
        rule(game, wanted);
    }

    fn quirk(game: &mut GameState, quirk: CombatQuirk, names: &[&str]) {
        let creatures = refs(game, names);
        rule(game, Rule::Combat { quirk, creatures });
    }

    fn tap(game: &mut GameState, name: &str) {
        let object = id(game, name);
        apply_event(game, &Event::PermanentTapped { object }).unwrap();
    }

    /// The named creatures of seat 0 attack seat 1, which declares blockers.
    fn attacked_by(game: &mut GameState, names: &[&str]) {
        let attackers = names
            .iter()
            .map(|name| (id(game, name), AttackTarget::Player(FOE)))
            .collect();
        let event = Event::AttackersDeclared {
            player: ME,
            attackers,
        };
        apply_event(game, &event).unwrap();
        game.turn.step = Step::DeclareBlockers;
    }

    fn attacked(game: &mut GameState) {
        attacked_by(game, &["Wolf", "Fox"]);
    }

    /// Every declaration the options offer, as "Blocker>Attacker" sets, with whether the check accepts it.
    fn declarations(game: &GameState) -> Vec<(BTreeSet<String>, bool)> {
        let options = block_options(game, FOE);
        let mut all: Vec<Vec<(ObjectId, ObjectId)>> = vec![Vec::new()];
        for option in &options.blockers {
            let mut next = Vec::new();
            for held in &all {
                next.push(held.clone());
                for attacker in &option.can_block {
                    let mut more = held.clone();
                    more.push((option.object, *attacker));
                    next.push(more);
                }
            }
            all = next;
        }
        all.into_iter()
            .map(|blocks| {
                let ok = check_blockers(game, &options, &blocks).is_ok();
                let words = blocks
                    .iter()
                    .map(|(b, a)| format!("{}>{}", name_of(game, *b), name_of(game, *a)))
                    .collect();
                (words, ok)
            })
            .collect()
    }

    fn legal(game: &GameState) -> Vec<BTreeSet<String>> {
        declarations(game)
            .into_iter()
            .filter_map(|(words, ok)| ok.then_some(words))
            .collect()
    }

    /// The default answer is legal, and some answer is.
    fn assert_playable(game: &GameState) {
        let options = block_options(game, FOE);
        let default = options.required_blocks();
        assert_eq!(
            check_blockers(game, &options, &default),
            Ok(()),
            "{default:?}"
        );
        assert!(!legal(game).is_empty());
    }

    #[rstest]
    #[case::one(&["Bear"], "Wolf", &["Bear>Wolf"])]
    #[case::two(&["Bear", "Cub"], "Fox", &["Bear>Fox", "Cub>Fox"])]
    #[case::all_three(&["Bear", "Cub", "Boar"], "Wolf", &["Bear>Wolf", "Cub>Wolf", "Boar>Wolf"])]
    fn a_provoked_creature_blocks_the_attacker_and_every_legal_declaration_has_it(
        #[case] blockers: &[&str],
        #[case] attacker: &str,
        #[case] must: &[&str],
    ) {
        let mut game = game();
        attacked(&mut game);
        provoke(&mut game, blockers, attacker);
        assert_playable(&game);
        let all = legal(&game);
        assert!(
            all.iter()
                .all(|words| must.iter().all(|m| words.contains(*m)))
        );
        // The others stay free: a declaration with one more block is legal where a free creature remains.
        assert_eq!(
            all.iter().any(|words| words.len() > must.len()),
            blockers.len() < 3
        );
        assert_eq!(block_options(&game, FOE).required.len(), blockers.len());
    }

    #[test]
    fn a_provoked_creature_that_cannot_block_the_attacker_is_free() {
        let mut game = game();
        attacked(&mut game);
        provoke(&mut game, &["Bear"], "Wolf");
        tap(&mut game, "Bear");
        assert!(block_options(&game, FOE).required.is_empty());
        assert_playable(&game);
    }

    #[test]
    fn a_provoked_creature_is_offered_the_attacker_it_must_block_only() {
        let mut game = game();
        attacked(&mut game);
        provoke(&mut game, &["Bear"], "Wolf");
        let options = block_options(&game, FOE);
        let find = |name: &str| {
            options
                .blockers
                .iter()
                .find(|o| o.object == id(&game, name))
                .unwrap()
        };
        assert_eq!(find("Bear").can_block, [id(&game, "Wolf")]);
        assert_eq!(find("Cub").can_block.len(), 2);
    }

    #[test]
    fn two_provoked_creatures_on_one_attacker_that_must_be_blocked_are_never_stuck() {
        let mut game = game();
        attacked(&mut game);
        provoke(&mut game, &["Bear", "Cub"], "Wolf");
        quirk(
            &mut game,
            CombatQuirk::MustBeBlocked(Blocking::AtLeastOne),
            &["Wolf"],
        );
        assert_playable(&game);
        let all = legal(&game);
        assert!(
            all.iter()
                .all(|w| w.contains("Bear>Wolf") && w.contains("Cub>Wolf"))
        );
    }

    #[test]
    fn provoke_with_a_menace_attacker_brings_a_free_creature() {
        let mut game = game_with(
            vec![keyworded("Brute", Keyword::Menace), creature("Fox", 1, 1)],
            vec![creature("Bear", 2, 2), creature("Cub", 1, 1)],
        );
        attacked_by(&mut game, &["Brute"]);
        provoke(&mut game, &["Bear"], "Brute");
        assert_playable(&game);
        assert_eq!(block_options(&game, FOE).required_blocks().len(), 2);
        tap(&mut game, "Cub");
        assert!(
            block_options(&game, FOE).required.is_empty(),
            "no second blocker: the attacker can't be blocked"
        );
        assert_playable(&game);
    }

    #[rstest]
    #[case::some(Blocking::AtLeastOne)]
    #[case::every(Blocking::Every)]
    fn an_attacker_that_must_be_blocked_is_blocked(#[case] how: Blocking) {
        let mut game = game();
        attacked(&mut game);
        quirk(&mut game, CombatQuirk::MustBeBlocked(how), &["Wolf"]);
        assert_playable(&game);
        let all = legal(&game);
        match how {
            Blocking::AtLeastOne => {
                assert!(all.iter().all(|w| w.iter().any(|b| b.ends_with(">Wolf"))));
                assert!(all.iter().any(|w| w.len() == 3));
            }
            Blocking::Every => assert!(all.iter().all(|w| {
                ["Bear>Wolf", "Cub>Wolf", "Boar>Wolf"]
                    .iter()
                    .all(|b| w.contains(*b))
            })),
        }
    }

    #[test]
    fn an_attacker_that_must_be_blocked_is_dropped_when_its_only_blocker_must_block_another() {
        let mut game = game_with(
            vec![creature("Wolf", 2, 2), creature("Fox", 1, 1)],
            vec![creature("Bear", 2, 2)],
        );
        attacked(&mut game);
        quirk(
            &mut game,
            CombatQuirk::MustBeBlocked(Blocking::AtLeastOne),
            &["Wolf"],
        );
        provoke(&mut game, &["Bear"], "Fox");
        assert_playable(&game);
        assert!(block_options(&game, FOE).must_be_blocked.is_empty());
        assert_eq!(legal(&game), [["Bear>Fox".to_owned()].into()]);
    }

    #[test]
    fn two_attackers_that_must_be_blocked_by_one_creature_leave_one_unblocked() {
        let mut game = game_with(
            vec![creature("Wolf", 2, 2), creature("Fox", 1, 1)],
            vec![creature("Bear", 2, 2)],
        );
        attacked(&mut game);
        quirk(
            &mut game,
            CombatQuirk::MustBeBlocked(Blocking::AtLeastOne),
            &["Wolf", "Fox"],
        );
        assert_playable(&game);
        let all = legal(&game);
        assert_eq!(all.len(), 2, "{all:?}");
        assert!(all.iter().all(|words| words.len() == 1));
    }

    #[rstest]
    #[case::alone(&[("Bear", "Wolf")], false)]
    #[case::with_another(&[("Bear", "Wolf"), ("Cub", "Fox")], true)]
    #[case::the_other_alone(&[("Cub", "Fox")], true)]
    #[case::nothing(&[], true)]
    fn a_creature_that_cant_block_alone_is_never_the_only_blocker(
        #[case] declared: &[(&str, &str)],
        #[case] ok: bool,
    ) {
        let mut game = game();
        attacked(&mut game);
        quirk(&mut game, CombatQuirk::CantBlockAlone, &["Bear"]);
        let options = block_options(&game, FOE);
        let blocks: Vec<(ObjectId, ObjectId)> = declared
            .iter()
            .map(|(b, a)| (id(&game, b), id(&game, a)))
            .collect();
        assert_eq!(check_blockers(&game, &options, &blocks).is_ok(), ok);
    }

    #[test]
    fn a_creature_that_cant_block_alone_is_not_forced_by_a_requirement() {
        let mut game = game_with(vec![creature("Wolf", 2, 2)], vec![creature("Bear", 2, 2)]);
        attacked_by(&mut game, &["Wolf"]);
        quirk(&mut game, CombatQuirk::CantBlockAlone, &["Bear"]);
        provoke(&mut game, &["Bear"], "Wolf");
        assert!(block_options(&game, FOE).required.is_empty());
        assert_playable(&game);
    }

    #[rstest]
    #[case::alone(&["Wolf"], false)]
    #[case::with_another(&["Wolf", "Fox"], true)]
    #[case::the_other_alone(&["Fox"], true)]
    #[case::none(&[], true)]
    fn a_creature_that_cant_attack_alone_is_never_the_only_attacker(
        #[case] declared: &[&str],
        #[case] ok: bool,
    ) {
        let mut game = game();
        quirk(&mut game, CombatQuirk::CantAttackAlone, &["Wolf"]);
        let options = attack_options(&game, ME);
        let ids: Vec<ObjectId> = declared.iter().map(|name| id(&game, name)).collect();
        let answer: Vec<(ObjectId, AttackTarget)> = ids
            .iter()
            .map(|o| (*o, AttackTarget::Player(FOE)))
            .collect();
        assert_eq!(check_attackers(&options, &answer), Ok(()));
        assert_eq!(check_attack_alone(&game, &ids).is_ok(), ok);
    }

    #[test]
    fn a_creature_that_cant_attack_alone_and_must_attack_is_not_forced_to() {
        let mut game = game();
        quirk(&mut game, CombatQuirk::CantAttackAlone, &["Wolf"]);
        let wolf = refs(&game, &["Wolf"]);
        rule(&mut game, Rule::MustAttack(wolf));
        assert!(attack_options(&game, ME).required.is_empty());
        assert!(attack_options(&game, ME).required_attack().is_empty());
    }

    #[rstest]
    #[case::without_the_rule(false)]
    #[case::with_the_rule(true)]
    fn a_creature_with_defender_attacks_when_a_rule_says_so(#[case] ruled: bool) {
        let mut game = game_with(
            vec![keyworded("Wall", Keyword::Defender), creature("Fox", 1, 1)],
            vec![creature("Bear", 2, 2)],
        );
        if ruled {
            quirk(&mut game, CombatQuirk::AttackDespiteDefender, &["Wall"]);
        }
        let wall = id(&game, "Wall");
        let options = attack_options(&game, ME);
        assert_eq!(options.attackers.iter().any(|a| a.object == wall), ruled);
    }

    #[rstest]
    #[case::without_the_rule(false)]
    #[case::with_the_rule(true)]
    fn a_tapped_creature_blocks_when_a_rule_says_so(#[case] ruled: bool) {
        let mut game = game();
        attacked(&mut game);
        tap(&mut game, "Bear");
        if ruled {
            quirk(&mut game, CombatQuirk::BlockWhileTapped, &["Bear"]);
        }
        let bear = id(&game, "Bear");
        let options = block_options(&game, FOE);
        assert_eq!(options.blockers.iter().any(|o| o.object == bear), ruled);
    }

    #[rstest]
    #[case::hexproof_ignored(Keyword::Hexproof, false, ME, false)]
    #[case::shroud_kept(Keyword::Shroud, false, ME, true)]
    #[case::shroud_ignored_when_named(Keyword::Shroud, true, ME, false)]
    fn a_rule_lets_its_players_target_through_hexproof_and_shroud(
        #[case] keyword: Keyword,
        #[case] shroud: bool,
        #[case] by: PlayerId,
        #[case] protected: bool,
    ) {
        let mut game = game_with(
            vec![creature("Wolf", 2, 2)],
            vec![keyworded("Shade", keyword)],
        );
        let shade = id(&game, "Shade");
        let what = refs(&game, &["Shade"]);
        let wanted = Rule::IgnoreHexproof {
            by: vec![ME],
            what,
            players: vec![FOE],
            shroud,
        };
        rule(&mut game, wanted);
        let source = id(&game, "Wolf");
        assert_eq!(
            crate::keywords::protects(&game, shade, source, by),
            protected
        );
    }

    #[test]
    fn a_rule_for_other_players_leaves_hexproof_alone() {
        let mut game = game_with(
            vec![creature("Wolf", 2, 2)],
            vec![keyworded("Shade", Keyword::Hexproof)],
        );
        let shade = id(&game, "Shade");
        let what = refs(&game, &["Shade"]);
        let wanted = Rule::IgnoreHexproof {
            by: vec![FOE],
            what,
            players: Vec::new(),
            shroud: false,
        };
        rule(&mut game, wanted);
        let source = id(&game, "Wolf");
        assert!(crate::keywords::protects(&game, shade, source, ME));
    }

    #[test]
    fn a_player_with_hexproof_is_targetable_through_a_rule() {
        let mut game = game();
        let wanted = Rule::PlayersHave {
            who: vec![FOE],
            keyword: Keyword::Hexproof,
        };
        rule(&mut game, wanted);
        assert!(crate::keywords::protects_player(&game, FOE, ME));
        let wanted = Rule::IgnoreHexproof {
            by: vec![ME],
            what: Vec::new(),
            players: vec![FOE],
            shroud: false,
        };
        rule(&mut game, wanted);
        assert!(!crate::keywords::protects_player(&game, FOE, ME));
    }

    #[rstest]
    #[case::cant_attack_alone(CombatQuirk::CantAttackAlone)]
    #[case::must_be_blocked(CombatQuirk::MustBeBlocked(Blocking::Every))]
    #[case::block_tapped(CombatQuirk::BlockWhileTapped)]
    fn the_data_of_a_rule_names_the_creatures_it_is_about(#[case] quirk: CombatQuirk) {
        let game = game();
        let wolf = game.object(id(&game, "Wolf")).unwrap().reference();
        let scope = Scope::announcing(&game, ME, wolf);
        let wanted = GameRule::Combat {
            who: Ref::This,
            quirk,
        };
        let lowered = lower_rule(&scope, &wanted).unwrap();
        assert_eq!(
            lowered,
            Rule::Combat {
                quirk,
                creatures: vec![wolf]
            }
        );
    }

    #[test]
    fn the_data_of_provoke_names_the_creatures_and_the_attacker() {
        let game = game();
        let wolf = game.object(id(&game, "Wolf")).unwrap().reference();
        let scope = Scope::announcing(&game, ME, wolf);
        let wanted = GameRule::MustBlockAttacker {
            who: Ref::This,
            attacker: Ref::This,
        };
        assert_eq!(
            lower_rule(&scope, &wanted).unwrap(),
            Rule::MustBlockAttacker {
                blockers: vec![wolf],
                attacker: wolf
            }
        );
    }
}
