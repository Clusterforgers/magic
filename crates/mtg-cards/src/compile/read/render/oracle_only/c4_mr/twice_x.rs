//! A size change of twice a number: "Each creature gets twice -X/-X until end of turn." The data is a product of -2 and X.

use super::twin_variants;
use crate::ability::Ability;

/// The size change of a product of -2 and X, and the plain size change the renderers write instead.
const TWICE: &str =
    r#""power":{"Times":{"by":-2,"of":"X"}},"toughness":{"Times":{"by":-2,"of":"X"}}"#;
const PLAIN: &str =
    r#""power":{"Times":{"by":-1,"of":"X"}},"toughness":{"Times":{"by":-1,"of":"X"}}"#;

/// Adds the wordings of an ability that changes sizes by twice -X/-X.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let said: Vec<String> = twin_variants(ability, TWICE, PLAIN)
        .iter()
        .filter(|text| text.contains(" -X/-X"))
        .map(|text| text.replace(" -X/-X", " twice -X/-X"))
        .collect();
    texts.extend(said);
}
