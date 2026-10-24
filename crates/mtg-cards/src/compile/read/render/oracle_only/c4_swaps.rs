//! Words the cards of the fourth sample of the names G to L use where the renderers write another: "draws a card, then
//! discards a card" for each player, "It can't be blocked this turn." after a counter, "you already control" for an
//! entering Angel, "their hand" for the spell of an opponent, the pronoun "he" of a legend, "Put one of them into your
//! hand and the rest on the bottom" after a look at the top cards, "you may have this creature deal damage", the
//! energy then a payment, "a 1/1 creature", "Investigate X times." and a trailing "if" with its counters.

use crate::ability::Ability;
use crate::effect::{Effect, TokenRef};
use crate::modify::Modification;
use crate::refs::{Pred, Ref};
use crate::trigger::{ReplaceBy, ReplaceWhen, TriggerPattern};
use crate::value::Amount;

/// What an ability must have for a swap to apply.
type Guard = fn(&Ability) -> bool;

/// The swaps: where they apply, the renderers' words and the card's. They apply in this order, each to the wordings the
/// swaps before it made too.
const SWAPS: [(Guard, &str, &str); 15] = [
    (
        loots_each_player,
        " draws a card and discards a card",
        " draws a card, then discards a card",
    ),
    (
        blocks_after_counter,
        ". ~ can't be blocked this turn.",
        ". It can't be blocked this turn.",
    ),
    (
        counts_entering_group,
        " you control.",
        " you already control.",
    ),
    (
        opponent_casts_spell,
        "other than your hand,",
        "other than their hand,",
    ),
    (pumps_itself, ", it gets +", ", he gets +"),
    (
        looks_at_top_cards,
        "Put a card from among them into your hand. Put the rest on the bottom of your library in any order.",
        "Put one of them into your hand and the rest on the bottom of your library in any order.",
    ),
    (
        may_deal_damage,
        ", you may ~ deals damage",
        ", you may have ~ deal damage",
    ),
    (
        may_deal_damage,
        " counters on ~ to target",
        " counters on it to target",
    ),
    (energy_then_pays, " and you may pay ", ", then you may pay "),
    (energy_then_pays, "If you do, untap", "If you pay, untap"),
    (
        energy_then_pays,
        " and after this phase",
        ", and after this phase",
    ),
    (
        cannot_attack,
        "a creature with power 1 with toughness 1",
        "a 1/1 creature",
    ),
    (
        investigates_x,
        "Create X Clue tokens.",
        "Investigate X times.",
    ),
    (
        doubles_counters,
        "If X is 5 or more, draw a card.",
        "If X is 5 or more, you draw a card.",
    ),
    (
        doubles_counters,
        "on each of any number of target other creatures",
        "on any number of other target creatures",
    ),
];

/// Adds the wordings with each swap that applies, one swap after the other.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for (applies, from, to) in SWAPS {
        if applies(ability) {
            let found: Vec<String> = texts
                .iter()
                .filter(|text| text.contains(from))
                .map(|text| text.replacen(from, to, 1))
                .collect();
            texts.extend(found);
        }
    }
}

/// The rules as JSON, which the guards search for the shape they want.
fn json(ability: &Ability) -> String {
    serde_json::to_string(ability).unwrap_or_default()
}

/// "Each player draws a card, then discards a card."
fn loots_each_player(ability: &Ability) -> bool {
    matches!(
        ability.effects(),
        [
            Effect::Draw {
                who: Ref::EachPlayer,
                ..
            },
            Effect::Discard {
                who: Ref::EachPlayer,
                ..
            }
        ]
    )
}

/// A counter on the card, then "It can't be blocked this turn."
fn blocks_after_counter(ability: &Ability) -> bool {
    matches!(
        ability.effects(),
        [
            Effect::PutCounters { on: Ref::This, .. },
            Effect::Continuous {
                affected: Ref::This,
                mods,
                ..
            }
        ] if mods.as_slice() == [Modification::CantBeBlocked]
    )
}

/// Another creature of a kind enters with counters for the creatures of the kind that were there before.
fn counts_entering_group(ability: &Ability) -> bool {
    matches!(
        ability,
        Ability::Replacement {
            when: ReplaceWhen::Enters { what },
            by: ReplaceBy::WithCounters { .. },
            ..
        } if what.0.contains(&Pred::Other)
    )
}

/// A spell an opponent casts.
fn opponent_casts_spell(ability: &Ability) -> bool {
    matches!(
        ability,
        Ability::Triggered {
            when: TriggerPattern::SpellCast {
                by: Ref::Opponents,
                ..
            },
            ..
        }
    )
}

/// The card gives itself a bonus when it attacks.
fn pumps_itself(ability: &Ability) -> bool {
    matches!(
        ability,
        Ability::Triggered {
            when: TriggerPattern::Attacks { .. },
            effects,
            ..
        } if matches!(effects.as_slice(), [Effect::Continuous { affected: Ref::This, .. }])
    )
}

/// A look at the top cards, one of which goes to the hand.
fn looks_at_top_cards(ability: &Ability) -> bool {
    ability
        .effects()
        .iter()
        .any(|effect| matches!(effect, Effect::LookAtTop { .. }))
}

/// The card may deal damage by itself.
fn may_deal_damage(ability: &Ability) -> bool {
    matches!(
        ability.effects(),
        [Effect::May { effects, .. }] if matches!(effects.as_slice(), [Effect::DealDamage { source: Some(Ref::This), .. }])
    )
}

/// Energy, then a payment of it.
fn energy_then_pays(ability: &Ability) -> bool {
    matches!(
        ability.effects(),
        [Effect::PutCounters { on: Ref::You, .. }, Effect::Pay { .. }]
    )
}

/// A static restriction on attacking.
fn cannot_attack(ability: &Ability) -> bool {
    matches!(ability, Ability::Static { mods, .. } if mods.as_slice() == [Modification::CantAttack])
}

/// Clue tokens, counted by X.
fn investigates_x(ability: &Ability) -> bool {
    matches!(
        ability.effects(),
        [Effect::CreateToken { token: TokenRef::Named(name), count: Amount::X, .. }] if name == "clue"
    )
}

/// A step that doubles the counters of the creatures it targets.
fn doubles_counters(ability: &Ability) -> bool {
    json(ability).contains("MultiplyCounters")
}
