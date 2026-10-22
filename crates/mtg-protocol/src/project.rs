//! Building a viewer's [`StateView`] from the full game state, and the visibility rule it rests on.

use mtg_core::card::{CardType, CardTypes};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::characteristics::{characteristics, controller_of};
use mtg_engine::combat::Attacker;
use mtg_engine::forecast::pending_combat_forecast;
use mtg_engine::keywords::{keywords_of, summoning_sick};
use mtg_engine::loyalty::loyalty_abilities;
use mtg_engine::object::{GameObject, ObjectKind};
use mtg_engine::player::{Player, PlayerStatus};
use mtg_engine::sight::offered_cards;
use mtg_engine::state::GameState;
use mtg_engine::zones::Zone;

use crate::view::{
    AttackerView, CombatView, LoyaltyAbilityView, ObjectView, PermanentView, PlayerView,
    PriorityView, StateView, TableView, TurnView, ZoneView,
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
/// seat (a spectator) sees no hands. A zone says which cards are there, never which of them lie face down: that is
/// judged object by object (`Sight::look`, which asks `GameState::may_look_at`), so a face-down card is listed in its
/// zone for every seat and identified only to the seat that may look at it.
pub fn zone_is_visible(zone: Zone, viewer: PlayerId) -> bool {
    match zone {
        Zone::Library(_) => false,
        Zone::Hand(owner) => owner == viewer,
        Zone::Battlefield | Zone::Graveyard(_) | Zone::Stack | Zone::Exile | Zone::Command(_) => {
            true
        }
    }
}

/// Whether `viewer` may see which cards are in `zone` of the game `state`: [`zone_is_visible`], and every hand when the
/// table has open hands.
pub fn zone_is_visible_in(state: &GameState, zone: Zone, viewer: PlayerId) -> bool {
    zone_is_visible(zone, viewer) || (matches!(zone, Zone::Hand(_)) && state.ruleset().open_hands())
}

/// What `viewer` may identify in a state: the cards of the zones they see into, the cards the resolution in progress
/// showed them (looked at, found, revealed) and the cards they are asked to choose among or to put in an order.
pub(crate) struct Sight<'a> {
    state: &'a GameState,
    viewer: PlayerId,
    offered: Vec<ObjectId>,
}

impl<'a> Sight<'a> {
    pub(crate) fn new(state: &'a GameState, viewer: PlayerId) -> Self {
        let offered = offered_cards(state)
            .filter(|(seat, _)| *seat == viewer)
            .map(|(_, cards)| cards)
            .unwrap_or_default();
        Sight {
            state,
            viewer,
            offered,
        }
    }

    /// How the viewer sees the object: whole, as a face-down card that is there with no identity, or not at all.
    pub(crate) fn look(&self, object: &GameObject) -> Look {
        let id = object.id();
        let in_view = zone_is_visible_in(self.state, object.zone(), self.viewer);
        if (in_view && self.state.may_look_at(self.viewer, id))
            || self.offered.contains(&id)
            || self.state.is_shown_to(self.viewer, id)
        {
            Look::Whole
        } else if in_view {
            Look::FaceDown
        } else {
            Look::Hidden
        }
    }
}

/// How a viewer sees an object (`Sight::look`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Look {
    /// They identify it.
    Whole,
    /// It lies face down in a zone they see into and they may not look at it: they see that it is there and nothing else.
    FaceDown,
    /// It is in a zone they cannot see into.
    Hidden,
}

fn count(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

fn zone_view(state: &GameState, viewer: PlayerId, zone: Zone) -> ZoneView {
    let ids = state.zone(zone);
    ZoneView {
        count: count(ids.len()),
        cards: zone_is_visible_in(state, zone, viewer).then(|| ids.to_vec()),
    }
}

/// The names of the keywords of `id`, each once, in the order the object lists them.
fn keyword_names(state: &GameState, id: ObjectId) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for keyword in keywords_of(state, id) {
        let name = keyword.name();
        if !names.iter().any(|seen| seen == name) {
            names.push(name.to_owned());
        }
    }
    names
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
        let sight = Sight::new(state, viewer);
        Self::with_attachments(state, &sight, id, || state.attachments_of(id))
    }

    /// [`ObjectView::of`] with the attachments of `id` given by `attachments`, which is asked only for a permanent: a whole
    /// view works them out once for every object (`GameState::attachment_index`).
    fn with_attachments(
        state: &GameState,
        sight: &Sight<'_>,
        id: ObjectId,
        attachments: impl FnOnce() -> Vec<ObjectId>,
    ) -> Option<ObjectView> {
        let object = state.object(id)?;
        match sight.look(object) {
            Look::Hidden => return None,
            Look::FaceDown => return Some(Self::face_down(state, object)),
            Look::Whole => {}
        }
        let chars = characteristics(state, id)?;
        let card = object.card();
        // An ability on the stack is named after its source and has nothing else of the source's printed side.
        let printed = object.kind() != ObjectKind::Ability;
        Some(ObjectView {
            id,
            owner: object.owner(),
            controller: controller_of(state, id).unwrap_or_else(|| object.controller()),
            zone: object.zone(),
            name: card.name.clone(),
            mana_cost: card.mana_cost.clone().filter(|_| printed),
            supertypes: if printed {
                card.supertypes.iter().copied().collect()
            } else {
                Vec::new()
            },
            types: type_list(chars.types),
            // What the card prints, then what effects gave it.
            subtypes: if printed {
                let mut all = card.subtypes.clone();
                let given = chars
                    .subtypes
                    .iter()
                    .filter(|subtype| !all.contains(subtype));
                all.extend(given.cloned().collect::<Vec<_>>());
                all
            } else {
                Vec::new()
            },
            colors: chars.colors.iter().collect(),
            power: chars.power,
            toughness: chars.toughness,
            abilities: chars.abilities,
            keywords: keyword_names(state, id),
            permanent: object.permanent().map(|p| PermanentView {
                tapped: p.tapped(),
                damage: p.damage(),
                // Haste excuses it, so the view says whether sickness holds it back, not only when it arrived.
                summoning_sick: summoning_sick(state, id),
            }),
            kind: object.kind(),
            source: object.ability().map(|ability| ability.source().id),
            targets: object.choices().targets.clone(),
            // Counters and attachments are public facts of a permanent, shown to every seat that sees the permanent.
            counters: object
                .permanent()
                .map(|permanent| {
                    permanent
                        .counters()
                        .map(|(kind, count)| (kind.clone(), count))
                        .collect()
                })
                .unwrap_or_default(),
            // The abilities of a planeswalker on the battlefield are printed on the card every seat can read.
            loyalty_abilities: loyalty_abilities(state, id)
                .into_iter()
                .map(|(ability, cost)| LoyaltyAbilityView { ability, cost })
                .collect(),
            attached_to: state.host_of(id),
            attachments: if object.permanent().is_some() {
                attachments()
            } else {
                Vec::new()
            },
            face_down: object.face_down().is_some(),
            face: object.face().filter(|_| printed),
        })
    }

    /// A face-down object as a seat that may not look at it sees it: where it is and whose it is, with no characteristics.
    fn face_down(state: &GameState, object: &GameObject) -> ObjectView {
        ObjectView {
            id: object.id(),
            owner: object.owner(),
            controller: controller_of(state, object.id()).unwrap_or_else(|| object.controller()),
            zone: object.zone(),
            name: String::new(),
            mana_cost: None,
            supertypes: Vec::new(),
            types: Vec::new(),
            subtypes: Vec::new(),
            colors: Vec::new(),
            power: None,
            toughness: None,
            abilities: Vec::new(),
            keywords: Vec::new(),
            permanent: None,
            kind: ObjectKind::Card,
            source: None,
            targets: Vec::new(),
            counters: Vec::new(),
            loyalty_abilities: Vec::new(),
            attached_to: None,
            attachments: Vec::new(),
            face_down: true,
            face: None,
        }
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
            mana: player.mana().clone(),
            counters: player
                .counters()
                .map(|(kind, count)| (kind.clone(), count))
                .collect(),
            mulligans: player.mulligans(),
            mulligan: player.mulligan_status(),
            hand: zone_view(state, viewer, Zone::Hand(seat)),
            library: zone_view(state, viewer, Zone::Library(seat)),
            revealed_top: state.revealed_top(seat),
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
            house_rules: Some(state.house_rules().clone()).filter(|rules| !rules.is_empty()),
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
    let attached = state.attachment_index();
    let sight = Sight::new(state, viewer);
    state
        .objects()
        .filter_map(|o| {
            ObjectView::with_attachments(state, &sight, o.id(), || {
                attached.get(&o.id()).cloned().unwrap_or_default()
            })
        })
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
