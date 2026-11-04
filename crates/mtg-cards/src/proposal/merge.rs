//! Merge: the compiler's reading of a card and a proposal's, ability by ability, so a card is whole when every printed
//! clause is read by one or the other.

use std::collections::BTreeSet;

use super::pairing::{Paired, pair_card};
use super::{ClauseState, Report, judge};
use crate::ability::Ability;
use crate::compile::compile;
use crate::load::Library;
use crate::support::{Caps, PrintedCard};

/// The ability of a face that stands for a clause, when it reads or only lacks a writer.
fn covering(state: &ClauseState) -> Option<usize> {
    match state {
        ClauseState::Reads { ability } | ClauseState::Unwritten { ability, .. } => Some(*ability),
        _ => None,
    }
}

/// For each ability of a face, the clauses (indices into the card's clauses) it stands for.
fn clauses_of(paired: &Paired, face: usize, abilities: usize) -> Vec<Vec<usize>> {
    let mut of = vec![Vec::new(); abilities];
    for (at, clause) in paired.clauses.iter().enumerate() {
        if clause.face == face
            && let Some(ability) = covering(&clause.state)
            && let Some(slot) = of.get_mut(ability)
        {
            slot.push(at);
        }
    }
    of
}

/// The proposal's rules with the compiler's reading wherever the compiler reads a clause: a proposal ability stays only
/// when none of its clauses is read by the compiler, and an ability without a clause is dropped, so no clause is
/// read twice or lost. `None` when the compiler reads nothing the proposal lacks or the proposal is not valid rules.
/// The merged rules are judged like any proposal; a clause neither reads stays a `Manual` ability.
pub fn merge(
    proposal: &Report,
    printed: &PrintedCard,
    hand_written: &Library,
    names: &BTreeSet<String>,
    caps: &Caps,
) -> Option<Report> {
    let proposed = proposal.rules.as_ref()?;
    let parsed = compile(printed);
    let compiler_card = parsed.card?;
    let of_proposal = pair_card(proposed, &parsed.clauses, printed);
    let of_compiler = pair_card(&compiler_card, &parsed.clauses, printed);
    let mut merged = proposed.clone();
    let mut took = false;
    for (face, rules) in merged.faces.iter_mut().enumerate() {
        let proposal_clauses = clauses_of(&of_proposal, face, rules.abilities.len());
        let compiler_abilities = compiler_card
            .faces
            .get(face)
            .map_or(&[][..], |f| &f.abilities);
        let compiler_clauses = clauses_of(&of_compiler, face, compiler_abilities.len());
        let read: BTreeSet<usize> = compiler_clauses.iter().flatten().copied().collect();
        let mut chosen = Vec::new();
        for (ability, clauses) in rules.abilities.iter().zip(&proposal_clauses) {
            let manual = matches!(ability, Ability::Manual { .. });
            if !manual && !clauses.is_empty() && clauses.iter().all(|c| !read.contains(c)) {
                chosen.push((clauses[0], ability.clone()));
            }
        }
        for (ability, clauses) in compiler_abilities.iter().zip(&compiler_clauses) {
            if !clauses.is_empty() && !matches!(ability, Ability::Manual { .. }) {
                took = true;
                chosen.push((clauses[0], ability.clone()));
            }
        }
        chosen.sort_by_key(|(first, _)| *first);
        rules.abilities = chosen.into_iter().map(|(_, ability)| ability).collect();
    }
    if !took {
        return None;
    }
    let mut report = Report {
        card: proposal.card.clone(),
        problems: Vec::new(),
        clauses: Vec::new(),
        strays: Vec::new(),
        support: None,
        rules: None,
        repairs: proposal.repairs.clone(),
    };
    report = judge(
        report,
        merged,
        &parsed.clauses,
        printed,
        (hand_written, names, caps),
    );
    Some(report)
}
