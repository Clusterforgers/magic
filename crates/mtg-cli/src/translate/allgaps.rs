//! The gaps a script line holds besides the first: readers stop at the first construct they cannot map, so the rest of
//! the line is found by dropping the parameter that stopped it and translating again.

use std::collections::BTreeSet;

use super::card::translate_face;
use super::ctx::World;
use crate::forge::script::{Face, Script};

/// The most rounds spent on one line: each round drops one parameter and reads what is behind it.
const ROUNDS: usize = 6;

/// The kinds of line whose parameters are dropped one at a time; keyword lines (`K:`) have no named parameters.
const KINDS: [char; 4] = ['A', 'T', 'S', 'R'];

/// Every gap of the script found behind the first of its line: the translation itself is not changed, this only reads
/// further. Keyword lines and the gaps of lines that no single parameter explains are not reached.
pub fn others(world: &World, script: &Script) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for face in &script.faces {
        let base = names(world, face);
        if base.is_empty() {
            continue;
        }
        for (kind, at) in lines(face) {
            let without = names(world, &drop_line(face, kind, at));
            if base.difference(&without).next().is_none() {
                continue;
            }
            found.extend(peel(world, face, kind, at, &base));
        }
    }
    found
}

/// The gap names of a face.
fn names(world: &World, face: &Face) -> BTreeSet<String> {
    translate_face(world, face)
        .gaps
        .into_iter()
        .map(|gap| gap.0)
        .collect()
}

/// The kind and index of every line that states a rule.
fn lines(face: &Face) -> Vec<(char, usize)> {
    KINDS
        .iter()
        .flat_map(|&kind| (0..bodies(face, kind).len()).map(move |at| (kind, at)))
        .collect()
}

fn bodies(face: &Face, kind: char) -> &Vec<String> {
    match kind {
        'A' => &face.abilities,
        'T' => &face.triggers,
        'S' => &face.statics,
        _ => &face.replacements,
    }
}

fn bodies_mut(face: &mut Face, kind: char) -> &mut Vec<String> {
    match kind {
        'A' => &mut face.abilities,
        'T' => &mut face.triggers,
        'S' => &mut face.statics,
        _ => &mut face.replacements,
    }
}

fn drop_line(face: &Face, kind: char, at: usize) -> Face {
    let mut out = face.clone();
    bodies_mut(&mut out, kind).remove(at);
    out
}

/// A place a parameter can be dropped: a line, or a named part that a line runs.
enum Spot {
    Line(char, usize),
    Svar(String),
}

fn text<'a>(face: &'a Face, spot: &Spot) -> Option<&'a String> {
    match spot {
        Spot::Line(kind, at) => bodies(face, *kind).get(*at),
        Spot::Svar(name) => face.svars.get(name),
    }
}

/// The text without its `n`th `|`-part.
fn drop_part(text: &str, n: usize) -> String {
    let kept: Vec<&str> = text
        .split('|')
        .enumerate()
        .filter(|(at, _)| *at != n)
        .map(|(_, part)| part)
        .collect();
    kept.join("|")
}

/// The line itself and the named parts that run from it, which are written as lines of parameters.
fn spots(face: &Face, kind: char, at: usize) -> Vec<Spot> {
    let mut spots = vec![Spot::Line(kind, at)];
    let mut queue = vec![bodies(face, kind)[at].clone()];
    let mut seen = BTreeSet::new();
    while let Some(body) = queue.pop() {
        for part in body.split('|') {
            let Some((_, value)) = part.split_once('$') else {
                continue;
            };
            let name = value.trim();
            let Some(svar) = face.svars.get(name) else {
                continue;
            };
            let head = svar.trim_start();
            let runs = ["DB$", "AB$", "SP$"]
                .iter()
                .any(|tag| head.starts_with(tag));
            if runs && seen.insert(name.to_owned()) {
                spots.push(Spot::Svar(name.to_owned()));
                queue.push(svar.clone());
            }
        }
    }
    spots
}

/// Whether a gap names a part the line lacks (`no-Execute`, `-without-cost`, `-missing`): dropping a parameter the
/// line needs causes it, so it says nothing about the line.
fn lacks_part(name: &str) -> bool {
    name.starts_with("line:")
        || name.starts_with("svar:")
        || name.ends_with(['=', ':'])
        || ["no-", "without", "-missing", "nothing", "none-for"]
            .iter()
            .any(|mark| name.contains(mark))
}

/// The gaps behind the first of one line: a parameter whose removal clears a gap is dropped for good and the line is
/// read again, until nothing clears. A removal that clears a gap and shows none is preferred.
fn peel(
    world: &World,
    face: &Face,
    kind: char,
    at: usize,
    base: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut current = face.clone();
    let mut known = base.clone();
    let mut found = BTreeSet::new();
    for _ in 0..ROUNDS {
        let mut step = None;
        'edits: for spot in spots(&current, kind, at) {
            let Some(body) = text(&current, &spot) else {
                continue;
            };
            for n in 1..body.split('|').count() {
                let mut next = current.clone();
                let edited = drop_part(body, n);
                match &spot {
                    Spot::Line(kind, at) => bodies_mut(&mut next, *kind)[*at] = edited,
                    Spot::Svar(name) => {
                        next.svars.insert(name.clone(), edited);
                    }
                }
                let after = names(world, &next);
                if known.difference(&after).next().is_none() {
                    continue;
                }
                let fresh: BTreeSet<String> = after.difference(&known).cloned().collect();
                if fresh.iter().any(|name| lacks_part(name)) {
                    continue;
                }
                let pure = fresh.is_empty();
                if step.is_none() || pure {
                    step = Some((next, after, fresh));
                }
                if pure {
                    break 'edits;
                }
            }
        }
        let Some((next, after, fresh)) = step else {
            break;
        };
        found.extend(fresh);
        known = after;
        current = next;
    }
    found
}
