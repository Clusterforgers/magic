//! The ways a card can be cast from where it is (CR 601.2b, 601.3, 118.9): its mana cost from a hand, an alternative
//! cost a keyword gives (flashback, evoke, overload), and what a cast permission lets a player do (cast from another
//! zone, without paying the mana cost, as though the card had flash).
//!
//! `of` lists them for a card and a player; the options of a player offer a card when one of its ways can be used now
//! (`castable_now`), and the announcement of the cast asks which one when there is more than one (`announce/alternative.rs`).

use mtg_cards::keyword::Keyword;
use mtg_cards::value::CastWay;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use crate::attach;
use crate::choose::announceable;
use crate::keywords::{self, keywords_of};
use crate::mana::{ManaOption, ManaPool, can_afford};
use crate::spend::Spend;
use crate::object::ObjectKind;
use crate::price::Price;
use crate::priority::sorcery_timing;
use crate::resolve::{spell_ability, zone_kind};
use crate::rules::cast_permissions;
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
    let in_hand = matches!(zone, Zone::Hand(_));
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
    if (in_hand || by_permission)
        && let Some(cost) = printed
    {
        ways.push(Way {
            alternative: None,
            label: format!("Cast for {cost}"),
            mana: None,
            instant,
        });
    }
    if permissions.iter().any(|permission| permission.free) {
        ways.push(Way {
            alternative: Some(CastWay::Free),
            label: "Cast without paying its mana cost".to_owned(),
            mana: Some(ManaCost::default()),
            instant,
        });
    }
    for keyword in keywords_of(state, id) {
        let (way, name, cost, from_here) = match keyword {
            Keyword::Flashback(cost) => (
                CastWay::Flashback,
                "Flashback",
                cost,
                matches!(zone, Zone::Graveyard(_)),
            ),
            Keyword::Evoke(cost) => (CastWay::Evoke, "Evoke", cost, in_hand),
            Keyword::Overload(cost) => (CastWay::Overload, "Overload", cost, in_hand),
            _ => continue,
        };
        if !from_here || ways.iter().any(|w| w.alternative == Some(way)) {
            continue;
        }
        let Some(mana) = mana_only(cost) else {
            continue;
        };
        ways.push(Way {
            alternative: Some(way),
            label: format!("{name} {mana}"),
            mana: Some(mana),
            instant,
        });
    }
    ways
}

/// Whether the targets of the card `id` can be announced when it is cast in `way`: an overloaded spell has none.
pub(crate) fn announceable_in(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    way: Option<CastWay>,
) -> bool {
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
    let Some(card) = state.object(id) else {
        return false;
    };
    let spend = Spend::for_spell(state, id, card.card().x_in_cost > 0);
    of(state, player, id).iter().any(|way| {
        (way.instant || sorcery_timing(state, player))
            && announceable_in(state, player, id, way.alternative)
            && way
                .cost(card.card().mana_cost.as_ref())
                .is_some_and(|cost| can_afford(&cost, pool, sources, life, &spend))
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
