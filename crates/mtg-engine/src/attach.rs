//! Attachments: Auras, Equipment and Fortifications attached to a permanent (CR 303.4, 301.5, 701.3), the target an Aura
//! spell has through its enchant ability, and the state-based actions that keep an attachment where it may be
//! (CR 704.5m to 704.5p).

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Bounds, Pred, Selector, TargetSpec, Targetable};
use mtg_core::card::{CardType, Subtype};
use mtg_core::ids::{ObjectId, PlayerId};

use crate::characteristics::{characteristics, controller_of_object};
use crate::choose::{Target, legal_targets, options_for};
use crate::error::EngineError;
use crate::events::{Event, ZoneCause};
use crate::fold::invalid;
use crate::keywords::{self, Holder, keywords_of};
use crate::object::{GameObject, ObjectKind};
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

/// What a permanent that attaches to another is, by its subtype.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// An Aura: it enchants what its enchant ability allows (CR 303.4).
    Aura,
    /// An Equipment: it equips a creature (CR 301.5).
    Equipment,
    /// A Fortification: it fortifies a land (CR 301.6).
    Fortification,
}

const KINDS: [(&str, Kind); 3] = [
    ("Aura", Kind::Aura),
    ("Equipment", Kind::Equipment),
    ("Fortification", Kind::Fortification),
];

/// Whether a card prints the subtype of an attachment, so a game that has none skips their state-based actions.
pub(crate) fn definition_attaches(definition: &CardDefinition) -> bool {
    definition
        .subtypes
        .iter()
        .any(|subtype| KINDS.iter().any(|(name, _)| subtype.0 == *name))
        || crate::ability_reader::c6_blk_definition_gives_subtype(
            definition,
            &KINDS.map(|(name, _)| name),
        )
}

/// What `id` is as an attachment, by the subtypes it prints and those effects gave it.
pub(crate) fn kind_of(state: &GameState, id: ObjectId) -> Option<Kind> {
    let object = state.object(id)?;
    // A bestowed creature card is an Aura though it prints no such subtype (CR 702.103b).
    if object.bestowed() {
        return Some(Kind::Aura);
    }
    // The subtypes effects gave a permanent are held by the layers, which a pass over the battlefield reads in place.
    let given = (object.zone() == Zone::Battlefield)
        .then(|| state.layers().get(id))
        .flatten()
        .map(|held| held.chars.subtypes.as_slice());
    let has = |subtypes: &[Subtype], name: &str| subtypes.iter().any(|s| s.0 == name);
    KINDS
        .iter()
        .find(|(name, _)| {
            has(&object.card().subtypes, name) || given.is_some_and(|given| has(given, name))
        })
        .map(|(_, kind)| *kind)
}

/// What the enchant abilities of `id` allow, as one selector since every one of them holds (CR 702.5d); `None` for an
/// object with none.
pub(crate) fn enchant_restriction(state: &GameState, id: ObjectId) -> Option<Selector> {
    let mut found = false;
    let mut preds = Vec::new();
    // A bestowed Aura has enchant creature (CR 702.103b), whatever else it says.
    if state.object(id).is_some_and(GameObject::bestowed) {
        found = true;
        preds.push(Pred::Type(CardType::Creature));
    }
    for keyword in keywords_of(state, id) {
        if let Keyword::Enchant(selector) = keyword {
            found = true;
            preds.extend(selector.0.iter().cloned());
        }
    }
    found.then_some(Selector(preds))
}

/// Whether `attachment` may become attached to `host` as they stand: the Aura enchants what its enchant ability allows and
/// is not a creature (CR 303.4d, 303.4e), an Equipment equips another creature and is no creature, unless it has reconfigure
/// (CR 301.5c), a Fortification fortifies another land (CR 301.6). A permanent never attaches to itself.
pub(crate) fn legal_host(state: &GameState, attachment: ObjectId, host: ObjectId) -> bool {
    judged_host(state, attachment, host, true)
}

/// Whether `attachment`, a card about to be put onto the battlefield, may be attached to `host` as it enters (CR 303.4g,
/// 301.5): `legal_host` for a card that is not there yet.
pub(crate) fn legal_host_entering(state: &GameState, attachment: ObjectId, host: ObjectId) -> bool {
    judged_host(state, attachment, host, false)
}

/// `legal_host`, for an attachment that must already be on the battlefield or not.
fn judged_host(state: &GameState, attachment: ObjectId, host: ObjectId, in_play: bool) -> bool {
    if attachment == host {
        return false;
    }
    let (Some(object), Some(target)) = (state.object(attachment), state.object(host)) else {
        return false;
    };
    if (in_play && object.zone() != Zone::Battlefield)
        || target.zone() != Zone::Battlefield
        || target.kind() == ObjectKind::Ability
    {
        return false;
    }
    let (Some(chars), Some(host_chars)) = (
        characteristics(state, attachment),
        characteristics(state, host),
    ) else {
        return false;
    };
    let allowed = match kind_of(state, attachment) {
        Some(Kind::Aura) => !chars.is_creature() && may_enchant(state, object, target),
        Some(Kind::Equipment) => {
            (!chars.is_creature() || has_reconfigure(state, attachment)) && host_chars.is_creature()
        }
        Some(Kind::Fortification) => host_chars.types.contains(CardType::Land),
        None => false,
    };
    allowed && !protects_host(state, attachment, host)
}

/// Whether the enchant ability of `id` is "enchant opponent" (`true`) or "enchant player" (`false`); `None` for an object
/// that enchants no player.
fn player_restriction(state: &GameState, id: ObjectId) -> Option<bool> {
    keywords_of(state, id).into_iter().find_map(|keyword| {
        if let Keyword::EnchantPlayer { opponent } = keyword {
            Some(*opponent)
        } else {
            None
        }
    })
}

/// Whether the Aura `attachment` may be attached to the player `player` as things stand: it enchants players, is not a
/// creature, the player is still in the game, an opponent of its controller when it enchants only opponents, and has no
/// protection from it (CR 303.4, 702.16c).
pub(crate) fn legal_player_host(state: &GameState, attachment: ObjectId, player: PlayerId) -> bool {
    let Some(object) = state.object(attachment) else {
        return false;
    };
    if object.zone() != Zone::Battlefield
        || object.kind() == ObjectKind::Ability
        || !state.is_alive(player)
        || kind_of(state, attachment) != Some(Kind::Aura)
    {
        return false;
    }
    let Some(opponent) = player_restriction(state, attachment) else {
        return false;
    };
    characteristics(state, attachment).is_some_and(|chars| !chars.is_creature())
        && (!opponent || player != controller_of_object(state, object))
        && !keywords::Source::live(state, attachment)
            .is_some_and(|source| keywords::protected_from(state, Holder::Player(player), source))
}

/// Whether a keyword of `host` (protection) keeps `attachment` from being attached to it (CR 702.16c).
fn protects_host(state: &GameState, attachment: ObjectId, host: ObjectId) -> bool {
    keywords::Source::live(state, attachment)
        .is_some_and(|source| keywords::protected_from(state, Holder::Object(host), source))
}

/// Whether `id` has reconfigure, so it may equip a creature while it is a creature itself (CR 301.5c).
fn has_reconfigure(state: &GameState, id: ObjectId) -> bool {
    keywords_of(state, id)
        .iter()
        .any(|keyword| matches!(keyword, Keyword::Reconfigure(_)))
}

/// Whether the enchant abilities of the Aura allow `host`, judged for the Aura's controller.
fn may_enchant(state: &GameState, aura: &GameObject, host: &GameObject) -> bool {
    let Some(selector) = enchant_restriction(state, aura.id()) else {
        return true;
    };
    Scope::announcing(state, controller_of_object(state, aura), aura.reference())
        .selects(&selector, host.reference())
        .unwrap_or(false)
}

/// The target specification of an Aura spell: what its enchant abilities allow, one target (CR 303.4a). `None` for
/// anything but an Aura card.
pub(crate) fn spell_target(state: &GameState, id: ObjectId) -> Option<TargetSpec> {
    let object = state.object(id)?;
    if object.kind() == ObjectKind::Ability
        || !(object.bestowed() || object.card().subtypes.iter().any(|s| s.0 == "Aura"))
        || object.base_types().contains(CardType::Instant)
        || object.base_types().contains(CardType::Sorcery)
    {
        return None;
    }
    let what = match (
        enchant_restriction(state, id),
        player_restriction(state, id),
    ) {
        (Some(selector), _) => Targetable::Object(selector),
        (None, Some(true)) => Targetable::Opponent,
        (None, Some(false)) => Targetable::Player,
        (None, None) => return None,
    };
    Some(aura_target(what))
}

/// The one target of an Aura spell (CR 303.4a).
fn aura_target(what: Targetable) -> TargetSpec {
    TargetSpec {
        what,
        bounds: Bounds::default(),
        divide: None,
    }
}

/// The target specification of a creature card cast for its bestow cost: one creature (CR 702.103b, 303.4a). The card
/// is not on the stack yet while its targets are announced, so it is not yet bestowed.
pub(crate) fn bestow_target() -> TargetSpec {
    aura_target(Targetable::Object(Selector(vec![Pred::Type(
        CardType::Creature,
    )])))
}

/// Whether `player` can announce a creature for the card `id` cast for its bestow cost (CR 601.2c).
pub(crate) fn bestow_announceable(state: &GameState, player: PlayerId, id: ObjectId) -> bool {
    options_for(
        state,
        id,
        player,
        0,
        &bestow_target(),
        true,
        None,
        None,
        &[],
        None,
    )
    .is_some()
}

/// What an Aura is attached to: a permanent or a player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Host {
    /// A permanent on the battlefield.
    Object(ObjectId),
    /// A player in the game.
    Player(PlayerId),
}

impl Host {
    /// The event that attaches `object` to this host.
    pub(crate) fn attach(self, object: ObjectId) -> Event {
        match self {
            Host::Object(to) => Event::Attached { object, to },
            Host::Player(player) => Event::AttachedToPlayer { object, player },
        }
    }
}

/// Whether `player` can cast the spell `id` as far as its target goes: an Aura spell needs something to enchant that it
/// may target (CR 601.2c); any other spell asks nothing here.
pub(crate) fn announceable(state: &GameState, player: PlayerId, id: ObjectId) -> bool {
    spell_target(state, id).is_none_or(|spec| {
        options_for(state, id, player, 0, &spec, true, None, None, &[], None).is_some()
    })
}

/// Whether `id` is an Aura spell on the stack.
pub(crate) fn is_aura_spell(state: &GameState, id: ObjectId) -> bool {
    state
        .object(id)
        .is_some_and(|object| object.zone() == Zone::Stack)
        && spell_target(state, id).is_some()
}

/// What the Aura spell `id` is going to enchant: the target it announced, if that is still legal as it resolves
/// (CR 608.2b).
pub(crate) fn spell_host(state: &GameState, id: ObjectId) -> Option<Host> {
    let spec = spell_target(state, id)?;
    let spell = state.object(id)?;
    let target = *spell.choices().targets.first()?.first()?;
    legal_targets(state, &spec, id, spell.controller())
        .contains(&target)
        .then_some(match target {
            Target::Object(object) => Host::Object(object.id),
            Target::Player(player) => Host::Player(player),
        })
}

/// The attachment of an Aura spell that resolves: it enters attached to what it targeted (CR 303.4f).
pub(crate) fn entering(state: &GameState, id: ObjectId) -> Option<Event> {
    if !is_aura_spell(state, id) {
        return None;
    }
    Some(spell_host(state, id)?.attach(id))
}

/// The permanents an Aura card that enters the battlefield without being cast under `controller` may enchant, in id
/// order (CR 303.4g); `None` for a card that is not an Aura.
pub(crate) fn hosts_when_entering(
    state: &GameState,
    aura: &GameObject,
    controller: PlayerId,
) -> Option<Vec<ObjectId>> {
    let spec = spell_target(state, aura.id())?;
    let Targetable::Object(selector) = spec.what else {
        // An Aura that enchants a player does not enter without being cast: the question of which player is not asked,
        // so the card stays where it was.
        return Some(Vec::new());
    };
    let scope = Scope::announcing(state, controller, aura.reference());
    let found = scope.matching(&selector).unwrap_or_default();
    Some(
        found
            .into_iter()
            .filter(|named| {
                state.object(named.id).is_some_and(|host| {
                    host.zone() == Zone::Battlefield
                        && host.kind() != ObjectKind::Ability
                        && !protects_host(state, aura.id(), named.id)
                })
            })
            .map(|named| named.id)
            .collect(),
    )
}

/// Attaches `object` to `to` (CR 701.3): it takes the time it became attached as its timestamp (CR 613.7a).
pub(crate) fn fold_attached(
    state: &mut GameState,
    object: ObjectId,
    to: ObjectId,
) -> Result<(), EngineError> {
    if object == to {
        return Err(invalid("a permanent cannot be attached to itself"));
    }
    let host = state
        .object(to)
        .filter(|host| host.zone() == Zone::Battlefield)
        .map(GameObject::reference)
        .ok_or_else(|| {
            invalid(format!(
                "{to:?} is not on the battlefield to be attached to"
            ))
        })?;
    let stamp = state.next_timestamp;
    let permanent = state
        .objects
        .get_mut(&object)
        .and_then(|o| o.permanent.as_mut())
        .ok_or_else(|| {
            invalid(format!(
                "{object:?} is not on the battlefield to be attached"
            ))
        })?;
    if permanent.attached_to == Some(host) {
        return Err(invalid(format!("{object:?} is attached to {to:?} already")));
    }
    permanent.attached_to = Some(host);
    permanent.attached_player = None;
    permanent.timestamp = stamp;
    state.next_timestamp += 1;
    state.possible.attachments = true;
    Ok(())
}

/// Attaches `object` to the player `player` (CR 303.4): it takes the time it became attached as its timestamp.
pub(crate) fn fold_attached_to_player(
    state: &mut GameState,
    object: ObjectId,
    player: PlayerId,
) -> Result<(), EngineError> {
    if !state.is_alive(player) {
        return Err(invalid(format!(
            "{player:?} is not in the game to be enchanted"
        )));
    }
    let stamp = state.next_timestamp;
    let permanent = state
        .objects
        .get_mut(&object)
        .and_then(|o| o.permanent.as_mut())
        .ok_or_else(|| {
            invalid(format!(
                "{object:?} is not on the battlefield to be attached"
            ))
        })?;
    if permanent.attached_player == Some(player) {
        return Err(invalid(format!(
            "{object:?} is attached to {player:?} already"
        )));
    }
    permanent.attached_to = None;
    permanent.attached_player = Some(player);
    permanent.timestamp = stamp;
    state.next_timestamp += 1;
    state.possible.attachments = true;
    Ok(())
}

/// Takes `object` off what it is attached to.
pub(crate) fn fold_unattached(state: &mut GameState, object: ObjectId) -> Result<(), EngineError> {
    let permanent = state
        .objects
        .get_mut(&object)
        .and_then(|o| o.permanent.as_mut())
        .ok_or_else(|| {
            invalid(format!(
                "{object:?} is not on the battlefield to be unattached"
            ))
        })?;
    let object_host = permanent.attached_to.take();
    let player_host = permanent.attached_player.take();
    if object_host.is_none() && player_host.is_none() {
        return Err(invalid(format!("{object:?} is not attached to anything")));
    }
    Ok(())
}

/// The state-based actions of attachments, all at once (CR 704.5m to 704.5p): an Aura attached to nothing or to what it may
/// not enchant goes to its owner's graveyard, an Equipment or Fortification attached to what it may not be becomes
/// unattached, and any other permanent attached to something becomes unattached. A host that left is no host (CR 400.7).
pub(crate) fn state_based(state: &GameState, leaving: &[ObjectId]) -> Vec<Event> {
    let mut events = Vec::new();
    if !state.possible.attachments {
        return events;
    }
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state
            .object(id)
            .filter(|object| state.is_alive(object.controller()) && !leaving.contains(&id))
        else {
            continue;
        };
        let attached = object.permanent().is_some_and(|permanent| {
            permanent.attached_to().is_some() || permanent.attached_player().is_some()
        });
        let legal = || match (state.host_of(id), state.player_host_of(id)) {
            (Some(host), _) => legal_host(state, id, host),
            (None, Some(player)) => legal_player_host(state, id, player),
            (None, None) => false,
        };
        // The enchant ability of an Aura the engine does not run is a clause the table follows: its controller attaches it
        // by hand (`manual.rs`), so it waits unattached.
        let by_hand = !attached && !object.card().manual.is_empty();
        match kind_of(state, id) {
            // A bestowed Aura that may not stay attached becomes a creature instead (CR 702.103e).
            Some(Kind::Aura) if object.bestowed() && !legal() => {
                events.push(Event::Unattached { object: id });
            }
            Some(Kind::Aura) if !legal() && !by_hand => events.push(Event::ZoneChanged {
                object: id,
                from: Zone::Battlefield,
                to: Zone::Graveyard(object.owner()),
                cause: ZoneCause::StateBased,
            }),
            Some(Kind::Equipment | Kind::Fortification) if attached && !legal() => {
                events.push(Event::Unattached { object: id });
            }
            None if attached => events.push(Event::Unattached { object: id }),
            _ => {}
        }
    }
    events
}

#[cfg(test)]
mod player_tests;

#[cfg(test)]
mod tests {
    //! Goal: an attachment is legal exactly where its kind allows (an Aura where its enchant ability does and never on itself or when it is a creature, an Equipment on a creature when it is none, a Fortification on a land), becoming attached sets the host and gives the attachment the next timestamp while becoming unattached clears the host and refuses what is not attached, the state-based actions put an Aura attached to nothing or to what it may not enchant into its owner's graveyard and take an Equipment, a Fortification or any other permanent off what it may not be attached to, an Aura card targets through its enchant ability and no other card does, and an Aura entering without being cast may enchant the permanents its ability allows.
    use std::sync::Arc;

    use mtg_cards::ability::Ability;
    use mtg_core::card::Subtype;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::fold::apply_event;
    use crate::setup::PresetPermanent;

    const ME: PlayerId = PlayerId(0);

    fn card(name: &str, types: &[CardType], subtype: &str, rules: &[&str]) -> Arc<CardDefinition> {
        let mut card = CardDefinition::new(name);
        for ty in types {
            card = card.with_type(*ty);
        }
        if !subtype.is_empty() {
            card.subtypes.push(Subtype(subtype.into()));
        }
        for json in rules {
            card = card.with_rules(serde_json::from_str::<Ability>(json).unwrap());
        }
        Arc::new(card)
    }

    const ON_CREATURES: &str = r#"{"Keyword":{"Enchant":[{"Type":"Creature"}]}}"#;
    const ON_OPPONENTS: &str =
        r#"{"Keyword":{"Enchant":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}"#;
    const ON_LANDS: &str = r#"{"Keyword":{"Enchant":[{"Type":"Land"}]}}"#;
    const RECONFIGURE: &str = r#"{"Keyword":{"Reconfigure":"{2}"}}"#;

    // Places on the battlefield of seat 0.
    const BEAR: usize = 0;
    const FOREST: usize = 1;
    const HALO: usize = 2;
    const CURSE: usize = 3;
    const RUNE: usize = 4;
    const BLADE: usize = 5;
    const WALL: usize = 6;
    const GOLEM: usize = 7;
    const WOLF: usize = 8;
    const RECONFIGURER: usize = 9;
    /// The Ogre of seat 1.
    const OGRE: (usize, usize) = (1, 0);

    /// Seat 0, in this order: a Bear, a Forest, a Halo Aura on creatures, a Curse Aura on creatures of opponents, a Rune
    /// Aura on lands, a Blade Equipment, a Wall Fortification, a Golem Equipment that is a creature, a Wolf and a
    /// Reconfigurer Equipment that is a creature. Seat 1: an Ogre. Each pair says which permanent of seat 0 starts attached
    /// to which.
    fn game(attached: &[(usize, (usize, usize))]) -> GameState {
        let mut game = setup(
            vec![vec![], vec![]],
            vec![
                vec![
                    creature("Bear", 2, 2),
                    card("Forest", &[CardType::Land], "", &[]),
                    card("Halo", &[CardType::Enchantment], "Aura", &[ON_CREATURES]),
                    card("Curse", &[CardType::Enchantment], "Aura", &[ON_OPPONENTS]),
                    card("Rune", &[CardType::Enchantment], "Aura", &[ON_LANDS]),
                    card("Blade", &[CardType::Artifact], "Equipment", &[]),
                    card("Wall", &[CardType::Artifact], "Fortification", &[]),
                    card(
                        "Golem",
                        &[CardType::Artifact, CardType::Creature],
                        "Equipment",
                        &[],
                    ),
                    creature("Wolf", 2, 2),
                    card(
                        "Reconfigurer",
                        &[CardType::Artifact, CardType::Creature],
                        "Equipment",
                        &[RECONFIGURE],
                    ),
                ],
                vec![creature("Ogre", 3, 4)],
            ],
        );
        for (place, host) in attached {
            let preset = &mut game.players[0].battlefield[*place];
            *preset = PresetPermanent {
                attached_to: Some(*host),
                ..preset.clone()
            };
        }
        GameState::from_setup(&game).unwrap()
    }

    fn id(game: &GameState, name: &str) -> ObjectId {
        game.objects().find(|o| o.card().name == name).unwrap().id()
    }

    #[rstest]
    #[case::an_aura_on_a_creature("Halo", "Bear", true)]
    #[case::an_aura_on_a_creature_of_another_seat("Halo", "Ogre", true)]
    #[case::an_aura_on_a_land_it_may_not_enchant("Halo", "Forest", false)]
    #[case::an_aura_for_opponents_on_their_creature("Curse", "Ogre", true)]
    #[case::an_aura_for_opponents_on_your_creature("Curse", "Bear", false)]
    #[case::an_aura_on_a_land("Rune", "Forest", true)]
    #[case::an_aura_on_itself("Halo", "Halo", false)]
    #[case::an_equipment_on_a_creature("Blade", "Wolf", true)]
    #[case::an_equipment_on_a_land("Blade", "Forest", false)]
    #[case::an_equipment_that_is_a_creature_on_a_creature("Golem", "Wolf", false)]
    #[case::an_equipment_creature_with_reconfigure_on_a_creature("Reconfigurer", "Wolf", true)]
    #[case::an_equipment_creature_with_reconfigure_on_a_land("Reconfigurer", "Forest", false)]
    #[case::an_equipment_creature_with_reconfigure_on_itself("Reconfigurer", "Reconfigurer", false)]
    #[case::an_equipment_on_itself("Blade", "Blade", false)]
    #[case::a_fortification_on_a_land("Wall", "Forest", true)]
    #[case::a_fortification_on_a_creature("Wall", "Bear", false)]
    #[case::a_creature_on_a_creature("Bear", "Wolf", false)]
    fn an_attachment_is_legal_where_its_kind_allows(
        #[case] attachment: &str,
        #[case] host: &str,
        #[case] legal: bool,
    ) {
        let game = game(&[]);
        assert_eq!(
            legal_host(&game, id(&game, attachment), id(&game, host)),
            legal
        );
    }

    #[test]
    fn becoming_attached_sets_the_host_and_the_next_timestamp_and_becoming_unattached_clears_it() {
        let mut game = game(&[]);
        let (halo, bear, ogre) = (id(&game, "Halo"), id(&game, "Bear"), id(&game, "Ogre"));
        let before = game.next_timestamp;
        let stamp = |game: &GameState| game.object(halo).unwrap().permanent().unwrap().timestamp();
        apply_event(
            &mut game,
            &Event::Attached {
                object: halo,
                to: bear,
            },
        )
        .unwrap();
        assert_eq!(game.host_of(halo), Some(bear));
        assert_eq!((stamp(&game), game.next_timestamp), (before, before + 1));
        // Moving it to another host takes another time and leaves the first host with nothing.
        apply_event(
            &mut game,
            &Event::Attached {
                object: halo,
                to: ogre,
            },
        )
        .unwrap();
        assert_eq!((game.host_of(halo), stamp(&game)), (Some(ogre), before + 1));
        assert_eq!(game.attachments_of(bear), []);
        apply_event(&mut game, &Event::Unattached { object: halo }).unwrap();
        assert_eq!(game.host_of(halo), None);
        assert_eq!(
            game.object(halo)
                .unwrap()
                .permanent()
                .unwrap()
                .attached_to(),
            None
        );
    }

    #[rstest]
    #[case::to_itself("Halo", "Halo")]
    #[case::to_what_is_already_its_host("Halo", "Bear")]
    fn an_attachment_that_cannot_become_attached_is_refused(
        #[case] object: &str,
        #[case] to: &str,
    ) {
        let mut game = game(&[(HALO, (0, BEAR))]);
        let event = Event::Attached {
            object: id(&game, object),
            to: id(&game, to),
        };
        assert!(apply_event(&mut game, &event).is_err());
    }

    #[test]
    fn a_permanent_attached_to_nothing_cannot_become_unattached() {
        let mut game = game(&[]);
        let event = Event::Unattached {
            object: id(&game, "Halo"),
        };
        assert!(apply_event(&mut game, &event).is_err());
    }

    /// What the state-based actions do to `game(attached)`: (name, put into the graveyard) for each, sorted; an attachment
    /// that becomes unattached is not put into the graveyard.
    fn actions(attached: &[(usize, (usize, usize))]) -> Vec<(String, bool)> {
        done_to(&game(attached))
    }

    fn done_to(game: &GameState) -> Vec<(String, bool)> {
        let mut done: Vec<(String, bool)> = state_based(game, &[])
            .iter()
            .map(|event| match event {
                Event::ZoneChanged {
                    object,
                    to: Zone::Graveyard(_),
                    cause: ZoneCause::StateBased,
                    ..
                } => (game.object(*object).unwrap().card().name.clone(), true),
                Event::Unattached { object } => {
                    (game.object(*object).unwrap().card().name.clone(), false)
                }
                other => panic!("{other:?}"),
            })
            .collect();
        done.sort();
        done
    }

    /// The Auras on a creature, an opponent's creature and a land, each where it may be.
    const AURAS: [(usize, (usize, usize)); 3] =
        [(HALO, (0, BEAR)), (CURSE, OGRE), (RUNE, (0, FOREST))];

    fn with(extra: &[(usize, (usize, usize))]) -> Vec<(usize, (usize, usize))> {
        AURAS.iter().chain(extra).copied().collect()
    }

    #[rstest]
    #[case::every_aura_waits_for_a_host(vec![], &[("Curse", true), ("Halo", true), ("Rune", true)])]
    #[case::attachments_where_they_may_be_stay(
        with(&[(BLADE, (0, WOLF)), (WALL, (0, FOREST))]),
        &[]
    )]
    #[case::an_aura_on_what_it_may_not_enchant_goes(
        vec![(HALO, (0, FOREST)), (CURSE, (0, BEAR)), (RUNE, (0, BEAR))],
        &[("Curse", true), ("Halo", true), ("Rune", true)]
    )]
    #[case::an_equipment_on_a_land_comes_off(with(&[(BLADE, (0, FOREST))]), &[("Blade", false)])]
    #[case::an_equipment_that_is_a_creature_comes_off(with(&[(GOLEM, (0, WOLF))]), &[("Golem", false)])]
    #[case::an_equipment_with_reconfigure_is_no_creature_while_attached_and_stays(
        with(&[(RECONFIGURER, (0, WOLF))]),
        &[]
    )]
    #[case::a_fortification_on_a_creature_comes_off(with(&[(WALL, (0, BEAR))]), &[("Wall", false)])]
    #[case::any_other_permanent_comes_off(with(&[(WOLF, (0, BEAR))]), &[("Wolf", false)])]
    fn the_state_based_actions_of_attachments(
        #[case] attached: Vec<(usize, (usize, usize))>,
        #[case] expected: &[(&str, bool)],
    ) {
        let expected: Vec<(String, bool)> = expected
            .iter()
            .map(|(name, put)| ((*name).to_owned(), *put))
            .collect();
        assert_eq!(actions(&attached), expected);
    }

    #[rstest]
    #[case::an_aura_goes_with_its_host("Bear", ("Halo", true))]
    #[case::an_equipment_comes_off("Wolf", ("Blade", false))]
    fn a_host_that_left_is_no_host_so_its_aura_goes_and_its_equipment_comes_off(
        #[case] host: &str,
        #[case] expected: (&str, bool),
    ) {
        let mut game = game(&with(&[(BLADE, (0, WOLF))]));
        let host = id(&game, host);
        assert_eq!(done_to(&game), []);
        apply_event(
            &mut game,
            &Event::ZoneChanged {
                object: host,
                from: Zone::Battlefield,
                to: Zone::Graveyard(ME),
                cause: ZoneCause::Destroyed,
            },
        )
        .unwrap();
        // The attachment still names the old object, which stopped being a host the moment it left (CR 400.7).
        assert_eq!(done_to(&game), [(expected.0.to_owned(), expected.1)]);
    }

    #[test]
    fn an_aura_that_dies_in_the_same_pass_is_not_moved_twice() {
        let game = game(&[]);
        let halo = id(&game, "Halo");
        assert!(!state_based(&game, &[halo]).iter().any(|event| matches!(
            event,
            Event::ZoneChanged { object, .. } if *object == halo
        )));
    }

    #[test]
    fn an_aura_card_targets_through_its_enchant_ability_and_no_other_card_does() {
        let game = game(&[]);
        let spec = spell_target(&game, id(&game, "Rune")).unwrap();
        assert_eq!(
            spec.what,
            Targetable::Object(Selector(vec![mtg_cards::refs::Pred::Type(CardType::Land)]))
        );
        assert!(spec.bounds.is_one());
        for other in ["Bear", "Blade", "Forest"] {
            assert!(spell_target(&game, id(&game, other)).is_none(), "{other}");
        }
    }

    #[test]
    fn an_aura_entering_without_being_cast_may_enchant_what_its_ability_allows() {
        let game = game(&[]);
        let hosts = |name: &str| {
            let aura = game.object(id(&game, name)).unwrap();
            hosts_when_entering(&game, aura, ME).map(|ids| {
                ids.iter()
                    .map(|id| game.object(*id).unwrap().card().name.clone())
                    .collect::<Vec<_>>()
            })
        };
        assert_eq!(
            hosts("Halo").unwrap(),
            ["Bear", "Golem", "Wolf", "Reconfigurer", "Ogre"]
        );
        assert_eq!(hosts("Curse").unwrap(), ["Ogre"]);
        assert_eq!(hosts("Rune").unwrap(), ["Forest"]);
        assert_eq!(hosts("Blade"), None);
    }
}
