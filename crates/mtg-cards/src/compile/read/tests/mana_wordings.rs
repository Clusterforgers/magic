//! Goal: scratch
use super::*;

#[test]
fn print() {
    for (text, kind) in [
        (
            "Enchanted land has \"{T}: Add one mana of any color.\"",
            ClauseKind::Static,
        ),
        (
            "Enchanted land has \"{T}: Add two mana of any one color.\"",
            ClauseKind::Static,
        ),
        (
            "{T}: Add one mana of any color. Spend this mana only to cast a creature spell of the chosen type.",
            ClauseKind::Activated,
        ),
        (
            "{T}: Add one mana of any color. Spend this mana only to cast a creature spell of the chosen type or activate an ability of a creature source of the chosen type.",
            ClauseKind::Activated,
        ),
    ] {
        let a = clause(text, kind, false);
        println!("{text}\n  {}", serde_json::to_string(&a).unwrap());
    }
}
