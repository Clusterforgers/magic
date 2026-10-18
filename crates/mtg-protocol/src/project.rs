//! Building a viewer's [`StateView`] from the full game state, and the visibility rule it rests on.

use mtg_core::card::{CardType, CardTypes};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::characteristics::characteristics;
use mtg_engine::combat::Attacker;
use mtg_engine::forecast::pending_combat_forecast;
use mtg_engine::player::{Player, PlayerStatus};
use mtg_engine::state::GameState;
use mtg_engine::zones::Zone;

use crate::view::{
    AttackerView, CombatView, ObjectView, PermanentView, PlayerView, PriorityView, StateView,
    TableView, TurnView, ZoneView,
};

/// Every card type, for expanding a [`CardTypes`] set into a list.
const ALL_TYPES: [CardType; 9] = [
    CardType::Artifact,
    CardType::Battle,
    CardType::Creature,
    CardType::Enchantment,
    CardType::Instant,
    CardType::Kindred,
    CardType::Land,
    CardType::Planeswalker,
    CardType::Sorcery,
];

// A new `CardType` variant fails to compile here, which is the cue to extend `ALL_TYPES`.
const fn _all_types_is_exhaustive(ty: CardType) {
    match ty {
        CardType::Artifact
        | CardType::Battle
        | CardType::Creature
        | CardType::Enchantment
        | CardType::Instant
        | CardType::Kindred
        | CardType::Land
        | CardType::Planeswalker
        | CardType::Sorcery => {}
    }
}

/// Whether `viewer` may see which cards are in `zone`.
///
/// Libraries are hidden from everyone, hands from everyone but their owner; a viewer who is not a
/// seat (a spectator) sees no hands. Face-down cards will need their own rule once the engine has them.
pub fn zone_is_visible(zone: Zone, viewer: PlayerId) -> bool {
    match zone {
        Zone::Library(_) => false,
        Zone::Hand(owner) => owner == viewer,
        Zone::Battlefield | Zone::Graveyard(_) | Zone::Stack | Zone::Exile | Zone::Command(_) => {
            true
        }
    }
}

fn count(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

fn zone_view(state: &GameState, viewer: PlayerId, zone: Zone) -> ZoneView {
    let ids = state.zone(zone);
    ZoneView {
        count: count(ids.len()),
        cards: zone_is_visible(zone, viewer).then(|| ids.to_vec()),
    }
}

fn type_list(types: CardTypes) -> Vec<CardType> {
    ALL_TYPES
        .into_iter()
        .filter(|ty| types.contains(*ty))
        .collect()
}

impl ObjectView {
    /// The view of `id` for `viewer`, or `None` when the object is unknown or hidden from them.
    pub fn of(state: &GameState, viewer: PlayerId, id: ObjectId) -> Option<ObjectView> {
        let object = state.object(id)?;
        if !zone_is_visible(object.zone(), viewer) {
            return None;
        }
        let chars = characteristics(state, id)?;
        let card = object.card();
        Some(ObjectView {
            id,
            owner: object.owner(),
            controller: object.controller(),
            zone: object.zone(),
            name: card.name.clone(),
            mana_cost: card.mana_cost.clone(),
            supertypes: card.supertypes.iter().copied().collect(),
            types: type_list(chars.types),
            subtypes: card.subtypes.clone(),
            colors: chars.colors.iter().collect(),
            power: chars.power,
            toughness: chars.toughness,
            abilities: chars.abilities,
            permanent: object.permanent().map(|p| PermanentView {
                tapped: p.tapped(),
                damage: p.damage(),
                summoning_sick: p.summoning_sick(),
            }),
        })
    }
}

impl PlayerView {
    fn of(state: &GameState, viewer: PlayerId, player: &Player) -> PlayerView {
        let seat = player.id();
        PlayerView {
            seat,
            life: player.life(),
            lost: match player.status() {
                PlayerStatus::InGame => None,
                PlayerStatus::Lost(reason) => Some(reason.clone()),
            },
            mana: *player.mana(),
            mulligans: player.mulligans(),
            mulligan: player.mulligan_status(),
            hand: zone_view(state, viewer, Zone::Hand(seat)),
            library: zone_view(state, viewer, Zone::Library(seat)),
            graveyard: zone_view(state, viewer, Zone::Graveyard(seat)),
            command: zone_view(state, viewer, Zone::Command(seat)),
        }
    }
}

fn attacker_view(attacker: &Attacker) -> AttackerView {
    AttackerView {
        object: attacker.object(),
        target: attacker.target(),
        blocked: attacker.blocked(),
        blockers: attacker.blockers().to_vec(),
    }
}

impl TableView {
    /// The table as `viewer` may see it: public zones listed, their own hand listed, counts for the rest.
    pub fn of(state: &GameState, viewer: PlayerId) -> TableView {
        let turn = state.turn();
        let combat = state.combat();
        TableView {
            viewer,
            format: state.format().name().to_owned(),
            stage: state.stage(),
            first_player: state.first_player(),
            turn: TurnView {
                number: turn.number(),
                active: turn.active(),
                step: turn.step(),
                lands_played: turn.lands_played(),
            },
            priority: state.priority().map(|p| PriorityView {
                holder: p.holder(),
                passed: p.passed().iter().copied().collect(),
            }),
            players: state
                .players()
                .iter()
                .map(|p| PlayerView::of(state, viewer, p))
                .collect(),
            battlefield: zone_view(state, viewer, Zone::Battlefield),
            stack: zone_view(state, viewer, Zone::Stack),
            exile: zone_view(state, viewer, Zone::Exile),
            combat: CombatView {
                attackers: combat.attackers().iter().map(attacker_view).collect(),
                attackers_declared: combat.attackers_declared(),
                blockers_declared: combat.blockers_declared().to_vec(),
            },
            combat_forecast: pending_combat_forecast(state),
            result: state.result(),
        }
    }
}

/// Every object `viewer` may identify, in id order.
pub(crate) fn visible_objects(state: &GameState, viewer: PlayerId) -> Vec<ObjectView> {
    state
        .objects()
        .filter_map(|o| ObjectView::of(state, viewer, o.id()))
        .collect()
}

impl StateView {
    /// Builds what `viewer` may know: their own hand, every public zone, and counts for the rest.
    pub fn for_player(state: &GameState, viewer: PlayerId) -> StateView {
        StateView {
            table: TableView::of(state, viewer),
            objects: visible_objects(state, viewer),
        }
    }
}
