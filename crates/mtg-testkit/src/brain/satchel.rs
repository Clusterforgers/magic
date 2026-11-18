//! When a boss's bot casts its Satchel (`mtg_engine::satchel`): each card waits for its cue ([`SatchelCue`]), read from
//! the campaign content ([`install`]) or, for a card the content gives none, from [`builtin`]; the bot casts a card as
//! its cue holds and holds it otherwise. A reusable card's tax is weighed against what it costs and against the hand.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use mtg_core::cue::SatchelCue;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::Target;
use mtg_engine::decision::{PriorityAction, PriorityOptions};
use mtg_engine::spotlight;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;

use super::seen::{Body, Seen};
use super::spotlight::{boss_seat, sweeps};
use super::threat::open_mana;

/// The cues the campaign content gives, by card name.
static INSTALLED: OnceLock<BTreeMap<String, SatchelCue>> = OnceLock::new();

/// Sets the content's cues by card name for every game of the process; false when cues were set before.
pub fn install(cues: BTreeMap<String, SatchelCue>) -> bool {
    INSTALLED.set(cues).is_ok()
}

/// The cue of the Satchel card named `name`: the content's, else the built-in one.
pub fn cue_of(name: &str) -> Option<SatchelCue> {
    INSTALLED
        .get()
        .and_then(|cues| cues.get(name).copied())
        .or_else(|| builtin(name))
}

/// The cues of the Drowned Spire's Satchel cards, which its content does not give yet.
pub fn builtin(name: &str) -> Option<SatchelCue> {
    use SatchelCue as C;
    Some(match name {
        "Entrancing Melody" => C::Eyed { creature: true },
        "Extract from Darkness" | "Reanimate" => C::Graveyard { creatures: 1 },
        "Sudden Spoiling" | "Aetherize" => C::Combat,
        "Memory Plunder" | "Grapple with the Past" => C::HeroesEnd { mana: 0 },
        "Spelltwine" => C::Mana { mana: 6 },
        "Rise of the Dark Realms" => C::Mana { mana: 9 },
        "Octopus Umbra" => C::Board { creatures: 1 },
        "Sleep" => C::Lethal { reach: 0 },
        "Crush of Tentacles" => C::BehindOnBoard { power: 1 },
        _ => return None,
    })
}

/// Whether `cue` holds for the seat of `seen` now.
pub fn fires(seen: &Seen<'_>, cue: SatchelCue) -> bool {
    let seat = seen.seat();
    let state = seen.state();
    let step = seen.turn().step();
    let bodies = seen.bodies();
    let heroes = |body: &&Body| spotlight::is_hero(state, body.controller);
    let ours = |body: &&Body| seen.same_team(body.controller, seat);
    let power = |pick: &dyn Fn(&&Body) -> bool| -> u32 {
        let total: i32 = bodies
            .iter()
            .filter(|b| b.is_creature())
            .filter(pick)
            .map(Body::power)
            .sum();
        u32::try_from(total).unwrap_or(0)
    };
    match cue {
        SatchelCue::EarlyTurns { turns } => state.turns_taken(seat) <= turns,
        SatchelCue::BehindOnBoard { power: by } => {
            power(&heroes) >= power(&ours).saturating_add(by)
        }
        SatchelCue::Kill { toughness } => bodies
            .iter()
            .filter(heroes)
            .any(|b| b.is_creature() && u32::try_from(b.toughness()).is_ok_and(|t| t <= toughness)),
        SatchelCue::Lethal { reach } => lethal(seen, &bodies, reach),
        SatchelCue::Protect => protects(seen),
        SatchelCue::Counter { mana_value } => seen.stack().last().is_some_and(|top| {
            hero_owned(seen, *top)
                && seen.mana_value(*top) >= mana_value
                && state.object(*top).is_some_and(|o| o.ability().is_none())
        }),
        SatchelCue::Combat => step == Step::DeclareBlockers,
        SatchelCue::HandLow { cards } => {
            u32::try_from(seen.hand_size(seat)).is_ok_and(|held| held <= cards)
        }
        SatchelCue::HeroesEnd { mana } => {
            step == Step::End
                && seen.stack().is_empty()
                && spotlight::standing(state)
                    .iter()
                    .any(|hero| seen.is_active(*hero))
                && open_mana(seen, &bodies) >= mana
        }
        SatchelCue::Mana { mana } => open_mana(seen, &bodies) >= mana,
        SatchelCue::Eyed { creature } => spotlight::focus(state, seat).is_some_and(|eyed| {
            bodies.iter().any(|b| {
                b.controller == eyed
                    && b.mana_value > 0
                    && !b.is_land()
                    && (!creature || b.is_creature())
            })
        }),
        SatchelCue::Graveyard { creatures } => {
            let count = state
                .zone(Zone::Graveyard(seat))
                .iter()
                .filter(|id| seen.chars(**id).is_some_and(|c| c.is_creature()))
                .count();
            u32::try_from(count).is_ok_and(|count| count >= creatures)
        }
        SatchelCue::Board { creatures } => {
            let count = bodies
                .iter()
                .filter(|b| b.controller == seat && b.is_creature())
                .count();
            u32::try_from(count).is_ok_and(|count| count >= creatures)
        }
    }
}

/// Whether the seat's creatures that can attack, each with `reach` more power, reach a standing hero's life, in its
/// own turn before combat.
fn lethal(seen: &Seen<'_>, bodies: &[Body], reach: u32) -> bool {
    let seat = seen.seat();
    if !seen.is_active(seat) || !matches!(seen.turn().step(), Step::Main1 | Step::BeginCombat) {
        return false;
    }
    let reach = i32::try_from(reach).unwrap_or(i32::MAX);
    let total: i32 = bodies
        .iter()
        .filter(|b| b.controller == seat && b.is_creature() && !b.tapped && !b.cant_attack)
        .map(|b| b.power().saturating_add(reach))
        .fold(0, i32::saturating_add);
    total > 0
        && spotlight::standing(seen.state())
            .iter()
            .any(|hero| seen.life(*hero) <= total)
}

/// Whether the object `id` on the stack is a hero's.
fn hero_owned(seen: &Seen<'_>, id: ObjectId) -> bool {
    seen.controller(id)
        .is_some_and(|p| spotlight::is_hero(seen.state(), p))
}

/// Whether a hero's spell or ability on the stack targets a permanent of the seat's side, or a hero's spell sweeps.
fn protects(seen: &Seen<'_>) -> bool {
    let seat = seen.seat();
    seen.stack().iter().any(|id| {
        if !hero_owned(seen, *id) {
            return false;
        }
        let Some(object) = seen.state().object(*id) else {
            return false;
        };
        let aimed = object.choices().targets.iter().flatten().any(|target| {
            matches!(target, Target::Object(at)
                if seen.zone(at.id) == Some(Zone::Battlefield)
                    && seen.controller(at.id).is_some_and(|p| seen.same_team(p, seat)))
        });
        aimed || (object.ability().is_none() && seen.card(*id).is_some_and(|c| sweeps(c, None)))
    })
}

/// Whether a reusable card with `tax` on top of its mana value `mana_value` is a bad deal now: the tax is more than
/// twice its mana value (two more for an urgent cue), or the cue is not urgent and the hand offers a castable spell of
/// at least its mana value (`hand_best`, the largest).
pub fn taxed_out(tax: u32, mana_value: u32, cue: SatchelCue, hand_best: Option<u32>) -> bool {
    if tax == 0 {
        return false;
    }
    let limit = mana_value.max(1).saturating_mul(2) + if cue.urgent() { 2 } else { 0 };
    tax > limit || (!cue.urgent() && hand_best.is_some_and(|best| best >= mana_value))
}

/// The cue of `id` when it is a card in the seat's Satchel and the seat plays for a boss's side.
fn satchel_cue(seen: &Seen<'_>, id: ObjectId) -> Option<SatchelCue> {
    let state = seen.state();
    if !state.in_satchel(id) || seen.controller(id) != Some(seen.seat()) || !boss_seat(seen) {
        return None;
    }
    cue_of(&seen.card(id)?.name)
}

/// Whether the seat may cast the Satchel card `id` with its cue `cue` now: the cue holds and the tax is worth it.
fn ready(seen: &Seen<'_>, options: &PriorityOptions, id: ObjectId, cue: SatchelCue) -> bool {
    if !fires(seen, cue) {
        return false;
    }
    let hand_best = options
        .castable
        .iter()
        .filter(|other| seen.zone(**other) == Some(Zone::Hand(seen.seat())))
        .map(|other| seen.mana_value(*other))
        .max();
    !taxed_out(
        seen.state().satchel_tax(id),
        seen.mana_value(id),
        cue,
        hand_best,
    )
}

/// Whether the seat holds `play` back: casting a card of its Satchel whose cue does not hold or whose tax is a bad deal.
pub fn holds(seen: &Seen<'_>, options: &PriorityOptions, play: &PriorityAction) -> bool {
    let PriorityAction::CastSpell(id) = play else {
        return false;
    };
    satchel_cue(seen, *id).is_some_and(|cue| !ready(seen, options, *id, cue))
}

/// The Satchel card the seat casts now: the first castable one whose cue holds and whose tax is worth paying, other
/// than the plays in `held`.
pub fn plan(
    seen: &Seen<'_>,
    options: &PriorityOptions,
    held: &[PriorityAction],
) -> Option<PriorityAction> {
    options.castable.iter().find_map(|id| {
        let play = PriorityAction::CastSpell(*id);
        let cue = satchel_cue(seen, *id)?;
        (!held.contains(&play) && ready(seen, options, *id, cue)).then_some(play)
    })
}
