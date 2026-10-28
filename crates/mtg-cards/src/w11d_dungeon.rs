//! Dungeons (CR 309): the definition files under `dungeons/`, embedded at build time, with each room's ability read
//! from its printed text by the card compiler or written out in the file.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::ability::Ability;
use crate::refs::TargetSpec;
use crate::compile::ClauseKind;
use crate::effect::Effect;

/// The embedded dungeon files, by path.
const W11D_FILES: &[(&str, &str)] = &[
    (
        "dungeons/lost_mine_of_phandelver.json",
        include_str!("../dungeons/lost_mine_of_phandelver.json"),
    ),
    (
        "dungeons/dungeon_of_the_mad_mage.json",
        include_str!("../dungeons/dungeon_of_the_mad_mage.json"),
    ),
    (
        "dungeons/tomb_of_annihilation.json",
        include_str!("../dungeons/tomb_of_annihilation.json"),
    ),
    ("dungeons/undercity.json", include_str!("../dungeons/undercity.json")),
];

/// A dungeon file: its name and its rooms, the first room first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct W11dDungeonFile {
    /// The printed name.
    pub name: String,
    /// Whether only "venture into Undercity" enters it (CR 309.4d).
    #[serde(default)]
    pub undercity: bool,
    /// The rooms, the first room first.
    pub rooms: Vec<W11dRoomFile>,
}

/// A room as printed: its name, its text and the rooms its arrows lead to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct W11dRoomFile {
    /// The room's name.
    pub name: String,
    /// The room ability's printed text.
    pub text: String,
    /// The names of the rooms one arrow down; none for the last room.
    #[serde(default)]
    pub leads_to: Vec<String>,
    /// The ability written out (an `Ability::Spell`), for a text the compiler does not read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ability: Option<Ability>,
}

/// A dungeon ready to play.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct W11dDungeon {
    /// The printed name.
    pub name: String,
    /// Whether only "venture into Undercity" enters it.
    pub undercity: bool,
    /// The rooms, the first room first.
    pub rooms: Vec<W11dRoom>,
}

/// A room ready to play.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct W11dRoom {
    /// The room's name.
    pub name: String,
    /// The room ability's printed text.
    pub text: String,
    /// The indices of the rooms one arrow down.
    pub next: Vec<u8>,
    /// The room ability's targets.
    pub targets: Vec<TargetSpec>,
    /// The room ability's effects.
    pub effects: Vec<Effect>,
    /// Whether the ability was read or written out; an unread room has no targets and no effects.
    pub read: bool,
}

impl W11dDungeon {
    /// Whether every room's ability was read, so the dungeon can be entered.
    pub fn w11d_ready(&self) -> bool {
        self.rooms.iter().all(|room| room.read)
    }
}

static W11D_DUNGEONS: OnceLock<Result<Vec<W11dDungeon>, Vec<String>>> = OnceLock::new();

/// The embedded dungeons in file order, or every problem found in them. The tests of this crate load them, so an
/// error here means the crate was built from files they would have refused.
pub fn w11d_dungeons() -> Result<&'static [W11dDungeon], &'static [String]> {
    W11D_DUNGEONS
        .get_or_init(|| w11d_load(W11D_FILES))
        .as_ref()
        .map(Vec::as_slice)
        .map_err(Vec::as_slice)
}

/// The embedded dungeon of this name.
pub fn w11d_dungeon(name: &str) -> Option<&'static W11dDungeon> {
    w11d_dungeons().ok()?.iter().find(|dungeon| dungeon.name == name)
}

/// Reads and checks dungeon files given as `(path, text)`.
pub fn w11d_load(files: &[(&str, &str)]) -> Result<Vec<W11dDungeon>, Vec<String>> {
    let mut problems = Vec::new();
    let mut dungeons = Vec::new();
    for (path, text) in files {
        match serde_json::from_str::<W11dDungeonFile>(text) {
            Ok(file) => match w11d_check(&file) {
                Ok(dungeon) => dungeons.push(dungeon),
                Err(found) => problems.extend(found.into_iter().map(|p| format!("{path}: {p}"))),
            },
            Err(error) => problems.push(format!("{path}: {error}")),
        }
    }
    if problems.is_empty() {
        Ok(dungeons)
    } else {
        Err(problems)
    }
}

/// Checks a dungeon file: at least one room, distinct room names, arrows that lead to a later room, every room
/// reached from the first, a last room with no arrow, and a room ability that reads and validates.
pub fn w11d_check(file: &W11dDungeonFile) -> Result<W11dDungeon, Vec<String>> {
    let mut problems = Vec::new();
    if file.rooms.is_empty() {
        problems.push("a dungeon has at least one room".to_owned());
    }
    let names: Vec<&str> = file.rooms.iter().map(|room| room.name.as_str()).collect();
    if names.iter().collect::<BTreeSet<_>>().len() != names.len() {
        problems.push("two rooms have the same name".to_owned());
    }
    let mut reached = vec![false; file.rooms.len()];
    if let Some(first) = reached.first_mut() {
        *first = true;
    }
    let mut rooms = Vec::new();
    for (index, room) in file.rooms.iter().enumerate() {
        let next = w11d_arrows(room, index, &names, &mut problems);
        if reached.get(index) == Some(&true) {
            for to in &next {
                if let Some(seen) = reached.get_mut(usize::from(*to)) {
                    *seen = true;
                }
            }
        }
        match w11d_ability(room) {
            Ok(read) => {
                let (targets, effects) = read.clone().unwrap_or_default();
                rooms.push(W11dRoom {
                    name: room.name.clone(),
                    text: room.text.clone(),
                    next,
                    targets,
                    effects,
                    read: read.is_some(),
                });
            }
            Err(problem) => problems.push(format!("{}: {problem}", room.name)),
        }
    }
    if let Some(unreached) = reached.iter().position(|seen| !seen) {
        problems.push(format!("no arrow leads to {}", names[unreached]));
    }
    if file.rooms.last().is_some_and(|room| !room.leads_to.is_empty()) {
        problems.push("the last room has no arrow".to_owned());
    }
    if problems.is_empty() {
        Ok(W11dDungeon {
            name: file.name.clone(),
            undercity: file.undercity,
            rooms,
        })
    } else {
        Err(problems)
    }
}

fn w11d_arrows(room: &W11dRoomFile, index: usize, names: &[&str], problems: &mut Vec<String>) -> Vec<u8> {
    let mut next = Vec::new();
    for to in &room.leads_to {
        match names.iter().position(|name| name == to) {
            Some(at) if at > index => next.push(u8::try_from(at).unwrap_or(u8::MAX)),
            Some(_) => problems.push(format!("{} leads back up to {to}", room.name)),
            None => problems.push(format!("{} leads to {to}, which is not a room", room.name)),
        }
    }
    next
}

type W11dRead = Option<(Vec<TargetSpec>, Vec<Effect>)>;

/// The room ability: the one written out, or the compiler's reading of the text as a spell's text; `None` when the
/// compiler does not read it.
fn w11d_ability(room: &W11dRoomFile) -> Result<W11dRead, String> {
    let read = match &room.ability {
        Some(written) => Some(written.clone()),
        None => crate::compile::w11d_room_clause(&room.text, ClauseKind::Spell),
    };
    let Some(ability) = read else {
        return Ok(None);
    };
    let problems = crate::validate::w11d_check_room(&ability, &crate::builtin::w11d_token_ids());
    if let Some(problem) = problems.first() {
        return Err(format!("{}: {}", problem.path, problem.message));
    }
    match ability {
        Ability::Spell { targets, effects } => Ok(Some((targets, effects))),
        _ => Err("a room ability is written as a spell's ability".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the embedded dungeons load with the rooms and arrows as printed, and the check refuses a dungeon with no room, a doubled room name, an arrow up or to nowhere, an unreached room, a last room with an arrow or a room ability that does not validate; a room the compiler does not read keeps its dungeon from being ready.
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use rstest::rstest;

    use super::*;

    #[test]
    fn the_embedded_dungeons_load_with_their_rooms() {
        let dungeons = w11d_dungeons().unwrap();
        let shape: Vec<(&str, bool, usize, bool)> = dungeons
            .iter()
            .map(|d| (d.name.as_str(), d.undercity, d.rooms.len(), d.w11d_ready()))
            .collect();
        assert_eq!(
            shape,
            [
                ("Lost Mine of Phandelver", false, 7, true),
                ("Dungeon of the Mad Mage", false, 9, false),
                ("Tomb of Annihilation", false, 5, false),
                ("Undercity", true, 9, false),
            ]
        );
        let mine = w11d_dungeon("Lost Mine of Phandelver").unwrap();
        let arrows: Vec<&[u8]> = mine.rooms.iter().map(|room| room.next.as_slice()).collect();
        assert_eq!(arrows, [&[1, 2][..], &[3, 4], &[4, 5], &[6], &[6], &[6], &[]]);
    }

    fn room(name: &str, text: &str, leads_to: &[&str]) -> W11dRoomFile {
        W11dRoomFile {
            name: name.to_owned(),
            text: text.to_owned(),
            leads_to: leads_to.iter().map(|to| (*to).to_owned()).collect(),
            ability: None,
        }
    }

    #[rstest]
    #[case::no_room(vec![], "at least one room")]
    #[case::doubled_name(vec![room("A", "Draw a card.", &["A"]), room("A", "Draw a card.", &[])], "same name")]
    #[case::arrow_up(vec![room("A", "Draw a card.", &["B"]), room("B", "Draw a card.", &["A"])], "leads back up")]
    #[case::arrow_nowhere(vec![room("A", "Draw a card.", &["C"])], "not a room")]
    #[case::unreached(vec![room("A", "Draw a card.", &[]), room("B", "Draw a card.", &[])], "no arrow leads to B")]
    #[case::last_with_arrow(vec![room("A", "Draw a card.", &["B"]), room("B", "Draw a card.", &["B"])], "last room")]
    fn the_check_refuses(#[case] rooms: Vec<W11dRoomFile>, #[case] problem: &str) {
        let file = W11dDungeonFile {
            name: "Test".to_owned(),
            undercity: false,
            rooms,
        };
        let found = w11d_check(&file).unwrap_err();
        assert!(found.iter().any(|p| p.contains(problem)), "{found:?}");
    }

    #[test]
    fn a_written_ability_stands_for_a_text_the_compiler_does_not_read() {
        let mut only = room("A", "Frobnicate wildly.", &[]);
        only.ability = Some(serde_json::from_str(r#"{"Spell":{"effects":[{"Draw":{}}]}}"#).unwrap());
        let file = W11dDungeonFile {
            name: "Test".to_owned(),
            undercity: false,
            rooms: vec![only],
        };
        let dungeon = w11d_check(&file).unwrap();
        assert_eq!(dungeon.rooms[0].effects.len(), 1);
        assert!(dungeon.w11d_ready());
        let unread = W11dDungeonFile {
            rooms: vec![room("A", "Frobnicate wildly.", &[])],
            ..file
        };
        assert!(!w11d_check(&unread).unwrap().w11d_ready());
        assert!(w11d_load(&[("x.json", "{}")]).is_err());
    }
}
