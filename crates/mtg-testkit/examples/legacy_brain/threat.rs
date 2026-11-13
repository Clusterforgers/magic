//! The threat model: what a permanent is worth as a danger beyond its body (card-advantage and token engines, tutors,
//! combo pieces, commanders), what a card does as interaction (counters, removal, wipes, burn, fogs, tricks), how much
//! keeping interaction in hand is worth, and how much each player threatens the seat's team.

use std::cell::RefCell;
use std::collections::HashMap;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};

use super::eval::{creature_value, permanent_value};
use super::seen::{Body, Seen};

/// What a card does, read once from its rules.
// Independent facts about one card.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Traits {
    /// A permanent ability that draws cards again and again.
    pub draws: bool,
    /// A permanent ability that makes tokens again and again.
    pub tokens: bool,
    /// It searches a library.
    pub tutor: bool,
    /// A spell that counters a spell or ability.
    pub counter: bool,
    /// It destroys, exiles or damages one target.
    pub removal: bool,
    /// A spell that destroys or exiles many permanents without a target.
    pub wipe: bool,
    /// It deals damage.
    pub burn: bool,
    /// A spell that prevents damage or changes a rule without a target (a fog).
    pub fog: bool,
    /// A spell that changes a target's characteristics until the turn ends (a pump, protection).
    pub trick: bool,
    /// It can be cast at instant speed (an instant or a card with flash).
    pub fast: bool,
    /// It is a permanent card.
    pub permanent: bool,
}

thread_local! {
    /// Traits by card name, read once per thread.
    static TRAITS: RefCell<HashMap<String, Traits>> = RefCell::new(HashMap::new());
}

/// The traits of `card`, read from its rules (and kept by name).
pub fn traits(card: &CardDefinition) -> Traits {
    if let Some(found) = TRAITS.with(|cache| cache.borrow().get(&card.name).copied()) {
        return found;
    }
    let read = read_traits(card);
    TRAITS.with(|cache| cache.borrow_mut().insert(card.name.clone(), read));
    read
}

/// [`traits`] read afresh.
fn read_traits(card: &CardDefinition) -> Traits {
    let instant = card.types.contains(&CardType::Instant);
    let mut traits = Traits {
        permanent: !instant && !card.types.contains(&CardType::Sorcery),
        ..Traits::default()
    };
    for rule in &card.rules {
        let text = format!("{:?}", rule.effects());
        let has = |word: &str| text.contains(word);
        match rule.as_ref() {
            Ability::Keyword(Keyword::Flash) => traits.fast = true,
            Ability::Spell { targets, .. } => {
                traits.counter |= has("Counter {");
                traits.tutor |= has("Search {");
                traits.burn |= has("DealDamage");
                if targets.is_empty() {
                    traits.wipe |= has("Destroy {") || has("Exile {");
                    traits.fog |= has("Rule") || has("Shield");
                } else {
                    traits.removal |= has("Destroy {") || has("Exile {") || has("DealDamage");
                    traits.trick |= has("Continuous");
                }
            }
            Ability::Triggered { targets, .. } | Ability::Activated { targets, .. }
                if !rule.is_mana_ability() =>
            {
                traits.draws |= has("Draw {");
                traits.tokens |= has("CreateToken");
                traits.tutor |= has("Search {");
                if !targets.is_empty() {
                    traits.removal |= has("Destroy {") || has("Exile {") || has("DealDamage");
                }
            }
            _ => {}
        }
    }
    traits.fast |= instant;
    traits
}

/// Pairs of cards that win or make infinite resources together: either half is worth far more beside the other.
const COMBOS: &[(&str, &str)] = &[
    ("Devoted Druid", "Vizier of Remedies"),
    ("Heliod, Sun-Crowned", "Walking Ballista"),
    ("Kiki-Jiki, Mirror Breaker", "Zealous Conscripts"),
    ("Kiki-Jiki, Mirror Breaker", "Pestermite"),
    ("Kiki-Jiki, Mirror Breaker", "Deceiver Exarch"),
    ("Splinter Twin", "Pestermite"),
    ("Splinter Twin", "Deceiver Exarch"),
    ("Thassa's Oracle", "Demonic Consultation"),
    ("Thassa's Oracle", "Tainted Pact"),
    ("Laboratory Maniac", "Demonic Consultation"),
    ("Sanguine Bond", "Exquisite Blood"),
    ("Niv-Mizzet, Parun", "Curiosity"),
    ("Isochron Scepter", "Dramatic Reversal"),
    ("Underworld Breach", "Brain Freeze"),
    ("Dockside Extortionist", "Temur Sabertooth"),
    ("Basalt Monolith", "Rings of Brighthearth"),
    ("Mikaeus, the Unhallowed", "Triskelion"),
    ("Food Chain", "Eternal Scourge"),
];

/// What a combo piece named `name` controlled by `controller` adds: much when its partner is on that player's
/// battlefield, a little alone.
fn combo_value(seen: &Seen<'_>, name: &str, controller: PlayerId, bodies: &[Body]) -> f64 {
    let partners: Vec<&str> = COMBOS
        .iter()
        .filter_map(|(a, b)| {
            if *a == name {
                Some(*b)
            } else if *b == name {
                Some(*a)
            } else {
                None
            }
        })
        .collect();
    if partners.is_empty() {
        return 0.0;
    }
    let assembled = bodies.iter().any(|body| {
        body.controller == controller
            && seen
                .card(body.id)
                .is_some_and(|card| partners.contains(&card.name.as_str()))
    });
    if assembled { 25.0 } else { 3.0 }
}

/// What `body` is worth as a danger beyond its body and cost: engines, tutors, combo pieces and commanders.
pub fn engine_value(seen: &Seen<'_>, body: &Body, bodies: &[Body]) -> f64 {
    let Some(card) = seen.card(body.id) else {
        return 0.0;
    };
    let traits = traits(card);
    let mut value = 0.0;
    if traits.draws {
        value += 7.0;
    }
    if traits.tokens {
        value += 4.0;
    }
    if traits.tutor {
        value += 3.0;
    }
    if traits.removal && traits.permanent {
        value += 2.0;
    }
    value += combo_value(seen, &card.name, body.controller, bodies);
    if seen.is_commander(body.id) {
        let dealt = seen
            .alive()
            .into_iter()
            .filter(|player| seen.is_teammate(*player))
            .map(|player| seen.commander_damage(body.id, player))
            .max()
            .unwrap_or(0);
        value += 4.0 + f64::from(dealt) * 0.6;
    }
    value
}

/// What `body` threatens all told: its value to its controller ([`permanent_value`]) and [`engine_value`].
pub fn object_threat(seen: &Seen<'_>, body: &Body, bodies: &[Body]) -> f64 {
    let base = if body.is_creature() {
        creature_value(body)
    } else {
        permanent_value(body)
    };
    base + engine_value(seen, body, bodies)
}

/// What the spell `id` on the stack threatens the seat's team with if it resolves: a permanent spell its
/// [`object_threat`], a wipe what the team's creatures are worth beyond the opponents', anything else a share of its
/// mana value.
pub fn spell_threat(seen: &Seen<'_>, id: ObjectId) -> f64 {
    let Some(card) = seen.card(id) else {
        return 0.0;
    };
    let traits = traits(card);
    let bodies = seen.bodies();
    if traits.permanent {
        return seen
            .body(id)
            .map_or(0.0, |body| object_threat(seen, &body, &bodies));
    }
    if traits.wipe {
        let ours: f64 = bodies
            .iter()
            .filter(|body| body.is_creature())
            .map(|body| {
                let worth = object_threat(seen, body, &bodies);
                if seen.is_teammate(body.controller) {
                    worth
                } else {
                    -worth
                }
            })
            .sum();
        return ours.max(0.0);
    }
    3.0 * f64::from(seen.mana_value(id))
}

/// How much interaction of `card`'s kind is worth keeping, as a share of [`hold_value`]: wipes most, then counters,
/// removal, fogs, burn and tricks; `None` for a card that is not interaction.
pub fn interaction(card: &CardDefinition) -> Option<f64> {
    let traits = traits(card);
    if traits.wipe {
        Some(1.4)
    } else if traits.counter {
        Some(1.0)
    } else if traits.removal && !traits.permanent {
        Some(0.8)
    } else if traits.fog {
        Some(0.6)
    } else if traits.burn {
        Some(0.5)
    } else if traits.trick {
        Some(0.35)
    } else {
        None
    }
}

/// What one piece of interaction in the seat's hand is worth beyond a card: more the more cards the opponents hold
/// that could still bring threats, up to [`HOLD_CARDS`] of them.
pub fn hold_value(seen: &Seen<'_>) -> f64 {
    let cards: usize = seen
        .opponents()
        .into_iter()
        .filter(|player| seen.is_alive(*player))
        .map(|player| seen.hand_size(player))
        .sum();
    HOLD_BASE + HOLD_PER_CARD * count(cards.min(HOLD_CARDS))
}

/// [`hold_value`] with no card in the opponents' hands.
const HOLD_BASE: f64 = 4.0;
/// What each card in the opponents' hands adds to [`hold_value`].
const HOLD_PER_CARD: f64 = 1.6;
/// The opponents' cards counted by [`hold_value`].
const HOLD_CARDS: usize = 5;
/// The share of its worth each further piece of interaction in hand keeps: the first answers the biggest threat, the
/// next ones smaller ones.
const HOLD_DECAY: f64 = 0.5;

/// What the interaction in the seat's own hand is worth beyond its cards ([`interaction`], [`hold_value`]): the most
/// valuable piece in full, each further one [`HOLD_DECAY`] of the one before, all at `share`.
pub fn held_interaction(seen: &Seen<'_>, share: f64) -> f64 {
    let hold = hold_value(seen);
    let mut factors: Vec<f64> = seen
        .own_hand()
        .iter()
        .filter_map(|id| seen.card(*id))
        .filter_map(interaction)
        .collect();
    factors.sort_by(|a, b| b.total_cmp(a));
    let mut keep = 1.0;
    let mut worth = 0.0;
    for factor in factors {
        worth += factor * hold * share * keep;
        keep *= HOLD_DECAY;
    }
    worth
}

/// The seat's mana sources that are untapped now.
pub fn open_mana(seen: &Seen<'_>, bodies: &[Body]) -> u32 {
    let open = bodies
        .iter()
        .filter(|body| {
            body.controller == seen.seat() && !body.tapped && (body.is_land() || body.makes_mana)
        })
        .count();
    u32::try_from(open).unwrap_or(u32::MAX)
}

/// The instant-speed interaction in the seat's hand other than `except`, as (mana value, share of [`hold_value`]).
pub fn fast_interaction(seen: &Seen<'_>, except: Option<ObjectId>) -> Vec<(u32, f64)> {
    seen.own_hand()
        .iter()
        .filter(|id| Some(**id) != except)
        .filter_map(|id| {
            let card = seen.card(*id)?;
            let factor = interaction(card)?;
            traits(card).fast.then(|| (seen.mana_value(*id), factor))
        })
        .collect()
}

/// How much `player` threatens the seat's team: board power and engines, cards in hand, mana and life.
pub fn player_threat(seen: &Seen<'_>, player: PlayerId, bodies: &[Body]) -> f64 {
    let mut threat = 0.0;
    let mut sources = 0;
    for body in bodies.iter().filter(|body| body.controller == player) {
        if body.is_land() || body.makes_mana {
            sources += 1;
        }
        if !body.is_land() {
            threat += object_threat(seen, body, bodies);
        }
    }
    threat
        + 2.2 * count(seen.hand_size(player).min(8))
        + 0.8 * count(sources)
        + 0.25 * f64::from(seen.life(player).max(0))
}

/// A small count as a float.
fn count(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}
