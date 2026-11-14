//! Target ranking: the likeliest targets of a spell or ability first, so a search tries them first and a quick answer
//! takes the first. Teammates count as the seat's own side.

use mtg_engine::choose::{Target, TargetChoice, TargetOptions};
use mtg_engine::decision::Answer;
use mtg_engine::zones::Zone;

use super::eval::{creature_value, permanent_value};
use super::seen::Seen;
use crate::bot::rmt1_within_total;

/// What `target` is worth to whoever holds it, as the seat reads it, and whether that is an opponent.
fn worth(seen: &Seen<'_>, target: Target) -> (f64, bool) {
    match target {
        Target::Player(player) => {
            // The lower an opponent's life, the more a hit at them is worth; a boss's side reads the spotlight instead.
            let worth = if super::spotlight::boss_seat(seen) {
                20.0 + super::spotlight::lean(seen, player)
            } else {
                30.0 - f64::from(seen.life(player)).min(30.0)
            };
            (worth, !seen.is_teammate(player))
        }
        Target::Object(object) => {
            let id = object.id;
            let Some(controller) = seen.controller(id) else {
                return (0.0, false);
            };
            let worth = match seen.zone(id) {
                Some(Zone::Battlefield) => seen.body(id).map_or(0.0, |body| {
                    if body.is_creature() {
                        creature_value(&body)
                    } else {
                        permanent_value(&body)
                    }
                }),
                _ => 1.0 + f64::from(seen.mana_value(id)),
            };
            let worth = worth * super::spotlight::object_factor(seen, controller);
            (worth, !seen.is_teammate(controller))
        }
    }
}

/// The targets `options` offers, likeliest first: the most valuable opposing permanent or spell, then the seat's own
/// most valuable one, then the opponent most worth hitting, and so on alternating, then the seat's side's players;
/// removal, burn and pacifying auras keep only opposing targets while there are any.
pub fn rank_targets(seen: &Seen<'_>, options: &TargetOptions) -> Vec<Target> {
    let mut theirs: Vec<(f64, Target)> = Vec::new();
    let mut ours: Vec<(f64, Target)> = Vec::new();
    let mut their_players: Vec<(f64, Target)> = Vec::new();
    let mut our_players: Vec<(f64, Target)> = Vec::new();
    for target in options.c10_dice_choosable() {
        let (worth, opposing) = worth(seen, *target);
        let list = match (target.player().is_some(), opposing) {
            (false, true) => &mut theirs,
            (false, false) => &mut ours,
            (true, true) => &mut their_players,
            (true, false) => &mut our_players,
        };
        list.push((worth, *target));
    }
    for list in [&mut theirs, &mut ours, &mut their_players] {
        list.sort_by(|a, b| b.0.total_cmp(&a.0));
    }
    // The seat itself before its teammates.
    our_players.sort_by_key(|(_, target)| target.player() != Some(seen.seat()));
    let mut ranked = Vec::new();
    let longest = theirs.len().max(ours.len()).max(their_players.len());
    for at in 0..longest {
        for list in [&theirs, &ours, &their_players] {
            if let Some((_, target)) = list.get(at) {
                ranked.push(*target);
            }
        }
    }
    ranked.extend(our_players.into_iter().map(|(_, target)| target));
    // Removal, burn and pacifying auras never aim at the seat's side while an opposing target is offered.
    let harmful = seen.card(options.source).is_some_and(|card| {
        let traits = super::threat::traits(card);
        traits.removal || traits.burn || {
            let text = format!("{:?}", card.rules);
            text.contains("CantAttack") || text.contains("CantBlock")
        }
    });
    if harmful && ranked.iter().any(|target| worth(seen, *target).1) {
        ranked.retain(|target| worth(seen, *target).1);
    }
    // A boss's side spares the heroes the spotlight spares while it has another opposing target.
    if super::spotlight::boss_seat(seen) {
        let fair = |target: &Target| {
            let holder = match target {
                Target::Player(player) => Some(*player),
                Target::Object(object) => seen.controller(object.id),
            };
            holder.is_none_or(|p| seen.is_teammate(p) || super::spotlight::may_aim(seen, p))
        };
        let spared = |target: &Target| !fair(target);
        if ranked.iter().any(spared)
            && ranked
                .iter()
                .any(|target| fair(target) && worth(seen, *target).1)
        {
            ranked.retain(fair);
        }
    }
    ranked
}

/// The answer naming `picked`, topped up to the least count from the legal targets and cut to the total the options
/// allow.
pub fn target_answer(options: &TargetOptions, mut picked: Vec<Target>) -> Answer {
    for target in options.c10_dice_choosable() {
        if picked.len() >= options.min as usize {
            break;
        }
        if !picked.contains(target) {
            picked.push(*target);
        }
    }
    picked.truncate(options.max.max(options.min) as usize);
    Answer::ChooseTargets(TargetChoice::Targets(rmt1_within_total(options, picked)))
}
