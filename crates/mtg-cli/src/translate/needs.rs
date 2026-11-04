//! The constructs a mapper cannot read because the language or the engine lacks a node, with the node that is missing.

/// Gap names (prefixes) and the node or primitive their cards need.
pub const NEEDS: [(&str, &str); 15] = [
    (
        "key:Static.Continuous.AffectedZone",
        "Static.affected on cards outside the battlefield (stack, hand, graveyard, exile)",
    ),
    (
        "effect:ChooseType",
        "an effect that chooses a card or creature type on resolution and binds it for later steps",
    ),
    (
        "effect:ChooseColor",
        "an effect that chooses a colour on resolution and binds it for later steps",
    ),
    (
        "effect:ChooseNumber",
        "an effect that chooses a number on resolution and binds it as an Amount",
    ),
    (
        "effect:ChoosePlayer",
        "an effect that chooses a player on resolution and binds it as a Ref",
    ),
    (
        "effect:GenericChoice",
        "a named-option choice (Either with labels) whose branch later steps can read",
    ),
    ("key:Mill.Remember", "Mill binding the milled cards (`as`)"),
    (
        "key:Sacrifice.Remember",
        "Sacrifice binding the sacrificed objects (`as`)",
    ),
    ("key:Draw.Remember", "Draw binding the drawn cards (`as`)"),
    (
        "key:Destroy.Remember",
        "Destroy binding the destroyed objects (`as`)",
    ),
    (
        "key:ChangeZone.RememberLKI",
        "Move binding the moved objects and their last known information",
    ),
    (
        "key:ChangeZoneAll.RememberChanged",
        "Move of a selector binding the moved objects (`as`)",
    ),
    (
        "valid:or-with-different-properties",
        "Pred::AllOf: a conjunction as one alternative of an AnyOf",
    ),
    (
        "static:UntapOtherPlayer",
        "GameRule::UntapDuring { what, whose }",
    ),
    (
        "static:AlternativeCost",
        "an alternative cost of other spells or of activated abilities (CostModifier change Instead(cost))",
    ),
];

/// The missing node for a gap name, when the gap needs one.
pub fn need(gap: &str) -> Option<&'static str> {
    NEEDS
        .iter()
        .find(|(prefix, _)| gap.starts_with(prefix))
        .map(|(_, need)| *need)
}
