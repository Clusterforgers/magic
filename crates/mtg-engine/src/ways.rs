//! The ways a card can be cast from where it is (CR 601.2b, 601.3, 118.9): its mana cost from a hand, an alternative
//! cost a keyword gives (flashback, evoke, overload), and what a cast permission lets a player do (cast from another
//! zone, without paying the mana cost, as though the card had flash).
//!
//! `of` lists them for a card and a player; the options of a player offer a card when one of its ways can be used now
//! (`castable_now`), and the announcement of the cast asks which one when there is more than one (`announce/alternative.rs`).

use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::definition::Layout;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_cards::value::{Amount, CastWay};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use crate::announce::{self, Granted};
use crate::attach;
use crate::choose::announceable;
use crate::faces;
use crate::help;
use crate::keywords::{self, aftermath, foretell, keywords_of};
use crate::mana::{ManaOption, ManaPool, can_afford, can_afford_with};
use crate::mana_takes::for_card;
use crate::object::{FaceDown, GameObject, ObjectKind};
use crate::parts::Part;
use crate::price::Price;
use crate::priority::{mana_options, sorcery_timing};
use crate::resolve::{spell_ability, zone_kind};
use crate::rules::cast_permissions;
use crate::spend::Spend;
use crate::state::GameState;
use crate::zones::Zone;

/// One way a card can be cast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Way {
    /// The alternative cost, or `None` for the mana cost printed on the card.
    pub(crate) alternative: Option<CastWay>,
    /// What the player reads: the cost, or the permission.
    pub(crate) label: String,
    /// The mana paid instead of the printed cost; `None` for the printed cost itself.
    pub(crate) mana: Option<ManaCost>,
    /// The card may be cast whenever its caster has priority, as an instant is.
    pub(crate) instant: bool,
    /// What paying the way takes besides mana: life and the objects the player picks.
    pub(crate) parts: Vec<Part>,
}

impl Way {
    /// The mana cost this way pays for `printed`.
    pub(crate) fn cost(&self, printed: Option<&ManaCost>) -> Option<ManaCost> {
        self.mana.clone().or_else(|| printed.cloned())
    }
}

/// The mana of the cost of a keyword when it is paid with mana alone: the engine pays no other part yet.
fn mana_only(cost: &mtg_cards::cost::Cost) -> Option<ManaCost> {
    let price = Price::of(cost).ok()?;
    if price.tap || price.moves_source() {
        return None;
    }
    price.mana
}

/// The mana and the parts of the cost of a keyword that is paid as a spell is cast (flashback, evoke, overload, madness):
/// the mana, and life and the objects the player picks. A cost that taps or loses its source, counts X or removes counters
/// is not paid; nor is one with nothing to pay.
fn cast_cost(cost: &mtg_cards::cost::Cost) -> Option<(ManaCost, Vec<Part>)> {
    let price = Price::announced(cost).ok()?;
    let unpaid = price.tap
        || price.moves_source()
        || price.x > 0
        || (price.mana.is_none() && price.parts.is_empty())
        || price
            .parts
            .iter()
            .any(|part| part.is_counted() || matches!(part, Part::Counters { .. }));
    (!unpaid).then(|| (price.mana.unwrap_or(ManaCost::ZERO), price.parts))
}

/// A cost in words: its mana, then its parts.
fn cost_words(mana: &ManaCost, parts: &[Part]) -> String {
    let mut words: Vec<String> = Vec::new();
    if mana.mana_value() > 0 || parts.is_empty() {
        words.push(mana.to_string());
    }
    words.extend(parts.iter().map(Part::text));
    words.join(", ")
}

/// Whether `player` can give what the parts of a way take besides mana for the card `id`.
fn parts_payable(state: &GameState, player: PlayerId, id: ObjectId, parts: &[Part]) -> bool {
    parts.is_empty()
        || Price::casting(parts.to_vec())
            .claimed(state, player, id)
            .is_some()
}

/// Whether the card is a split card as a whole, which is never cast: one of its halves is (CR 709.3a).
fn both_halves(card: &GameObject) -> bool {
    card.whole_card().layout == Layout::Split && card.face().is_none()
}

/// Every way `player` may cast the card `id` from where it is, whatever the timing and the mana in hand.
pub(crate) fn of(state: &GameState, player: PlayerId, id: ObjectId) -> Vec<Way> {
    let Some(card) = state.object(id) else {
        return Vec::new();
    };
    if card.kind() != ObjectKind::Card || card.base_types().contains(CardType::Land) {
        return Vec::new();
    }
    let zone = card.zone();
    let allowed = match zone {
        Zone::Hand(owner) | Zone::Graveyard(owner) | Zone::Command(owner) => owner == player,
        // Only the top card of a library may be cast, by a permission.
        Zone::Library(owner) => owner == player && state.zone(zone).last() == Some(&id),
        Zone::Exile => true,
        Zone::Battlefield | Zone::Stack => false,
    };
    if !allowed {
        return Vec::new();
    }
    // The half with aftermath is cast from a graveyard and from no other zone (CR 702.127a).
    let by_aftermath = aftermath::has(state, id);
    if by_aftermath && !matches!(zone, Zone::Graveyard(_)) {
        return Vec::new();
    }
    // A split card is cast as one of its halves, never as both (CR 709.3a): its halves are tried one at a time.
    if both_halves(card) {
        return Vec::new();
    }
    let in_hand = matches!(zone, Zone::Hand(_));
    // A commander is cast from its owner's command zone for its mana cost and the tax (CR 903.8).
    let from_command = matches!(zone, Zone::Command(_)) && state.is_commander(id);
    // The creature card of an Adventure that was exiled on an adventure (CR 715.4).
    let on_adventure =
        zone == Zone::Exile && card.on_adventure() == Some(player) && card.face().is_none();
    let permissions: Vec<_> = cast_permissions(state)
        .iter()
        .filter(|permission| permission.covers(state, player, card))
        .collect();
    let types = card.base_types();
    let instant = types.contains(CardType::Instant)
        || keywords::grants_instant_timing(state, id)
        || permissions.iter().any(|permission| permission.flash);
    let mut ways = Vec::new();
    let printed = card.card().mana_cost.as_ref();
    // Cast for its mana cost from a hand, or from the zone a permission names.
    let by_permission = permissions
        .iter()
        .any(|permission| !permission.free && permission.from == Some(zone_kind(zone)));
    if (in_hand || from_command || by_permission || on_adventure || by_aftermath)
        && let Some(cost) = printed
    {
        ways.push(Way {
            alternative: None,
            label: format!("Cast for {cost}"),
            mana: None,
            instant,
            parts: Vec::new(),
        });
    }
    if permissions.iter().any(|permission| permission.free) {
        ways.push(Way {
            alternative: Some(CastWay::Free),
            label: "Cast without paying its mana cost".to_owned(),
            mana: Some(ManaCost::default()),
            instant,
            parts: Vec::new(),
        });
    }
    for keyword in keywords_of(state, id) {
        if let Some(way) = keyword_way(state, player, id, keyword, instant)
            && !ways.iter().any(|w| w.alternative == way.alternative)
        {
            ways.push(way);
        }
    }
    ways
}

/// The alternative cost the keyword gives the card `id` for casting it where it is, when it gives one.
fn keyword_way(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    keyword: &Keyword,
    instant: bool,
) -> Option<Way> {
    let zone = state.object(id)?.zone();
    let in_hand = matches!(zone, Zone::Hand(_));
    let in_graveyard = matches!(zone, Zone::Graveyard(_));
    let (way, name, cost, from_here) = match keyword {
        Keyword::Flashback(cost) => (CastWay::Flashback, "Flashback", cost, in_graveyard),
        Keyword::Escape(cost) => (CastWay::Escape, "Escape", cost, in_graveyard),
        Keyword::JumpStart if in_graveyard => {
            return discard_way(state, id, CastWay::JumpStart, instant);
        }
        Keyword::Retrace if in_graveyard => {
            return discard_way(state, id, CastWay::Retrace, instant);
        }
        Keyword::Evoke(cost) => (CastWay::Evoke, "Evoke", cost, in_hand),
        Keyword::Overload(cost) => (CastWay::Overload, "Overload", cost, in_hand),
        Keyword::Dash(cost) => (
            CastWay::Dash,
            "Dash",
            cost,
            in_hand && is_creature(state, id),
        ),
        Keyword::Bestow(cost) => (
            CastWay::Bestow,
            "Bestow",
            cost,
            in_hand && is_creature(state, id),
        ),
        Keyword::Blitz(cost) => (
            CastWay::Blitz,
            "Blitz",
            cost,
            in_hand && is_creature(state, id),
        ),
        Keyword::Foretell(cost) => (
            CastWay::Foretold,
            "Foretell",
            cost,
            foretold_before_now(state, player, id),
        ),
        _ => return None,
    };
    if !from_here {
        return None;
    }
    // Overload and foretell are paid with mana alone.
    let (mana, parts) = if matches!(
        way,
        CastWay::Flashback
            | CastWay::Evoke
            | CastWay::Escape
            | CastWay::Dash
            | CastWay::Bestow
            | CastWay::Blitz
    ) {
        cast_cost(cost)?
    } else {
        (mana_only(cost)?, Vec::new())
    };
    Some(Way {
        alternative: Some(way),
        label: format!("{name} {}", cost_words(&mana, &parts)),
        mana: Some(mana),
        instant,
        parts,
    })
}

/// Jump-start (CR 702.133) and retrace (CR 702.81): the card is cast from a graveyard for its mana cost and the discard of a
/// card, a land card for retrace. Nothing replaces the mana cost, so its X is announced as for a card cast from a hand.
fn discard_way(state: &GameState, id: ObjectId, way: CastWay, instant: bool) -> Option<Way> {
    let printed = state.object(id)?.card().mana_cost.as_ref()?;
    let (name, filter, words) = if way == CastWay::Retrace {
        (
            "Retrace",
            Some(Selector(vec![Pred::Type(CardType::Land)])),
            "discard a land card",
        )
    } else {
        ("Jump-start", None, "discard a card")
    };
    let price = Price::announced(&Cost(vec![CostItem::Discard {
        count: Amount::one(),
        filter,
    }]))
    .ok()?;
    Some(Way {
        alternative: Some(way),
        label: format!("{name} {printed}, {words}"),
        mana: None,
        instant,
        parts: price.parts,
    })
}

/// Whether the card `id` is a creature card: dash is an alternative cost for a creature spell only (CR 702.109a).
fn is_creature(state: &GameState, id: ObjectId) -> bool {
    state
        .object(id)
        .is_some_and(|card| card.base_types().contains(CardType::Creature))
}

/// Whether `player` foretold the card `id` on an earlier turn and it still lies face down in exile (CR 702.143a): the cast
/// for its foretell cost is open to them then, and never on the turn it was foretold in.
fn foretold_before_now(state: &GameState, player: PlayerId, id: ObjectId) -> bool {
    state.object(id).is_some_and(|card| {
        card.zone() == Zone::Exile
            && card.owner() == player
            && matches!(card.face_down(), Some(FaceDown::Foretold { turn }) if turn < state.turn().number())
    })
}

/// The way `player` casts the card `id` when a resolution lets them (CR 608.2g): for its mana cost, or without paying it.
/// A land cannot be cast, nor can a card that is on the battlefield or the stack.
pub(crate) fn granted(
    state: &GameState,
    id: ObjectId,
    free: bool,
    way: Option<CastWay>,
) -> Vec<Way> {
    let Some(card) = state.object(id) else {
        return Vec::new();
    };
    let castable = card.kind() == ObjectKind::Card
        && !card.base_types().contains(CardType::Land)
        && !matches!(card.zone(), Zone::Battlefield | Zone::Stack);
    if !castable
        || both_halves(card)
        || (aftermath::has(state, id) && !matches!(card.zone(), Zone::Graveyard(_)))
    {
        return Vec::new();
    }
    if let Some(named @ (CastWay::Madness | CastWay::Miracle)) = way {
        // Miracle is cast from the hand the card was drawn into (CR 702.94a).
        if named == CastWay::Miracle && !matches!(card.zone(), Zone::Hand(_)) {
            return Vec::new();
        }
        return keyword_cost(state, id, named)
            .map(|(mana, parts)| Way {
                alternative: Some(named),
                label: format!("{} {}", way_name(named), cost_words(&mana, &parts)),
                mana: Some(mana),
                instant: true,
                parts,
            })
            .into_iter()
            .collect();
    }
    if free {
        return vec![Way {
            alternative: Some(way.unwrap_or(CastWay::Free)),
            label: "Cast without paying its mana cost".to_owned(),
            mana: Some(ManaCost::default()),
            instant: true,
            parts: Vec::new(),
        }];
    }
    card.card()
        .mana_cost
        .as_ref()
        .map(|cost| Way {
            alternative: None,
            label: format!("Cast for {cost}"),
            mana: None,
            instant: true,
            parts: Vec::new(),
        })
        .into_iter()
        .collect()
}

/// The time counters and the mana of the suspend cost of the card `id` (CR 702.62), when the cost is paid with mana alone.
pub(crate) fn suspend_of(state: &GameState, id: ObjectId) -> Option<(u32, ManaCost)> {
    keywords_of(state, id).into_iter().find_map(|keyword| {
        let Keyword::Suspend { time, cost } = keyword else {
            return None;
        };
        Some((*time, mana_only(cost)?))
    })
}

/// Whether `player` can suspend the card `id` now: it is in their hand and could be cast at this time (CR 702.62a, whether
/// or not it has a legal target), and they can pay the suspend cost with the mana that floats and what they control.
pub(crate) fn can_suspend(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    pool: &ManaPool,
    sources: &[ManaOption],
    life: i32,
) -> bool {
    let Some(card) = state.object(id) else {
        return false;
    };
    let Some((_, cost)) = suspend_of(state, id) else {
        return false;
    };
    if card.kind() != ObjectKind::Card
        || card.base_types().contains(CardType::Land)
        || card.zone() != Zone::Hand(player)
    {
        return false;
    }
    let instant = card.base_types().contains(CardType::Instant)
        || keywords::grants_instant_timing(state, id)
        || cast_permissions(state)
            .iter()
            .any(|permission| permission.flash && permission.covers(state, player, card));
    (instant || sorcery_timing(state, player))
        && can_afford(
            &cost,
            pool,
            &for_card(state, sources, id),
            life,
            &Spend::other(),
        )
}

/// The mana the special action of foretell costs for the card `id` (CR 702.143a), when it has the keyword.
pub(crate) fn foretell_action_cost(state: &GameState, id: ObjectId) -> Option<ManaCost> {
    keywords_of(state, id)
        .into_iter()
        .any(|keyword| matches!(keyword, Keyword::Foretell(_)))
        .then(foretell::action_cost)
}

/// Whether `player` can foretell the card `id` now: it is in their hand, it is their turn, and they can pay {2} with the
/// mana that floats and what they control (CR 702.143a). Priority is theirs when the options are asked, and the timing is
/// any time during their turn, so the step and the stack do not matter.
pub(crate) fn can_foretell(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    pool: &ManaPool,
    sources: &[ManaOption],
    life: i32,
) -> bool {
    let Some(card) = state.object(id) else {
        return false;
    };
    let Some(cost) = foretell_action_cost(state, id) else {
        return false;
    };
    card.kind() == ObjectKind::Card
        && card.zone() == Zone::Hand(player)
        && state.turn().active() == player
        && can_afford(
            &cost,
            pool,
            &for_card(state, sources, id),
            life,
            &Spend::other(),
        )
}

/// The way and the mana of the madness or miracle cost of the card `id` (CR 702.35, 702.94) when a resolution casts it
/// in that way and the card has a cost the engine pays.
pub(crate) fn alternative_cost(
    state: &GameState,
    id: ObjectId,
    way: Option<CastWay>,
) -> Option<(CastWay, ManaCost)> {
    let way = way.filter(|way| matches!(way, CastWay::Madness | CastWay::Miracle))?;
    keyword_cost(state, id, way).map(|(mana, _)| (way, mana))
}

/// The name a cost-bearing way goes by in a label.
const fn way_name(way: CastWay) -> &'static str {
    match way {
        CastWay::Miracle => "Miracle",
        _ => "Madness",
    }
}

/// The mana and the other parts of the cost of the keyword that names `way` on the card `id` (madness or miracle).
fn keyword_cost(state: &GameState, id: ObjectId, way: CastWay) -> Option<(ManaCost, Vec<Part>)> {
    keywords_of(state, id)
        .into_iter()
        .find_map(|keyword| match (way, keyword) {
            (CastWay::Madness, Keyword::Madness(cost))
            | (CastWay::Miracle, Keyword::Miracle(cost)) => cast_cost(cost),
            _ => None,
        })
}

/// Whether a resolution can have `player` cast the card `id` now: its targets can be announced and, unless it is cast
/// free, the mana can be paid with what floats and what they control. Cost increases count even for a free cast.
pub(crate) fn grantable(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    free: bool,
    way: Option<CastWay>,
) -> bool {
    // Players cannot cast spells while a spell with split second is on the stack, whatever lets them (CR 702.61a, 101.2).
    if crate::keywords::split_second_on_stack(state) {
        return false;
    }
    if faces::is_multi_face(state, id) {
        return !faces::grantable_faces(state, player, id, Granted { free, way }).is_empty();
    }
    grantable_as_is(state, player, id, free, way)
}

/// [`grantable`] for the card as it stands: a card with several faces is judged as the face it is in use as.
pub(crate) fn grantable_as_is(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    free: bool,
    way: Option<CastWay>,
) -> bool {
    let Some(card) = state.object(id) else {
        return false;
    };
    let Some(seat) = state.player(player) else {
        return false;
    };
    let all = mana_options(state, player);
    let sources = for_card(state, &all, id);
    let spend = Spend::for_spell(state, id, card.card().x_in_cost > 0);
    let help = help::offered(state, player, id);
    granted(state, id, free, way).iter().any(|way| {
        announceable_in(state, player, id, way.alternative)
            && parts_payable(state, player, id, &way.parts)
            && way
                .cost(card.card().mana_cost.as_ref())
                .is_some_and(|cost| {
                    announce::payable(
                        state,
                        player,
                        id,
                        way.alternative.is_some_and(CastWay::replaces_cost),
                        &cost,
                        |total| {
                            can_afford_with(
                                total,
                                seat.mana(),
                                &sources,
                                help.help(),
                                seat.life(),
                                &spend,
                            )
                        },
                    )
                })
    })
}

/// Whether the targets of the card `id` can be announced when it is cast in `way`: an overloaded spell has none.
pub(crate) fn announceable_in(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    way: Option<CastWay>,
) -> bool {
    if way == Some(CastWay::Bestow) {
        // The creature card is an Aura spell with the one target of its enchant ability and no other (CR 702.103b).
        return attach::bestow_announceable(state, player, id);
    }
    way == Some(CastWay::Overload)
        || (spell_ability(state, id)
            .is_none_or(|(_, rules)| announceable(state, player, id, &rules))
            && attach::announceable(state, player, id))
}

/// Whether `player` can cast `id` now in some way: at a time the way allows, with targets to announce and mana to pay
/// with what floats and what they control.
pub(crate) fn castable_now(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    pool: &ManaPool,
    sources: &[ManaOption],
    life: i32,
) -> bool {
    if faces::is_multi_face(state, id) {
        return !faces::castable_faces(state, player, id, pool, sources, life).is_empty();
    }
    castable_as_is(state, player, id, pool, sources, life)
}

/// [`castable_now`] for the card as it stands: a card with several faces is judged as the face it is in use as.
pub(crate) fn castable_as_is(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    pool: &ManaPool,
    sources: &[ManaOption],
    life: i32,
) -> bool {
    let Some(card) = state.object(id) else {
        return false;
    };
    let printed_spend = Spend::for_spell(state, id, card.card().x_in_cost > 0);
    let sources = &for_card(state, sources, id);
    let help = help::offered(state, player, id);
    of(state, player, id).iter().any(|way| {
        // A bestowed spell is an Aura spell, whose mana restrictions read it so.
        let spend = if way.alternative == Some(CastWay::Bestow) {
            printed_spend.clone().bestowed()
        } else {
            printed_spend.clone()
        };
        (way.instant || sorcery_timing(state, player))
            && announceable_in(state, player, id, way.alternative)
            && parts_payable(state, player, id, &way.parts)
            && way
                .cost(card.card().mana_cost.as_ref())
                .is_some_and(|cost| {
                    announce::payable(
                        state,
                        player,
                        id,
                        way.alternative.is_some_and(CastWay::replaces_cost),
                        &cost,
                        |total| can_afford_with(total, pool, sources, help.help(), life, &spend),
                    )
                })
    })
}

/// The cards `player` may be able to cast, in the zones other than the hand: a graveyard, exile, the top of the library.
pub(crate) fn elsewhere(state: &GameState, player: PlayerId) -> Vec<ObjectId> {
    let mut found: Vec<ObjectId> = state.zone(Zone::Graveyard(player)).to_vec();
    found.extend(state.zone(Zone::Exile).iter().copied());
    found.extend(state.zone(Zone::Library(player)).last().copied());
    found.extend(state.zone(Zone::Command(player)).iter().copied());
    found
}

/// The lands in exile, in the graveyard and on top of the library of `player` that they may play (CR 305.1) under a cast
/// permission that lets them play the cards it covers. The timing and the land plays left are those of any land, judged
/// by the caller.
pub(crate) fn lands_elsewhere(state: &GameState, player: PlayerId) -> Vec<ObjectId> {
    let permissions: Vec<_> = cast_permissions(state)
        .iter()
        .filter(|permission| permission.play)
        .collect();
    if permissions.is_empty() {
        return Vec::new();
    }
    elsewhere(state, player)
        .into_iter()
        .filter(|id| {
            state.object(*id).is_some_and(|card| {
                card.kind() == ObjectKind::Card
                    && card.base_types().contains(CardType::Land)
                    && permissions
                        .iter()
                        .any(|permission| permission.plays(state, player, card))
            })
        })
        .collect()
}
