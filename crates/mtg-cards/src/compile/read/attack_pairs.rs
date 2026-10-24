//! A static ability of a group and the rule that the group attacks, in one sentence: "All creatures have double strike
//! and attack each combat if able."

use super::render::variants;
use crate::ability::Ability;
use crate::modify::GameRule;
use crate::refs::ZoneKind;

/// The words that end the sentence the rule is worded alone.
const ATTACK: &str = " attack each combat if able.";

/// Every wording of a static ability of a group and the rule that the same group attacks each combat; empty for any
/// other two abilities.
pub(super) fn texts(first: &Ability, second: &Ability) -> Vec<String> {
    let (
        Ability::Static {
            affected,
            condition: None,
            defining: false,
            from: ZoneKind::Battlefield,
            ..
        },
        Ability::Rule {
            rule: GameRule::MustAttack(attackers),
            condition: None,
            from: ZoneKind::Battlefield,
        },
    ) = (first, second)
    else {
        return Vec::new();
    };
    if affected != attackers {
        return Vec::new();
    }
    let rules = variants(second);
    variants(first)
        .iter()
        .flat_map(|front| {
            rules.iter().filter_map(move |rule| {
                let subject = rule.strip_suffix(ATTACK)?;
                let has = front.strip_prefix(subject)?.strip_suffix('.')?;
                has.starts_with(" have ")
                    .then(|| format!("{subject}{has} and attack each combat if able."))
            })
        })
        .collect()
}
