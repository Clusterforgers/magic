//! Class levels (CR 716): "{3}{U}: Level 2" and the lines under it are the level up ability of a Class and the abilities it
//! has from that level on, so each line is read like any other line of text and conditioned on the level of the Class.

use std::str::FromStr;

use super::Own;
use super::levels::Plan;
use crate::ability::{Ability, Timing};
use crate::compile::clauses;
use crate::cost::Cost;
use crate::effect::Effect;
use crate::mana::ManaCostX;
use crate::refs::{Ref, ZoneKind};
use crate::value::{Cmp, Condition};

/// The abilities of one level of a Class: the level up ability for the cost on the header line, then every line under it,
/// each working from that level on; `None` when the header or any line is not read (the whole block stays with the table).
pub fn block(text: &str, x_free: bool, own: &Own) -> Option<Vec<Ability>> {
    let abilities = plan(text)?.read(x_free, own)?;
    (abilities.len() > 1).then_some(abilities)
}

/// The header of a level of a Class (its level up ability) and the lines under it, working from that level on.
pub(super) fn plan(text: &str) -> Option<Plan> {
    let mut lines = text.lines();
    let header = lines.next()?;
    let (level, cost) = clauses::class_header(header)?;
    let up = level_up(level, Cost::mana(ManaCostX::from_str(cost).ok()?));
    Some(Plan {
        condition: Condition::Level {
            of: Ref::This,
            op: Cmp::Ge,
            level,
        },
        head: vec![(header.to_owned(), up)],
        lines: lines.map(str::to_owned).collect(),
    })
}

/// "{cost}: Level N" (CR 716.2): at sorcery speed, only while the Class is at the level below, it becomes level N.
fn level_up(level: u32, cost: Cost) -> Ability {
    Ability::Activated {
        cost,
        timing: Timing::Sorcery,
        condition: Some(Condition::Level {
            of: Ref::This,
            op: Cmp::Eq,
            level: level.saturating_sub(1),
        }),
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: vec![Effect::BecomeLevel { level }],
    }
}
