//! Building a viewer's [`StateView`] from the full game state, and the visibility rule it rests on.

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::compile::protection_quality_word;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::{CardType, CardTypes, Subtype};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::characteristics::{characteristics, controller_of};
use mtg_engine::combat::Attacker;
use mtg_engine::companions::CompanionStage;
use mtg_engine::forecast::pending_combat_forecast;
use mtg_engine::gives::gives;
use mtg_engine::keywords::{keywords_of, summoning_sick};
use mtg_engine::loyalty::loyalty_abilities;
use mtg_engine::object::{FaceDown, GameObject, ObjectKind};
use mtg_engine::player::{Player, PlayerStatus};
use mtg_engine::resolve::targets_legal;
use mtg_engine::sight::offered_to;
use mtg_engine::state::{GameState, Stage};
use mtg_engine::zones::Zone;

use crate::view::{
    AttackerView, CombatView, CommanderDamageView, CommanderView, CompanionView, CopyView,
    GivesView, LoyaltyAbilityView, MergedCardView, ObjectView, PermanentView, PlayerView,
    PriorityView, StateView, TableView, TurnView, ZoneView,
};
use crate::wording::activated_views;

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
        Zone::Battlefield
        | Zone::PhasedOut
        | Zone::Graveyard(_)
        | Zone::Stack
        | Zone::Exile
        | Zone::Command(_) => true,
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
        let offered = offered_to(state, viewer);
        Sight {
            state,
            viewer,
            offered,
        }
    }

    /// Whether the viewer reads the rules of the object: not those of a card in another seat's hand or library, even one
    /// a reveal showed them.
    pub(crate) fn reads_rules_of(&self, object: &GameObject) -> bool {
        !matches!(object.zone(), Zone::Hand(_) | Zone::Library(_)) || object.owner() == self.viewer
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

/// What the attachment `id` gives its host now, the keywords by name; none while it gives nothing.
fn gives_view(state: &GameState, id: ObjectId) -> Option<GivesView> {
    let gives = gives(state, id)?;
    Some(GivesView {
        power: gives.power,
        toughness: gives.toughness,
        set: gives.set,
        keywords: given_keyword_names(&gives.keywords),
        rules: gives.rules,
    })
}

/// The names of granted keywords, protection with its qualities in oracle wording: "Protection from creatures", and
/// one name for protections granted one after the other ("Protection from red and from blue"). A quality without a
/// word stays "Protection".
fn given_keyword_names(keywords: &[Keyword]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut protecting = false;
    for keyword in keywords {
        let word = match keyword {
            Keyword::Protection(quality) => protection_quality_word(quality),
            _ => None,
        };
        match (word, names.last_mut()) {
            (Some(word), Some(last)) if protecting => {
                last.push_str(" and from ");
                last.push_str(word);
            }
            (Some(word), _) => names.push(format!("Protection from {word}")),
            (None, _) => names.push(keyword.name().to_owned()),
        }
        protecting = word.is_some();
    }
    names
}

/// The names of the keywords a card prints, each once: what the controller of a face-down permanent reads on its card.
fn printed_keyword_names(card: &CardDefinition) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for rules in &card.rules {
        if let Ability::Keyword(keyword) = &**rules {
            let name = keyword.name();
            if !names.iter().any(|seen| seen == name) {
                names.push(name.to_owned());
            }
        }
    }
    names
}

/// What the card prints, then what effects gave it; nothing for an ability on the stack.
fn shown_subtypes(card: &CardDefinition, now: &[Subtype], printed: bool) -> Vec<Subtype> {
    if !printed {
        return Vec::new();
    }
    let mut all = card.subtypes.clone();
    let given: Vec<Subtype> = now
        .iter()
        .filter(|subtype| !all.contains(subtype))
        .cloned()
        .collect();
    all.extend(given);
    all
}

fn type_list(types: CardTypes) -> Vec<CardType> {
    ALL_TYPES
        .into_iter()
        .filter(|ty| types.contains(*ty))
        .collect()
}

/// What the viewer reads of the copy effect an object is under, or of the spell it copies.
fn copy_view(state: &GameState, sight: &Sight<'_>, object: &GameObject) -> Option<CopyView> {
    object
        .copy_effect()
        .map(mtg_engine::copies::CopyEffect::of)
        .or_else(|| object.copied_from())
        .map(|original| CopyView {
            // The original is named only while the viewer can see that very object.
            of: Some(original.id).filter(|original_id| {
                state.object(*original_id).is_some_and(|seen| {
                    seen.counter() == original.counter && !matches!(sight.look(seen), Look::Hidden)
                })
            }),
        })
}

/// The counters on a permanent by kind, in the order of the kind; none for an object that is no permanent.
fn counters_view(object: &GameObject) -> Vec<(CounterKind, u32)> {
    object
        .permanent()
        .map(|permanent| {
            permanent
                .counters()
                .map(|(kind, count)| (kind.clone(), count))
                .collect()
        })
        .unwrap_or_default()
}

/// The cards of a merged permanent from the top down; every card of a face-up permanent is public (CR 729).
fn merged_view(object: &GameObject) -> Vec<MergedCardView> {
    object.merged().map_or_else(Vec::new, |merged| {
        merged
            .from_the_top()
            .into_iter()
            .map(|card| MergedCardView {
                id: card.id(),
                name: card.card().name.clone(),
            })
            .collect()
    })
}

/// The name the viewer reads on the object: the card's, except for an ability on the stack of a permanent that still lies
/// face down and that the viewer may not look at, which is named after its source, and a face-down permanent has no name
/// (CR 708.2a).
fn shown_name(state: &GameState, viewer: PlayerId, object: &GameObject, card_name: &str) -> String {
    let hidden = object.ability().is_some_and(|ability| {
        let source = ability.source();
        state.object(source.id).is_some_and(|source_object| {
            source_object.counter() == source.counter
                && source_object
                    .face_down()
                    .is_some_and(FaceDown::shows_characteristics)
                && !state.may_look_at(viewer, source.id)
        })
    });
    if hidden {
        String::new()
    } else {
        card_name.to_owned()
    }
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
        let look = sight.look(object);
        // A face-down creature shows every seat the characteristics of a 2/2 (CR 708.2a); the card behind it is the
        // controller's alone.
        let shows = object
            .face_down()
            .is_some_and(FaceDown::shows_characteristics);
        match look {
            Look::Hidden => return None,
            Look::FaceDown if !shows => return Some(Self::face_down(state, object)),
            Look::FaceDown | Look::Whole => {}
        }
        let chars = characteristics(state, id)?;
        let behind = shows && look == Look::Whole;
        let card = if behind {
            object.whole_card()
        } else {
            object.card()
        };
        // An ability on the stack is named after its source and has nothing else of the source's printed side.
        let printed = object.kind() != ObjectKind::Ability;
        // A face-down permanent's abilities are the 2/2's; the card behind it is read off `abilities`.
        let activated = if printed && !behind && sight.reads_rules_of(object) {
            activated_views(state, id, &chars.abilities, &card.name)
        } else {
            Vec::new()
        };
        Some(ObjectView {
            id,
            owner: object.owner(),
            controller: controller_of(state, id).unwrap_or_else(|| object.controller()),
            zone: object.zone(),
            name: shown_name(state, sight.viewer, object, &card.name),
            mana_cost: card.mana_cost.clone().filter(|_| printed),
            supertypes: card
                .supertypes
                .iter()
                .copied()
                .filter(|_| printed)
                .collect(),
            types: type_list(chars.types),
            subtypes: shown_subtypes(card, &chars.subtypes, printed),
            colors: chars.colors.iter().collect(),
            power: chars.power,
            toughness: chars.toughness,
            abilities: if behind {
                card.abilities.clone()
            } else {
                chars.abilities
            },
            keywords: if behind {
                printed_keyword_names(card)
            } else {
                keyword_names(state, id)
            },
            permanent: object.permanent().map(|p| PermanentView {
                tapped: p.tapped(),
                damage: p.damage(),
                // Haste excuses it, so the view says whether sickness holds it back, not only when it arrived.
                summoning_sick: summoning_sick(state, id),
                prepared: p.prepared(),
                level: chars
                    .subtypes
                    .iter()
                    .any(|subtype| subtype.0 == "Class")
                    .then(|| p.level()),
            }),
            kind: object.kind(),
            source: object.ability().map(|ability| ability.source().id),
            targets: object.choices().targets.clone(),
            targets_legal: targets_legal(state, id),
            // Counters and attachments are public facts of a permanent, shown to every seat that sees the permanent.
            counters: counters_view(object),
            // The abilities of a planeswalker on the battlefield are printed on the card every seat can read.
            loyalty_abilities: loyalty_abilities(state, id)
                .into_iter()
                .map(|(ability, cost)| LoyaltyAbilityView { ability, cost })
                .collect(),
            attached_to: state.host_of(id),
            attached_player: state.player_host_of(id),
            paired_with: state.partner_of(id),
            attachments: if object.permanent().is_some() {
                attachments()
            } else {
                Vec::new()
            },
            gives: gives_view(state, id),
            face_down: object.face_down().is_some(),
            face: object.face().filter(|_| printed),
            copy: copy_view(state, sight, object),
            merged: merged_view(object),
            // The clauses the engine leaves to the table are printed on the card every seat can read.
            manual: if printed {
                card.manual.clone()
            } else {
                Vec::new()
            },
            note: mtg_engine::sight::note_of(state, id).map(str::to_owned),
            identity: card.identity.clone().filter(|_| printed),
            activated,
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
            targets_legal: Vec::new(),
            counters: Vec::new(),
            loyalty_abilities: Vec::new(),
            attached_to: None,
            attached_player: None,
            // Who is paired with whom is told by the public `Paired` event, so a face-down permanent shows it too.
            paired_with: state.partner_of(object.id()),
            attachments: Vec::new(),
            gives: None,
            face_down: true,
            face: None,
            copy: None,
            merged: Vec::new(),
            manual: Vec::new(),
            note: None,
            identity: None,
            activated: Vec::new(),
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
            citys_blessing: state.has_citys_blessing(seat),
            enchanted_by: state.attachments_of_player(seat),
            mulligans: player.mulligans(),
            mulligan: player.mulligan_status(),
            hand: zone_view(state, viewer, Zone::Hand(seat)),
            library: zone_view(state, viewer, Zone::Library(seat)),
            revealed_top: state
                .revealed_top(seat)
                .or_else(|| mtg_engine::sight::revealed_top_by_hand(state, seat)),
            revealed_hand: mtg_engine::sight::revealed_hand(state, seat),
            known_top: if seat == viewer {
                mtg_engine::sight::known_top(state, seat)
            } else {
                Vec::new()
            },
            graveyard: zone_view(state, viewer, Zone::Graveyard(seat)),
            command: zone_view(state, viewer, Zone::Command(seat)),
            commanders: state
                .commanders()
                .filter(|id| state.object(*id).is_some_and(|card| card.owner() == seat))
                .map(|id| CommanderView {
                    object: id,
                    casts: state.commander_casts(id),
                    tax: state.commander_tax(id),
                })
                .collect(),
            commander_damage: state
                .commanders()
                .filter_map(|id| {
                    let damage = state.commander_damage(id, seat);
                    (damage > 0).then_some(CommanderDamageView {
                        commander: id,
                        name: state.commander_name(id).unwrap_or_default().to_owned(),
                        damage,
                    })
                })
                .collect(),
            commander_lethal: state.commander_lethal(),
            companion: companion_view(state, viewer, seat),
            bank: 0,
        }
    }
}

/// The companion of `owner` as `viewer` sees it: that it exists to everyone, which card it is to its owner and, once
/// revealed, to every seat, and the object it became once it is in the game.
fn companion_view(state: &GameState, viewer: PlayerId, owner: PlayerId) -> Option<CompanionView> {
    let info = state.companion(owner)?;
    let known = viewer == owner || info.stage != CompanionStage::Hidden;
    Some(CompanionView {
        stage: info.stage,
        name: known.then_some(info.name),
        object: (info.stage == CompanionStage::Brought).then_some(info.card),
    })
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
            spectator: is_spectator(state, viewer),
            format: state.format().name().to_owned(),
            house_rules: Some(state.house_rules().clone()).filter(|rules| !rules.is_empty()),
            stage: state.stage(),
            first_player: state.first_player(),
            turn: TurnView {
                number: turn.number(),
                active: turn.active(),
                active_left: state.stage() == Stage::Turns && state.active_player().is_none(),
                extra: turn.is_extra(),
                extra_turns: state
                    .extra_turns()
                    .iter()
                    .map(|extra| extra.player)
                    .collect(),
                step: turn.step(),
                lands_played: turn.lands_played(),
                day_night: state.day_night(),
                monarch: state.monarch(),
                initiative: state.w15a_initiative(),
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
            phased_out: state.zone(Zone::PhasedOut).to_vec(),
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

/// The viewer id of a spectator of the game `state`: the number of seats, which names no seat. Every view and event batch
/// built for it is a spectator's (`TableView::spectator`).
pub fn spectator_viewer(state: &GameState) -> PlayerId {
    PlayerId::from_index(state.players().len())
}

/// Whether `viewer` is no seat of the game `state`.
fn is_spectator(state: &GameState, viewer: PlayerId) -> bool {
    viewer.index() >= state.players().len()
}

impl StateView {
    /// Builds what `viewer` may know: their own hand, every public zone, and counts for the rest. A viewer that is no
    /// seat gets a spectator's view.
    pub fn for_player(state: &GameState, viewer: PlayerId) -> StateView {
        StateView {
            table: TableView::of(state, viewer),
            objects: visible_objects(state, viewer),
        }
    }

    /// Builds what a spectator may know: every public zone and the counts of the hands and libraries, with no seat's
    /// hidden cards. The table says it is a spectator's (`TableView::spectator`).
    pub fn for_spectator(state: &GameState) -> StateView {
        StateView::for_player(state, spectator_viewer(state))
    }
}
