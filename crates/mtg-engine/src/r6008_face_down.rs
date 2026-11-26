//! Face-down permanents an effect sets up (CR 708): characteristics an effect lists and a name it notes on the
//! permanent (`Destination::R6008FaceDownAs`, `FaceDown::R6008Listed`), and a creature cast face down by an effect that is
//! turned face up before it deals or is dealt damage or becomes tapped (`CastWay::R6008FaceDown`, `FaceDown::R6008Masked`).

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_core::mana::{ManaCost, ManaType};
use serde::{Deserialize, Serialize};

use crate::characteristics::controller_of_object;
use crate::damage::DamagePass;
use crate::combat::DamageTarget;
use crate::events::Event;
use crate::object::{FaceDown, R6008Listing};
use crate::state::GameState;
use crate::zones::Zone;

/// A card put onto the battlefield face down took the characteristics the effect listed, and the name it noted (CR 708.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct R6008Listed {
    /// The permanent.
    pub object: ObjectId,
    /// The characteristics listed, which are its copiable values.
    #[cfg_attr(feature = "ts", ts(skip))]
    pub definition: Arc<CardDefinition>,
    /// The name noted on it: public, as the card was in a graveyard before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub noted: Option<String>,
}

/// The characteristics a `Destination::R6008FaceDownAs` lists: a nameless, colourless creature of that power and toughness
/// with those abilities and nothing else.
pub(crate) fn listed_definition(
    power: i32,
    toughness: i32,
    rules: &[mtg_cards::ability::Ability],
) -> CardDefinition {
    let mut shown = CardDefinition {
        rules: rules.iter().cloned().map(Arc::new).collect(),
        ..CardDefinition::new("")
    };
    shown.types.insert(CardType::Creature);
    shown.power = Some(power);
    shown.toughness = Some(toughness);
    shown
}

/// Gives the face-down permanent the listing; a card that is not on the battlefield face down (a replacement moved it
/// elsewhere) is passed over.
pub(crate) fn fold_listed(state: &mut GameState, listed: &R6008Listed) {
    let Some(object) = state.objects.get_mut(&listed.object) else {
        return;
    };
    if object.zone != Zone::Battlefield || object.face_down.is_none() {
        return;
    }
    object.face_down = Some(FaceDown::R6008Listed);
    object.r6008_listed = Some(Arc::new(R6008Listing::new(
        Arc::clone(&listed.definition),
        listed.noted.clone(),
    )));
}

/// The definition of the card with the name noted on `source`: the card behind the permanent when it has that name, else
/// one the game can conjure by it.
pub(crate) fn noted_card(state: &GameState, source: ObjectId) -> Option<Arc<CardDefinition>> {
    let object = state.object(source)?;
    let noted = object.r6008_listed()?.noted()?;
    if object.whole_card().name == noted {
        return Some(Arc::new(object.whole_card().clone()));
    }
    state.conjurable.get(noted).cloned()
}

/// Whether a face-down instant or sorcery card is turned face up: it is revealed and stays face down (CR 708, the
/// rulings of Magar of the Magic Strings).
pub(crate) fn stays_face_down(state: &GameState, id: ObjectId) -> bool {
    state.object(id).is_some_and(|object| {
        let types = &object.whole_card().types;
        types.contains(&CardType::Instant) || types.contains(&CardType::Sorcery)
    })
}

/// Whether some or all of `spent` could pay `cost`: each symbol paid by one mana, the amount and the types counting; an
/// {X} in the cost counts as 0, and life pays no Phyrexian symbol.
pub(crate) fn paid_by(cost: &ManaCost, spent: &[ManaType]) -> bool {
    let mut pool = crate::mana::ManaPool::default();
    for mana in spent {
        pool.add(*mana);
    }
    crate::mana::can_afford(cost, &pool, &[], 0, &crate::spend::Spend::other())
}

/// Whether the object lies face down as a masked creature that has not been turned face up.
fn masked(state: &GameState, id: ObjectId) -> bool {
    state.object(id).is_some_and(|object| {
        object.zone() == Zone::Battlefield && object.face_down() == Some(FaceDown::R6008Masked)
    })
}

/// The masked permanents an event would tap, damage or have deal damage.
fn masked_in(state: &GameState, event: &Event) -> Vec<ObjectId> {
    let ids: Vec<ObjectId> = match event {
        Event::PermanentTapped { object } | Event::Regenerated { object } => vec![*object],
        Event::DamageDealt { source, target, .. } => {
            let mut ids = vec![*source];
            if let DamageTarget::Object(target) = target {
                ids.push(*target);
            }
            ids
        }
        _ => Vec::new(),
    };
    ids.into_iter().filter(|id| masked(state, *id)).collect()
}

/// The turn up of a masked permanent by its controller.
fn turned_up(state: &GameState, id: ObjectId) -> Option<Event> {
    let object = state.object(id)?;
    Some(Event::TurnedFaceUp {
        object: id,
        player: controller_of_object(state, object),
        x: 0,
    })
}

/// "If the creature ... would ... deal damage, be dealt damage, or become tapped, instead it's turned face up and deals
/// damage, is dealt damage, or becomes tapped": each masked permanent an event of the batch taps or damages, or that deals
/// damage, is turned face up just before the first such event (CR 614.1a).
pub(crate) fn r6008_turned_up_first(state: &GameState, proposed: Vec<Event>) -> Vec<Event> {
    if !proposed.iter().any(|event| !masked_in(state, event).is_empty()) {
        return proposed;
    }
    // A batch the step runs again holds the turns up it added before.
    let mut done: Vec<ObjectId> = proposed
        .iter()
        .filter_map(|event| match event {
            Event::TurnedFaceUp { object, .. } => Some(*object),
            _ => None,
        })
        .collect();
    let mut events = Vec::with_capacity(proposed.len() + 1);
    for event in proposed {
        for id in masked_in(state, &event) {
            if !done.contains(&id) {
                done.push(id);
                events.extend(turned_up(state, id));
            }
        }
        events.push(event);
    }
    events
}

/// "would assign ... combat damage": the masked creatures in combat that assign combat damage in this pass are turned
/// face up before any of it is assigned, so they assign as what they are (CR 510.1).
pub(crate) fn r6008_assigning(state: &GameState, pass: DamagePass) -> Vec<Event> {
    let fighting: Vec<ObjectId> = state
        .combat()
        .attackers()
        .iter()
        .map(|attacker| attacker.object)
        .chain(
            state
                .combat()
                .attackers()
                .iter()
                .flat_map(|attacker| attacker.blockers().iter().copied()),
        )
        .collect();
    let mut events = Vec::new();
    for id in fighting {
        if masked(state, id)
            && crate::damage::strike_power(state, id, pass).is_some_and(|power| power > 0)
            && !events
                .iter()
                .any(|event| matches!(event, Event::TurnedFaceUp { object, .. } if *object == id))
        {
            events.extend(turned_up(state, id));
        }
    }
    events
}

#[cfg(test)]
mod tests {
    //! Goal: a card listed face down reads as the listed creature through the layers and is copied as it, only its
    //! controller looks at it, the noted name finds its card and a copy knows none; an instant or sorcery card stays face
    //! down; the mana spent pays a cost when some of it could; a masked permanent is turned face up just before the first
    //! event that taps it, damages it or has it deal damage, once per batch.
    use mtg_cards::ability::Ability;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::characteristics::characteristics;
    use crate::combat::DamageForm;
    use crate::copies::copiable_values;
    use crate::fixtures::{creature, state};

    /// Seat 0 controls a card of `kind` that lies face down as `how`.
    fn lying(how: FaceDown, kind: CardType) -> (GameState, ObjectId) {
        let card = Arc::new(CardDefinition::new("Hidden Thing").with_type(kind));
        let mut game = state(vec![vec![], vec![]], vec![vec![card], vec![]]);
        let id = game.zone(Zone::Battlefield)[0];
        game.objects.get_mut(&id).unwrap().face_down = Some(how);
        game.layers.reset();
        (game, id)
    }

    /// Lists a 3/3 flyer for the face-down `id`, with the name noted.
    fn listed(game: &mut GameState, id: ObjectId, noted: Option<&str>) {
        let flying: Ability = serde_json::from_str(r#"{"Keyword":"Flying"}"#).unwrap();
        let listed = R6008Listed {
            object: id,
            definition: Arc::new(listed_definition(3, 3, &[flying])),
            noted: noted.map(str::to_owned),
        };
        crate::fold::apply_event(game, &Event::R6008Listed(Box::new(listed))).unwrap();
        game.layers.reset();
    }

    #[test]
    fn a_listed_card_is_the_listed_creature_to_the_layers_and_to_a_copy() {
        let (mut game, id) = lying(FaceDown::Morph, CardType::Sorcery);
        listed(&mut game, id, Some("Hidden Thing"));
        let chars = characteristics(&game, id).unwrap();
        assert!(chars.types.contains(CardType::Creature));
        assert!(!chars.types.contains(CardType::Sorcery));
        assert_eq!((chars.power, chars.toughness), (Some(3), Some(3)));
        assert!(chars.colors.is_empty());
        assert_eq!(game.object(id).unwrap().card().name, "");
        assert_eq!(crate::keywords::keywords_of(&game, id).len(), 1);
        let values = copiable_values(&game, game.object(id).unwrap().reference(), None).unwrap();
        assert_eq!((values.power, values.toughness), (Some(3), Some(3)));
        assert_eq!(values.name, "");
        assert_eq!(game.object(id).unwrap().whole_card().name, "Hidden Thing");
    }

    #[rstest]
    #[case::the_controller(0, true)]
    #[case::an_opponent(1, false)]
    fn only_the_controller_looks_at_a_listed_or_masked_permanent(
        #[case] viewer: usize,
        #[case] looks: bool,
        #[values(FaceDown::R6008Listed, FaceDown::R6008Masked)] how: FaceDown,
    ) {
        let (mut game, id) = lying(FaceDown::Morph, CardType::Creature);
        if how == FaceDown::R6008Listed {
            listed(&mut game, id, None);
        } else {
            game.objects.get_mut(&id).unwrap().face_down = Some(how);
        }
        assert_eq!(game.object(id).unwrap().face_down(), Some(how));
        assert_eq!(game.may_look_at(PlayerId::from_index(viewer), id), looks);
    }

    #[rstest]
    #[case::its_own_name(Some("Hidden Thing"), true)]
    #[case::a_name_the_game_cannot_find(Some("Nothing Such"), false)]
    #[case::no_name_as_a_copy_knows_none(None, false)]
    fn the_noted_name_finds_the_card_behind_the_permanent(
        #[case] noted: Option<&str>,
        #[case] found: bool,
    ) {
        let (mut game, id) = lying(FaceDown::Morph, CardType::Instant);
        listed(&mut game, id, noted);
        let card = noted_card(&game, id);
        assert_eq!(card.is_some(), found);
        assert!(card.is_none_or(|card| card.name == "Hidden Thing"));
    }

    #[rstest]
    #[case::an_instant(CardType::Instant, true)]
    #[case::a_sorcery(CardType::Sorcery, true)]
    #[case::a_creature(CardType::Creature, false)]
    fn a_face_down_instant_or_sorcery_card_stays_face_down(
        #[case] kind: CardType,
        #[case] stays: bool,
    ) {
        let (game, id) = lying(FaceDown::Morph, kind);
        assert_eq!(stays_face_down(&game, id), stays);
    }

    #[rstest]
    #[case::the_same_symbols("{U}{U}", true)]
    #[case::one_generic("{1}{U}", true)]
    #[case::all_generic("{2}", true)]
    #[case::hybrid("{W/U}{W/U}", true)]
    #[case::some_of_it("{U}", true)]
    #[case::x_counts_as_zero("{X}{U}", true)]
    #[case::phyrexian_by_mana("{U/P}", true)]
    #[case::more_than_spent("{2}{U}", false)]
    #[case::another_color("{G}", false)]
    #[case::colorless_mana_needed("{C}", false)]
    fn the_mana_spent_pays_a_cost_some_of_it_could(#[case] cost: &str, #[case] paid: bool) {
        let cost = ManaCost::parse(cost).unwrap();
        assert_eq!(paid_by(&cost, &[ManaType::Blue, ManaType::Blue]), paid);
    }

    fn dealt(source: ObjectId, target: DamageTarget) -> Event {
        Event::DamageDealt {
            source,
            target,
            amount: 2,
            deathtouch: false,
            form: DamageForm::Normal,
            toxic: 0,
            unpreventable: false,
        }
    }

    fn tapped(id: ObjectId, _: ObjectId) -> Event {
        Event::PermanentTapped { object: id }
    }

    fn damaged(id: ObjectId, other: ObjectId) -> Event {
        dealt(other, DamageTarget::Object(id))
    }

    fn dealing(id: ObjectId, _: ObjectId) -> Event {
        dealt(id, DamageTarget::Player(PlayerId::from_index(1)))
    }

    #[rstest]
    #[case::tapped(tapped)]
    #[case::dealt_damage(damaged)]
    #[case::dealing_damage(dealing)]
    fn a_masked_permanent_turns_face_up_before_it_is_tapped_or_damaged_or_deals_damage(
        #[case] event: fn(ObjectId, ObjectId) -> Event,
        #[values(true, false)] masked: bool,
    ) {
        let mut game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Whale", 6, 6)], vec![creature("Bear", 2, 2)]],
        );
        let id = game.zone(Zone::Battlefield)[0];
        let other = game.zone(Zone::Battlefield)[1];
        game.objects.get_mut(&id).unwrap().face_down = Some(if masked {
            FaceDown::R6008Masked
        } else {
            FaceDown::Morph
        });
        let proposed = vec![event(id, other), event(id, other)];
        let out = r6008_turned_up_first(&game, proposed.clone());
        if !masked {
            assert_eq!(out, proposed);
            return;
        }
        let up = Event::TurnedFaceUp {
            object: id,
            player: PlayerId::from_index(0),
            x: 0,
        };
        assert_eq!(out, vec![up, proposed[0].clone(), proposed[1].clone()]);
        // The step run again on the same batch adds no second turn up.
        assert_eq!(r6008_turned_up_first(&game, out.clone()), out);
    }
}

