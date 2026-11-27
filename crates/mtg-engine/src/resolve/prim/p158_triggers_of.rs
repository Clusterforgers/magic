//! `TokenRef::P158WithTriggersOf`: a described token that also has every triggered ability of the cards references
//! name, as part of its copiable values (The Book of Vile Darkness: Vecna).

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::assemble::token_definition;
use mtg_cards::card::TokenSpec;
use mtg_cards::modify::Modification;
use mtg_cards::refs::Ref;

use crate::caps::caps;
use crate::copies::copiable_values;
use crate::resolve::{Cx, ResolveError};

/// The token `token` describes, with the triggered abilities of each object the references of `of` name added in
/// order, read from its copiable values as it is now (cards in exile: as printed); an object named twice gives its
/// abilities once.
pub(super) fn p158_with_triggers_of(
    cx: &Cx<'_>,
    token: &TokenSpec,
    of: &[Ref],
) -> Result<CardDefinition, ResolveError> {
    let mut definition = token_definition(token, &caps());
    let mut seen = Vec::new();
    let mut named_all = Vec::new();
    for reference in of {
        named_all.extend(cx.scope.objects(reference)?);
    }
    for named in named_all {
        if seen.contains(&named) {
            continue;
        }
        seen.push(named);
        let Some(values) = copiable_values(cx.state(), named, cx.scope.snapshot_of(named)) else {
            continue;
        };
        let triggered = crate::ability_reader::printed_rules(&values)
            .filter(|rules| matches!(&***rules, Ability::Triggered { .. }));
        for rules in triggered {
            // Part of the token's own definition, so of its copiable values.
            definition.except(&Modification::GrantAbility(Arc::clone(rules)));
        }
    }
    Ok(definition)
}
