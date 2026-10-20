//! Keyword abilities: one rules value per keyword, each in its own file (`docs/cards-dsl.md`, Keywords).
//!
//! A keyword task replaces its stub, adds its `Caps` entry and answers the hooks of [`KeywordRules`] the
//! engine asks about: blocking, attacking, damage, targeting, timing and destruction. The engine reads the
//! keywords an object has through [`keywords_of`] and the helpers below it, never from the rules of the card.

mod battle_cry;
mod cascade;
mod companion;
mod convoke;
mod cycling;
mod deathtouch;
mod defender;
mod delve;
mod disguise;
mod double_strike;
mod enchant;
mod equip;
mod evoke;
mod first_strike;
mod flash;
mod flashback;
mod flying;
mod fuse;
mod haste;
mod hexproof;
mod indestructible;
mod kicker;
mod lifelink;
mod madness;
mod menace;
mod morph;
mod multikicker;
mod overload;
mod partner;
mod partner_with;
mod prowess;
mod reach;
mod reconfigure;
mod storm;
mod suspend;
mod trample;
mod vigilance;
mod ward;

use std::fmt::Debug;

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::object::{GameObject, ObjectKind, PermanentState};
use crate::state::GameState;

/// How a keyword changes the combat damage its holder deals.
// Independent switches the keyword tasks set by name, not the states of one machine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DamageRules {
    /// Deals damage in the first-strike step.
    pub first_strike: bool,
    /// Deals damage in the first-strike step and in the regular one.
    pub double_strike: bool,
    /// Any damage it deals to a creature is lethal.
    pub deathtouch: bool,
    /// Assigns the rest of its damage to the player or planeswalker it attacks.
    pub trample: bool,
    /// Its controller gains that much life.
    pub lifelink: bool,
}

impl DamageRules {
    /// Both sets of rules at once: a creature with two keywords gets what each gives.
    #[must_use]
    pub const fn and(self, other: DamageRules) -> DamageRules {
        DamageRules {
            first_strike: self.first_strike || other.first_strike,
            double_strike: self.double_strike || other.double_strike,
            deathtouch: self.deathtouch || other.deathtouch,
            trample: self.trample || other.trample,
            lifelink: self.lifelink || other.lifelink,
        }
    }

    /// Whether the holder deals damage in the first-strike step.
    pub const fn strikes_first(self) -> bool {
        self.first_strike || self.double_strike
    }

    /// Whether the holder deals damage in the regular step: always, but for first strike alone (CR 510.4).
    pub const fn strikes_regularly(self) -> bool {
        self.double_strike || !self.first_strike
    }
}

/// What a keyword does to the rules machinery; every hook defaults to doing nothing.
pub trait KeywordRules: Debug + Sync {
    /// Whether `blocker` may not block `attacker`, which has the keyword.
    fn restricts_blocking(
        &self,
        _state: &GameState,
        _keyword: &Keyword,
        _attacker: ObjectId,
        _blocker: ObjectId,
    ) -> bool {
        false
    }

    /// Whether a holder of the keyword may block a creature whose `evasion` keyword restricts blocking.
    fn answers_evasion(&self, _keyword: &Keyword, _evasion: &Keyword) -> bool {
        false
    }

    /// The fewest creatures that may block the holder together, when any block it.
    fn min_blockers(&self, _keyword: &Keyword) -> usize {
        1
    }

    /// Whether the holder cannot be declared as an attacker.
    fn forbids_attacking(&self, _keyword: &Keyword) -> bool {
        false
    }

    /// Whether summoning sickness does not hold the holder back from attacking and from `{T}` abilities.
    fn ignores_summoning_sickness(&self, _keyword: &Keyword) -> bool {
        false
    }

    /// Whether declaring the holder as an attacker leaves it untapped.
    fn attacks_untapped(&self, _keyword: &Keyword) -> bool {
        false
    }

    /// Whether the holder is not destroyed by lethal damage, by deathtouch or by an effect that says destroy.
    fn is_indestructible(&self, _keyword: &Keyword) -> bool {
        false
    }

    /// Whether `holder`, which has the keyword, may not be targeted by a spell or ability of `source` that `controller` controls.
    fn restricts_targeting(
        &self,
        _state: &GameState,
        _keyword: &Keyword,
        _holder: ObjectId,
        _source: ObjectId,
        _controller: PlayerId,
    ) -> bool {
        false
    }

    /// Whether `holder`, a player who has the keyword, may not be targeted by a spell or ability that `controller` controls.
    fn restricts_player_targeting(
        &self,
        _keyword: &Keyword,
        _holder: PlayerId,
        _controller: PlayerId,
    ) -> bool {
        false
    }

    /// How the keyword changes the combat damage its holder deals.
    fn modifies_damage(&self, _keyword: &Keyword) -> DamageRules {
        DamageRules::default()
    }

    /// Whether the keyword lets its holder be cast whenever its controller could cast an instant.
    fn grants_timing(&self, _keyword: &Keyword) -> bool {
        false
    }

    /// The abilities the keyword stands for, written in the DSL like any card.
    fn adds_abilities(&self, _keyword: &Keyword) -> Vec<Ability> {
        Vec::new()
    }
}

/// The rules of `keyword`.
pub fn rules(keyword: &Keyword) -> &'static dyn KeywordRules {
    match keyword {
        Keyword::Flash => &flash::Flash,
        Keyword::Flying => &flying::Flying,
        Keyword::Reach => &reach::Reach,
        Keyword::FirstStrike => &first_strike::FirstStrike,
        Keyword::DoubleStrike => &double_strike::DoubleStrike,
        Keyword::Deathtouch => &deathtouch::Deathtouch,
        Keyword::Trample => &trample::Trample,
        Keyword::Lifelink => &lifelink::Lifelink,
        Keyword::Vigilance => &vigilance::Vigilance,
        Keyword::Haste => &haste::Haste,
        Keyword::Menace => &menace::Menace,
        Keyword::Defender => &defender::Defender,
        Keyword::Indestructible => &indestructible::Indestructible,
        Keyword::Hexproof => &hexproof::Hexproof,
        Keyword::Prowess => &prowess::Prowess,
        Keyword::Convoke => &convoke::Convoke,
        Keyword::Delve => &delve::Delve,
        Keyword::Cascade => &cascade::Cascade,
        Keyword::Storm => &storm::Storm,
        Keyword::Partner => &partner::Partner,
        Keyword::Companion => &companion::Companion,
        Keyword::BattleCry => &battle_cry::BattleCry,
        Keyword::Fuse => &fuse::Fuse,
        Keyword::Ward(_) => &ward::Ward,
        Keyword::Equip(_) => &equip::Equip,
        Keyword::Reconfigure(_) => &reconfigure::Reconfigure,
        Keyword::Enchant(_) => &enchant::Enchant,
        Keyword::Cycling(_) => &cycling::Cycling,
        Keyword::Kicker(_) => &kicker::Kicker,
        Keyword::Multikicker(_) => &multikicker::Multikicker,
        Keyword::Flashback(_) => &flashback::Flashback,
        Keyword::Madness(_) => &madness::Madness,
        Keyword::Morph(_) => &morph::Morph,
        Keyword::Disguise(_) => &disguise::Disguise,
        Keyword::Suspend { .. } => &suspend::Suspend,
        Keyword::Evoke(_) => &evoke::Evoke,
        Keyword::Overload(_) => &overload::Overload,
        Keyword::PartnerWith(_) => &partner_with::PartnerWith,
    }
}

/// The keywords printed on a card, in the order of its rules; only [`keywords_of`] starts from them.
fn printed(object: &GameObject) -> impl Iterator<Item = &Keyword> {
    object
        .card()
        .rules
        .iter()
        .filter_map(|ability| match &**ability {
            Ability::Keyword(keyword) => Some(keyword),
            _ => None,
        })
}

/// The keywords object `id` has now, in the order they are listed: the one reader of an object's keywords.
///
/// Today these are the printed ones. Layer 6 joins here (`docs/cards-dsl.md`, Triggers, replacement and
/// layers): the continuous effects that add a keyword append to the list and those that remove one take
/// it out, in timestamp order, so nothing else in the engine learns where a keyword came from. An ability on
/// the stack has no keywords of its own.
pub fn keywords_of(state: &GameState, id: ObjectId) -> Vec<&Keyword> {
    let Some(object) = state.object(id) else {
        return Vec::new();
    };
    if object.kind() == ObjectKind::Ability {
        return Vec::new();
    }
    printed(object).collect()
}

/// Whether object `id` has `keyword` now.
pub fn has_keyword(state: &GameState, id: ObjectId, keyword: &Keyword) -> bool {
    keywords_of(state, id).contains(&keyword)
}

/// What the keywords of `id` do to the combat damage it deals.
pub(crate) fn damage_rules(state: &GameState, id: ObjectId) -> DamageRules {
    keywords_of(state, id)
        .into_iter()
        .fold(DamageRules::default(), |all, keyword| {
            all.and(rules(keyword).modifies_damage(keyword))
        })
}

/// Whether a keyword stops `id` from being declared as an attacker.
pub(crate) fn forbids_attacking(state: &GameState, id: ObjectId) -> bool {
    keywords_of(state, id)
        .into_iter()
        .any(|keyword| rules(keyword).forbids_attacking(keyword))
}

/// Whether summoning sickness holds the permanent back from attacking and from `{T}` abilities now.
///
/// It has not been under its controller's control since their turn began and no keyword (haste) excuses it.
pub fn summoning_sick(state: &GameState, id: ObjectId) -> bool {
    state
        .object(id)
        .and_then(GameObject::permanent)
        .is_some_and(PermanentState::summoning_sick)
        && !keywords_of(state, id)
            .into_iter()
            .any(|keyword| rules(keyword).ignores_summoning_sickness(keyword))
}

/// Whether declaring `id` as an attacker leaves it untapped.
pub(crate) fn attacks_untapped(state: &GameState, id: ObjectId) -> bool {
    keywords_of(state, id)
        .into_iter()
        .any(|keyword| rules(keyword).attacks_untapped(keyword))
}

/// The fewest creatures that may block `id` together, when any do: 1 unless a keyword (menace) asks for more.
pub(crate) fn min_blockers(state: &GameState, id: ObjectId) -> usize {
    keywords_of(state, id)
        .into_iter()
        .map(|keyword| rules(keyword).min_blockers(keyword))
        .max()
        .unwrap_or(1)
        .max(1)
}

/// Whether a keyword keeps `id` from being destroyed by lethal damage, deathtouch or an effect that says destroy.
///
/// Zero toughness is not destruction (CR 704.5f): the state-based action for it ignores this. The
/// `Destroy` primitive asks it before it moves a permanent.
pub fn is_indestructible(state: &GameState, id: ObjectId) -> bool {
    keywords_of(state, id)
        .into_iter()
        .any(|keyword| rules(keyword).is_indestructible(keyword))
}

/// The keywords player `id` has now: the one reader of a player's keywords.
///
/// Today these are the ones their setup gives them; the continuous effects of layer 6 that name a player join
/// here, as they join [`keywords_of`].
pub fn player_keywords(state: &GameState, id: PlayerId) -> Vec<&Keyword> {
    state
        .player(id)
        .map_or_else(Vec::new, |player| player.keywords().iter().collect())
}

/// Whether a keyword of `holder` stops `controller`'s `source` from targeting it.
pub(crate) fn protects(
    state: &GameState,
    holder: ObjectId,
    source: ObjectId,
    controller: PlayerId,
) -> bool {
    keywords_of(state, holder).into_iter().any(|keyword| {
        rules(keyword).restricts_targeting(state, keyword, holder, source, controller)
    })
}

/// Whether a keyword of the player `holder` stops `controller`'s spell or ability from targeting them.
pub(crate) fn protects_player(state: &GameState, holder: PlayerId, controller: PlayerId) -> bool {
    player_keywords(state, holder)
        .into_iter()
        .any(|keyword| rules(keyword).restricts_player_targeting(keyword, holder, controller))
}

/// Whether a keyword of the attacker stops `blocker` from blocking it.
pub(crate) fn stops_block(state: &GameState, attacker: ObjectId, blocker: ObjectId) -> bool {
    keywords_of(state, attacker)
        .into_iter()
        .any(|keyword| rules(keyword).restricts_blocking(state, keyword, attacker, blocker))
}

/// Whether a keyword lets the card be cast whenever an instant could be.
pub(crate) fn grants_instant_timing(state: &GameState, id: ObjectId) -> bool {
    keywords_of(state, id)
        .into_iter()
        .any(|keyword| rules(keyword).grants_timing(keyword))
}

#[cfg(test)]
mod tests {
    //! Goal: every keyword the vocabulary declares has rules, the capabilities name exactly the keywords whose rules the engine runs, each keyword answers the hooks its row says and no others, and one function reads what an object has and one what a player has.
    use std::sync::Arc;

    use super::*;
    use crate::caps::caps;
    use crate::fixtures::{creature, state};
    use crate::zones::Zone;
    use mtg_cards::CardDefinition;
    use mtg_cards::cost::Cost;
    use mtg_cards::refs::Selector;
    use mtg_cards::validate::used_primitives;
    use mtg_core::card::CardType;
    use rstest::rstest;

    /// One value of every keyword, parameters included.
    fn every_keyword() -> Vec<Keyword> {
        let cost = || Cost::mana("{1}".parse().unwrap());
        vec![
            Keyword::Flash,
            Keyword::Flying,
            Keyword::Reach,
            Keyword::FirstStrike,
            Keyword::DoubleStrike,
            Keyword::Deathtouch,
            Keyword::Trample,
            Keyword::Lifelink,
            Keyword::Vigilance,
            Keyword::Haste,
            Keyword::Menace,
            Keyword::Defender,
            Keyword::Indestructible,
            Keyword::Hexproof,
            Keyword::Prowess,
            Keyword::Convoke,
            Keyword::Delve,
            Keyword::Cascade,
            Keyword::Storm,
            Keyword::Partner,
            Keyword::Companion,
            Keyword::BattleCry,
            Keyword::Fuse,
            Keyword::Ward(cost()),
            Keyword::Equip(cost()),
            Keyword::Reconfigure(cost()),
            Keyword::Enchant(Selector::default()),
            Keyword::Cycling(cost()),
            Keyword::Kicker(cost()),
            Keyword::Multikicker(cost()),
            Keyword::Flashback(cost()),
            Keyword::Madness(cost()),
            Keyword::Morph(cost()),
            Keyword::Disguise(cost()),
            Keyword::Suspend {
                time: 3,
                cost: cost(),
            },
            Keyword::Evoke(cost()),
            Keyword::Overload(cost()),
            Keyword::PartnerWith("Pir, Imaginative Rascal".into()),
        ]
    }

    /// The keywords whose rules the engine runs, named as the capabilities name them.
    const RUN: [&str; 13] = [
        "Keyword.Flying",
        "Keyword.Reach",
        "Keyword.FirstStrike",
        "Keyword.DoubleStrike",
        "Keyword.Deathtouch",
        "Keyword.Trample",
        "Keyword.Lifelink",
        "Keyword.Vigilance",
        "Keyword.Haste",
        "Keyword.Menace",
        "Keyword.Defender",
        "Keyword.Indestructible",
        "Keyword.Hexproof",
    ];

    /// What a keyword answers to each hook that does not need a game.
    // One flag per hook, named after it, so a row of the table reads as the hooks it sets.
    #[allow(clippy::struct_excessive_bools)]
    #[derive(Debug, PartialEq, Eq)]
    struct Hooks {
        answers_flying: bool,
        min_blockers: usize,
        forbids_attacking: bool,
        ignores_sickness: bool,
        attacks_untapped: bool,
        indestructible: bool,
        shields_player: bool,
        damage: DamageRules,
        timing: bool,
        abilities: usize,
    }

    impl Hooks {
        /// A keyword that changes nothing.
        fn none() -> Hooks {
            Hooks {
                answers_flying: false,
                min_blockers: 1,
                forbids_attacking: false,
                ignores_sickness: false,
                attacks_untapped: false,
                indestructible: false,
                shields_player: false,
                damage: DamageRules::default(),
                timing: false,
                abilities: 0,
            }
        }

        fn of(keyword: &Keyword) -> Hooks {
            let rules = rules(keyword);
            Hooks {
                answers_flying: rules.answers_evasion(keyword, &Keyword::Flying),
                min_blockers: rules.min_blockers(keyword),
                forbids_attacking: rules.forbids_attacking(keyword),
                ignores_sickness: rules.ignores_summoning_sickness(keyword),
                attacks_untapped: rules.attacks_untapped(keyword),
                indestructible: rules.is_indestructible(keyword),
                shields_player: rules.restricts_player_targeting(keyword, PlayerId(0), PlayerId(1)),
                damage: rules.modifies_damage(keyword),
                timing: rules.grants_timing(keyword),
                abilities: rules.adds_abilities(keyword).len(),
            }
        }
    }

    fn damage(f: impl FnOnce(&mut DamageRules)) -> DamageRules {
        let mut rules = DamageRules::default();
        f(&mut rules);
        rules
    }

    #[test]
    fn the_list_has_every_keyword_the_vocabulary_names() {
        let names: Vec<&str> = every_keyword().iter().map(Keyword::name).collect();
        assert_eq!(names, Keyword::NAMES);
    }

    #[rstest]
    #[case::flying(Keyword::Flying, Hooks { answers_flying: true, ..Hooks::none() })]
    #[case::reach(Keyword::Reach, Hooks { answers_flying: true, ..Hooks::none() })]
    #[case::menace(Keyword::Menace, Hooks { min_blockers: 2, ..Hooks::none() })]
    #[case::defender(Keyword::Defender, Hooks { forbids_attacking: true, ..Hooks::none() })]
    #[case::haste(Keyword::Haste, Hooks { ignores_sickness: true, ..Hooks::none() })]
    #[case::vigilance(Keyword::Vigilance, Hooks { attacks_untapped: true, ..Hooks::none() })]
    #[case::indestructible(Keyword::Indestructible, Hooks { indestructible: true, ..Hooks::none() })]
    #[case::hexproof(Keyword::Hexproof, Hooks { shields_player: true, ..Hooks::none() })]
    #[case::first_strike(Keyword::FirstStrike, Hooks { damage: damage(|d| d.first_strike = true), ..Hooks::none() })]
    #[case::double_strike(Keyword::DoubleStrike, Hooks { damage: damage(|d| d.double_strike = true), ..Hooks::none() })]
    #[case::deathtouch(Keyword::Deathtouch, Hooks { damage: damage(|d| d.deathtouch = true), ..Hooks::none() })]
    #[case::trample(Keyword::Trample, Hooks { damage: damage(|d| d.trample = true), ..Hooks::none() })]
    #[case::lifelink(Keyword::Lifelink, Hooks { damage: damage(|d| d.lifelink = true), ..Hooks::none() })]
    fn a_keyword_answers_the_hooks_of_its_row(#[case] keyword: Keyword, #[case] row: Hooks) {
        assert_eq!(Hooks::of(&keyword), row);
    }

    #[test]
    fn every_other_keyword_changes_nothing() {
        for keyword in every_keyword() {
            if RUN.contains(&format!("Keyword.{}", name_of_variant(&keyword)).as_str()) {
                continue;
            }
            assert_eq!(Hooks::of(&keyword), Hooks::none(), "{keyword:?}");
        }
    }

    /// The variant name a capability uses (`FirstStrike`), which is the debug text up to its parameters.
    fn name_of_variant(keyword: &Keyword) -> String {
        let text = format!("{keyword:?}");
        text.split(['(', ' ', '{']).next().unwrap_or("").to_owned()
    }

    #[test]
    fn the_capabilities_name_exactly_the_keywords_whose_rules_run() {
        let caps = caps();
        let claimed: Vec<String> = every_keyword()
            .into_iter()
            .flat_map(|keyword| used_primitives(&Ability::Keyword(keyword)))
            .filter(|name| name.starts_with("Keyword.") && caps.runs(name))
            .collect();
        let mut want: Vec<String> = RUN.iter().map(|name| (*name).to_owned()).collect();
        want.sort();
        let mut claimed = claimed;
        claimed.sort();
        assert_eq!(claimed, want);
    }

    fn holder(keywords: &[Keyword]) -> Arc<CardDefinition> {
        let mut card = CardDefinition::new("Holder")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2);
        for keyword in keywords {
            card = card.with_rules(Ability::Keyword(keyword.clone()));
        }
        Arc::new(card)
    }

    #[test]
    fn one_function_lists_what_a_card_prints_in_the_order_of_its_rules() {
        let game = state(
            vec![vec![], vec![]],
            vec![
                vec![
                    holder(&[Keyword::Haste, Keyword::Flying, Keyword::Trample]),
                    creature("Plain", 1, 1),
                ],
                vec![],
            ],
        );
        let [held, plain] = [0, 1].map(|at| game.zone(Zone::Battlefield)[at]);
        assert_eq!(
            keywords_of(&game, held),
            [&Keyword::Haste, &Keyword::Flying, &Keyword::Trample]
        );
        assert!(has_keyword(&game, held, &Keyword::Flying));
        assert!(!has_keyword(&game, held, &Keyword::Reach));
        assert_eq!(keywords_of(&game, plain).len(), 0);
        assert_eq!(keywords_of(&game, ObjectId(u64::MAX)).len(), 0);
    }

    #[test]
    fn one_function_lists_the_keywords_a_player_has() {
        let mut setup = crate::fixtures::setup(vec![vec![], vec![]], vec![vec![], vec![]]);
        setup.players[1].keywords = vec![Keyword::Hexproof];
        let game = GameState::from_setup(&setup).unwrap();
        assert_eq!(player_keywords(&game, PlayerId(1)), [&Keyword::Hexproof]);
        assert_eq!(player_keywords(&game, PlayerId(0)).len(), 0);
        assert_eq!(player_keywords(&game, PlayerId(9)).len(), 0);
        assert!(protects_player(&game, PlayerId(1), PlayerId(0)));
        assert!(!protects_player(&game, PlayerId(1), PlayerId(1)));
        assert!(!protects_player(&game, PlayerId(0), PlayerId(1)));
    }

    #[test]
    fn the_helpers_combine_the_keywords_of_one_object() {
        let game = state(
            vec![vec![], vec![]],
            vec![
                vec![
                    holder(&[Keyword::FirstStrike, Keyword::Lifelink, Keyword::Menace]),
                    holder(&[Keyword::Menace, Keyword::Defender, Keyword::Indestructible]),
                    creature("Plain", 1, 1),
                ],
                vec![],
            ],
        );
        let [mixed, wall, plain] = [0, 1, 2].map(|at| game.zone(Zone::Battlefield)[at]);
        assert_eq!(
            damage_rules(&game, mixed),
            damage(|d| {
                d.first_strike = true;
                d.lifelink = true;
            })
        );
        assert_eq!(damage_rules(&game, plain), DamageRules::default());
        assert_eq!(
            (min_blockers(&game, mixed), min_blockers(&game, plain)),
            (2, 1)
        );
        assert!(forbids_attacking(&game, wall) && !forbids_attacking(&game, mixed));
        assert!(is_indestructible(&game, wall) && !is_indestructible(&game, mixed));
        assert!(!attacks_untapped(&game, wall));
    }

    #[rstest]
    #[case::first_strike_alone(damage(|d| d.first_strike = true), true, false)]
    #[case::double_strike(damage(|d| d.double_strike = true), true, true)]
    #[case::both(damage(|d| { d.first_strike = true; d.double_strike = true; }), true, true)]
    #[case::neither(DamageRules::default(), false, true)]
    #[case::deathtouch_only(damage(|d| d.deathtouch = true), false, true)]
    fn a_creature_strikes_in_the_steps_its_keywords_allow(
        #[case] rules: DamageRules,
        #[case] first: bool,
        #[case] regular: bool,
    ) {
        assert_eq!(
            (rules.strikes_first(), rules.strikes_regularly()),
            (first, regular)
        );
    }
}
