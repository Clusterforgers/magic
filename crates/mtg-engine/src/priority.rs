//! Priority: who may act, what they may do, and when a round of passes is complete.

use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_cards::ability::{Ability, Limit, LoyaltyCost, Timing};
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::{ManaCost, ManaSymbol};

use crate::abilities::AbilityRef;
use crate::ability_reader::{abilities_of, provided_here, working_here};
use crate::announce::some_x_has_targets;
use crate::answers::Outcome;
use crate::characteristics::{characteristics, controller_of_object};
use crate::choose::announceable;
use crate::decision::{FaceOption, PriorityAction, PriorityOptions};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::faces;
use crate::keywords;
use crate::loyalty;
use crate::mana::{ManaOption, ManaPool, can_afford};
use crate::mana_ability;
use crate::manual;
use crate::object::GameObject;
use crate::player::Player;
use crate::price::{Leaves, Price};
use crate::resolve::{Scope, zone_kind};
use crate::spend::Spend;
use crate::state::GameState;
use crate::ways;
use crate::zones::Zone;

/// Who holds priority and who has passed since the last action.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PriorityState {
    pub(crate) holder: PlayerId,
    pub(crate) passed: BTreeSet<PlayerId>,
}

impl PriorityState {
    /// The player who may act now.
    pub const fn holder(&self) -> PlayerId {
        self.holder
    }

    /// Players who passed in succession.
    pub const fn passed(&self) -> &BTreeSet<PlayerId> {
        &self.passed
    }
}

/// True once every living player has passed in succession.
pub(crate) fn all_passed(state: &GameState) -> bool {
    state
        .priority()
        .is_some_and(|p| state.alive_players().all(|a| p.passed.contains(&a)))
}

/// The mana abilities `player` can activate right now, to pay for something or at priority (`mana_ability.rs`).
pub(crate) fn mana_options(state: &GameState, player: PlayerId) -> Vec<ManaOption> {
    mana_ability::options(state, player)
}

/// Whether `player` could cast a sorcery now: their own turn, a main step and an empty stack (CR 307.1).
pub(crate) fn sorcery_timing(state: &GameState, player: PlayerId) -> bool {
    state.turn().active() == player && state.turn().step().is_main() && state.stack().is_empty()
}

fn hand_land(state: &GameState, id: ObjectId) -> bool {
    state
        .object(id)
        .is_some_and(|o| o.base_types().contains(CardType::Land))
}

/// Whether the ability `rules`, which `address` names among the abilities of the permanent `object`, could be
/// activated by `player` now.
///
/// The engine pays `{T}`, mana, the loss of the source and the parts of `Price::parts` for an ability and nothing else;
/// `Caps` keeps abilities with other costs out of the game. The mana must be affordable with the floating mana and the other
/// permanents the player taps.
fn can_activate(
    state: &GameState,
    player: PlayerId,
    object: &GameObject,
    address: AbilityRef,
    rules: &Ability,
) -> bool {
    // A loyalty ability costs counters, not a cost the price below knows (CR 606).
    if let Ability::Loyalty { cost, .. } = rules {
        // A `-X` removes X counters, at most those the planeswalker holds (CR 606.6).
        let counters = object
            .permanent()
            .map_or(0, |permanent| permanent.counters_of(&CounterKind::Loyalty));
        return loyalty::can_activate(state, player, object, *cost)
            && announceable(state, player, object.id(), rules)
            && some_x_has_targets(state, player, object.id(), rules, |x| {
                x == 0 || (*cost == LoyaltyCost::MinusX && x <= counters)
            });
    }
    let Ability::Activated {
        cost,
        timing,
        condition,
        limit,
        from,
        ..
    } = rules
    else {
        return false;
    };
    // An ability is activated from the zone it works from, and a cost that moves its source needs it in the right one.
    if *from != zone_kind(object.zone()) {
        return false;
    }
    if *limit == Some(Limit::OncePerTurn)
        && state.limit_spent(object.reference(), address.source, address.ability)
    {
        return false;
    }
    let Ok(price) = Price::announced(cost) else {
        return false;
    };
    if rules.is_mana_ability() {
        return false;
    }
    let leaves_from_here = match price.leaves {
        Leaves::Stays | Leaves::Exiled => true,
        Leaves::Sacrificed => object.zone() == Zone::Battlefield,
        Leaves::Discarded => matches!(object.zone(), Zone::Hand(_)),
    };
    if !leaves_from_here {
        return false;
    }
    // Life, counters and the objects the other parts take must be there to pay (CR 118.3).
    let Some(claimed) = price.claimed(state, player, object.id()) else {
        return false;
    };
    if *timing == Timing::Sorcery && !sorcery_timing(state, player) {
        return false;
    }
    // "Activate only if ..." is judged as the ability is activated (CR 602.5b).
    if let Some(condition) = condition
        && !Scope::announcing(state, player, object.reference())
            .holds(condition)
            .unwrap_or(false)
    {
        return false;
    }
    if price.tap {
        let Some(permanent) = object.permanent() else {
            return false;
        };
        // Only creatures are subject to summoning sickness for {T} abilities (CR 302.6).
        let sick = keywords::summoning_sick(state, object.id())
            && characteristics(state, object.id()).is_some_and(|c| c.is_creature());
        if permanent.tapped() || sick {
            return false;
        }
    }
    if let Some(mana) = &price.mana {
        let Some(seat) = state.player(player) else {
            return false;
        };
        let mut sources = price.sources(state, player, object.id());
        // What the other parts of the cost take is not there to tap for mana, and the life they pay is not there to spend.
        sources.retain(|option| !claimed.contains(&option.source.object));
        let life = seat
            .life()
            .saturating_sub(i32::try_from(price.life()).unwrap_or(i32::MAX));
        let spend = Spend::for_ability(state, object.id(), price.x > 0);
        if !can_afford(mana, seat.mana(), &sources, life, &spend) {
            return false;
        }
    }
    announceable(state, player, object.id(), rules)
        && some_x_has_targets(state, player, object.id(), rules, |x| {
            x == 0 || (price.x > 0 && can_pay_x(state, player, object, &price, &claimed, x))
        })
}

/// Whether `player` can pay the mana of `price` with `x` more generic mana for each `{X}`, as `can_activate` judges the
/// mana: from what floats and the permanents they can tap other than those the other parts of the cost take.
fn can_pay_x(
    state: &GameState,
    player: PlayerId,
    object: &GameObject,
    price: &Price,
    claimed: &[ObjectId],
    x: u32,
) -> bool {
    let Some(seat) = state.player(player) else {
        return false;
    };
    let mana = price
        .mana
        .clone()
        .unwrap_or(ManaCost::ZERO)
        .with(ManaSymbol::Generic(x.saturating_mul(price.x)));
    let mut sources = price.sources(state, player, object.id());
    sources.retain(|option| !claimed.contains(&option.source.object));
    let life = seat
        .life()
        .saturating_sub(i32::try_from(price.life()).unwrap_or(i32::MAX));
    let spend = Spend::for_ability(state, object.id(), price.x > 0);
    can_afford(&mana, seat.mana(), &sources, life, &spend)
}

/// The cards outside the battlefield whose abilities `player` may use: the owner of a card in a hand, a graveyard or the
/// command zone, or in exile, activates its abilities (CR 602.2).
fn own_cards_elsewhere(state: &GameState, player: PlayerId) -> impl Iterator<Item = &GameObject> {
    [
        Zone::Hand(player),
        Zone::Graveyard(player),
        Zone::Command(player),
        Zone::Exile,
    ]
    .into_iter()
    .flat_map(|zone| state.zone(zone).iter())
    .filter_map(|id| state.object(*id))
    .filter(move |object| object.owner() == player)
}

/// Activated abilities of `player`'s permanents, and of the cards they own in a hand, a graveyard, exile or the command
/// zone, that are not mana abilities and could be used now.
pub(crate) fn other_abilities(state: &GameState, player: PlayerId) -> Vec<AbilityRef> {
    let mut found = Vec::new();
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(id) else {
            continue;
        };
        if controller_of_object(state, object) != player {
            continue;
        }
        for (address, rules) in abilities_of(state, id) {
            if can_activate(state, player, object, address, &rules) {
                found.push(address);
            }
        }
    }
    if state.possible.elsewhere {
        for object in own_cards_elsewhere(state, player) {
            let printed = working_here(object).map(|(address, rules)| (address, Arc::clone(rules)));
            for (address, rules) in printed.chain(provided_here(object)) {
                if can_activate(state, player, object, address, &rules) {
                    found.push(address);
                }
            }
        }
    }
    found
}

/// Whether `player` can do anything worth stopping for: play a land, cast a spell or use an ability that is not a mana ability.
///
/// `castable` already holds only the spells the floating mana and the untapped sources can pay for. Mana
/// abilities alone never count: tapping a land with nothing to spend the mana on is not an action.
pub(crate) fn can_act(
    playable_lands: &[ObjectId],
    castable: &[ObjectId],
    abilities: &[AbilityRef],
    special_actions: &[ObjectId],
) -> bool {
    !playable_lands.is_empty()
        || !castable.is_empty()
        || !abilities.is_empty()
        || !special_actions.is_empty()
}

/// The faces of the cards with several faces that can be cast now (`castable`) or played as a land, each with its cost.
fn face_options(
    state: &GameState,
    player: PlayerId,
    castable: &[ObjectId],
    may_play_land: bool,
    pool: &ManaPool,
    sources: &[ManaOption],
    life: i32,
) -> Vec<FaceOption> {
    let mut found = Vec::new();
    let hand = state.zone(Zone::Hand(player));
    for &card in hand.iter().chain(castable) {
        if !faces::is_multi_face(state, card) || found.iter().any(|o: &FaceOption| o.card == card) {
            continue;
        }
        let listed = |face: Option<u8>, land: bool| {
            let (name, cost) = faces::describe(state, card, face);
            FaceOption {
                card,
                face: face.unwrap_or(0),
                name,
                cost: if land { None } else { cost },
                land,
            }
        };
        if may_play_land && hand.contains(&card) {
            found.extend(
                faces::land_faces(state, card)
                    .into_iter()
                    .map(|face| listed(face, true)),
            );
        }
        if castable.contains(&card) {
            found.extend(
                faces::castable_faces(state, player, card, pool, sources, life)
                    .into_iter()
                    .map(|face| listed(face, false)),
            );
        }
    }
    found
}

/// Everything `player` may do while holding priority.
pub(crate) fn priority_options(state: &GameState, player: PlayerId) -> PriorityOptions {
    let hand = state.zone(Zone::Hand(player));
    let all_mana = mana_options(state, player);
    let pool = state
        .player(player)
        .map(|p| p.mana().clone())
        .unwrap_or_default();
    let life = state.player(player).map_or(0, Player::life);
    let may_play_land = sorcery_timing(state, player)
        && state.turn().lands_played()
            < state
                .ruleset()
                .land_plays_per_turn()
                .saturating_add(crate::rules::extra_land_plays(state, player));
    let playable_lands: Vec<ObjectId> = if may_play_land {
        hand.iter()
            .copied()
            .filter(|id| hand_land(state, *id))
            .chain(ways::lands_elsewhere(state, player))
            .collect()
    } else {
        Vec::new()
    };
    // CR 601.2c: a spell that needs a target is cast only when it has a legal one; CR 700.2a: a modal spell only when
    // enough of its modes can be chosen. A card is offered when some way to cast it can be used now (`ways`).
    let castable: Vec<ObjectId> = hand
        .iter()
        .copied()
        .chain(ways::elsewhere(state, player))
        .filter(|id| ways::castable_now(state, player, *id, &pool, &all_mana, life))
        .collect();
    // A special action: no stack, so no target or modes are needed (CR 116.2f).
    let suspendable: Vec<ObjectId> = hand
        .iter()
        .copied()
        .filter(|id| ways::can_suspend(state, player, *id, &pool, &all_mana, life))
        .collect();
    let foretellable: Vec<ObjectId> = hand
        .iter()
        .copied()
        .filter(|id| ways::can_foretell(state, player, *id, &pool, &all_mana, life))
        .collect();
    let abilities = other_abilities(state, player);
    let faces = face_options(
        state,
        player,
        &castable,
        may_play_land,
        &pool,
        &all_mana,
        life,
    );
    // A mana ability that costs mana is used inside a payment, from the mana the payment's other sources make.
    let mana_abilities = all_mana
        .into_iter()
        .filter(|option| !option.is_costly() && option.takes.is_empty())
        .collect();
    PriorityOptions {
        can_act: can_act(
            &playable_lands,
            &castable,
            &abilities,
            &[suspendable.as_slice(), foretellable.as_slice()].concat(),
        ) || !faces.is_empty(),
        playable_lands,
        castable,
        mana_abilities,
        abilities,
        suspendable,
        foretellable,
        faces,
        can_pass: true,
    }
}

/// Validates a priority action and turns it into events (or a cast in progress).
pub(crate) fn answer(
    state: &GameState,
    player: PlayerId,
    options: &PriorityOptions,
    action: PriorityAction,
) -> Result<Outcome, EngineError> {
    match action {
        PriorityAction::Pass => Ok(Outcome::Events(vec![Event::PriorityPassed { player }])),
        PriorityAction::PlayLand(object) => {
            if !options.playable_lands.contains(&object) {
                return Err(IllegalAnswer::NotOffered("land").into());
            }
            Ok(Outcome::Events(vec![Event::LandPlayed {
                player,
                object,
                face: None,
            }]))
        }
        PriorityAction::PlayLandFace(object, face) => {
            let offered = options
                .faces
                .iter()
                .any(|option| option.land && option.card == object && option.face == face);
            if !offered {
                return Err(IllegalAnswer::NotOffered("land").into());
            }
            // The front face of an adventure or modal double-faced card is the card itself.
            Ok(Outcome::Events(vec![Event::LandPlayed {
                player,
                object,
                face: (face != 0).then_some(face),
            }]))
        }
        PriorityAction::CastSpell(object) => {
            if !options.castable.contains(&object) {
                return Err(IllegalAnswer::NotOffered("spell").into());
            }
            Ok(Outcome::BeginCast(object))
        }
        PriorityAction::Activate(ability) => {
            if !options.abilities.contains(&ability) {
                return Err(IllegalAnswer::NotOffered("ability").into());
            }
            Ok(Outcome::BeginActivation(ability))
        }
        PriorityAction::Suspend(object) => {
            if !options.suspendable.contains(&object) {
                return Err(IllegalAnswer::NotOffered("suspend").into());
            }
            Ok(Outcome::BeginSuspend(object))
        }
        PriorityAction::Foretell(object) => {
            if !options.foretellable.contains(&object) {
                return Err(IllegalAnswer::NotOffered("foretell").into());
            }
            Ok(Outcome::BeginForetell(object))
        }
        PriorityAction::Manual(action) => manual::events(state, player, &action)
            .map(Outcome::Events)
            .map_err(|refusal| IllegalAnswer::Manual(refusal).into()),
        PriorityAction::ActivateMana(source) => {
            let option = options
                .mana_abilities
                .iter()
                .find(|o| o.source == source)
                .ok_or(IllegalAnswer::NotOffered("mana ability"))?;
            Ok(Outcome::Events(mana_ability::activation(
                state,
                player,
                option,
                &[],
            )?))
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: an activated ability is offered when its whole price can be paid now: a `{T}` needs an untapped source, mana needs floating mana or untapped mana sources (the source of an ability that taps it is none of them), an `{X}` is offered when X = 0 can be paid, and a part the engine does not pay is never offered.
    use std::sync::Arc;

    use mtg_cards::AbilityDef;
    use mtg_cards::CardDefinition;
    use mtg_cards::definition::ManaAbility;
    use mtg_core::card::CardType;
    use mtg_core::color::Color;
    use mtg_core::mana::ManaType;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::state;
    use crate::fold::apply_event;

    fn gadget(cost: &str, makes_mana: bool) -> Arc<CardDefinition> {
        let rules = format!(r#"{{"Activated":{{"cost":{cost},"effects":[{{"Draw":{{}}}}]}}}}"#);
        let card = CardDefinition::new("Gadget")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str(&rules).unwrap());
        Arc::new(if makes_mana {
            card.with_ability(AbilityDef::Mana(ManaAbility {
                produces: ManaType::Colorless,
            }))
        } else {
            card
        })
    }

    fn forest() -> Arc<CardDefinition> {
        Arc::new(CardDefinition::basic_land(Some(Color::Green)))
    }

    fn offered(game: &GameState) -> bool {
        !other_abilities(game, PlayerId(0)).is_empty()
    }

    #[rstest]
    #[case::free("[]", 0, true)]
    #[case::a_tap("[\"Tap\"]", 0, true)]
    #[case::mana_without_a_land("\"{1}\"", 0, false)]
    #[case::mana_with_a_land("\"{1}\"", 1, true)]
    #[case::two_mana_with_one_land("\"{2}\"", 1, false)]
    #[case::two_mana_with_two_lands("\"{2}\"", 2, true)]
    #[case::a_colour_the_lands_do_not_make("\"{R}\"", 3, false)]
    #[case::mana_and_a_tap("[{\"Mana\":\"{1}\"},\"Tap\"]", 1, true)]
    #[case::mana_and_a_tap_without_a_land("[{\"Mana\":\"{1}\"},\"Tap\"]", 0, false)]
    #[case::x("[{\"Mana\":\"{X}\"}]", 3, true)]
    #[case::x_of_nothing("[{\"Mana\":\"{X}\"}]", 0, true)]
    #[case::x_with_a_colour_that_cannot_be_paid("[{\"Mana\":\"{X}{R}\"}]", 3, false)]
    #[case::x_and_a_tap_without_a_source("[{\"Mana\":\"{X}\"},\"Tap\"]", 0, true)]
    #[case::life_as_well("[{\"Mana\":\"{1}\"},{\"PayLife\":1}]", 3, true)]
    #[case::more_life_than_the_player_has("[{\"Mana\":\"{1}\"},{\"PayLife\":21}]", 3, false)]
    fn an_ability_is_offered_when_its_price_can_be_paid(
        #[case] cost: &str,
        #[case] lands: usize,
        #[case] expected: bool,
    ) {
        let mut cards = vec![gadget(cost, false)];
        cards.extend(vec![forest(); lands]);
        let game = state(vec![vec![], vec![]], vec![cards, vec![]]);
        assert_eq!(offered(&game), expected);
    }

    #[rstest]
    #[case::taps_itself_for_its_own_ability("\"{1}\"", true)]
    #[case::cannot_tap_twice("[{\"Mana\":\"{1}\"},\"Tap\"]", false)]
    fn the_source_pays_for_its_ability_only_when_it_is_not_tapped_for_the_cost(
        #[case] cost: &str,
        #[case] expected: bool,
    ) {
        let game = state(vec![vec![], vec![]], vec![vec![gadget(cost, true)], vec![]]);
        assert_eq!(offered(&game), expected);
    }

    #[test]
    fn floating_mana_pays_and_a_tapped_source_cannot_tap() {
        let mut game = state(
            vec![vec![], vec![]],
            vec![vec![gadget("[{\"Mana\":\"{1}\"},\"Tap\"]", false)], vec![]],
        );
        assert!(!offered(&game));
        apply_event(
            &mut game,
            &Event::ManaAdded {
                player: PlayerId(0),
                mana: ManaType::Red,
                source: None,
                only: Vec::new(),
            },
        )
        .unwrap();
        assert!(offered(&game));
        let gadget = game.zone(Zone::Battlefield)[0];
        apply_event(&mut game, &Event::PermanentTapped { object: gadget }).unwrap();
        assert!(!offered(&game));
    }

    #[test]
    fn the_other_seat_is_not_offered_the_ability() {
        let game = state(
            vec![vec![], vec![]],
            vec![vec![gadget("[]", false)], vec![forest()]],
        );
        assert_eq!(
            other_abilities(&game, PlayerId(1)),
            Vec::<AbilityRef>::new()
        );
    }
}
