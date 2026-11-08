//! The rest of Forge's filter grammar: atoms the older readers leave (the cast of a spell, shared colors, stats against
//! the source, counted bounds, keyword parameters, extremes of a kind) and the table of atoms that stay gaps, each with
//! the fact of the game it needs.

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{CARD_TYPES, Ctx, Tr, unmapped};

/// The creature types that make a party (CR 700.8).
const PARTY_TYPES: [&str; 4] = ["Cleric", "Rogue", "Warrior", "Wizard"];

/// The predicates of an atom that is one of these, `None` for any other atom.
pub(super) fn atom(ctx: &Ctx, prop: &str) -> Tr<Option<Vec<Value>>> {
    let one = |value: Value| Ok(Some(vec![value]));
    match prop {
        "Kicked" => return one(json!("Kicked")),
        "Mayhem" => return one(json!({"CastWith": "Mayhem"})),
        "Party" => return party(ctx).map(Some),
        _ => {}
    }
    // `CastSa Spell.ManaFromTreasure`: the properties of the spell that cast the card are the card's own cast facts.
    if let Some(rest) = prop.strip_prefix("CastSa Spell.") {
        return cast_by(ctx, rest).map(Some);
    }
    // `EquippedByValid Equipment.Legendary` is the older `EquippedBy Equipment.Legendary`.
    for (head, plain) in [
        ("EquippedByValid ", "EquippedBy "),
        ("EnchantedByValid ", "EnchantedBy "),
    ] {
        if let Some(arg) = prop.strip_prefix(head) {
            return super::valid_atoms::argument(ctx, &format!("{plain}{arg}"));
        }
    }
    if let Some(word) = prop.strip_prefix("SharesColorWith ") {
        return shares_color(ctx, word).map(Some);
    }
    if let Some(found) = against_source(prop) {
        return one(found);
    }
    if let Some(found) = counted_bound(ctx, prop)? {
        return one(found);
    }
    Ok(None)
}

/// Party creatures: one of the four types, when the world knows them all.
fn party(ctx: &Ctx) -> Tr<Vec<Value>> {
    if !PARTY_TYPES
        .iter()
        .all(|name| ctx.world.subtypes.contains(*name))
    {
        return unmapped("valid:Party");
    }
    let any: Vec<Value> = PARTY_TYPES
        .iter()
        .map(|name| json!({"Subtype": name}))
        .collect();
    Ok(vec![json!({"AnyOf": any})])
}

/// The cast facts after `CastSa Spell.`: each part read as a property of the card, a negated part refused.
fn cast_by(ctx: &Ctx, rest: &str) -> Tr<Vec<Value>> {
    let mut preds = Vec::new();
    for part in super::valid::split_properties(rest) {
        if part.starts_with('!') {
            return unmapped(format!("valid:CastSa Spell.{part}"));
        }
        preds.extend(super::valid::property(ctx, &part)?);
    }
    if preds.is_empty() {
        return unmapped("valid:CastSa");
    }
    Ok(preds)
}

/// `SharesColorWith Equipped`, `SharesColorWith Valid Creature.YouCtrl`: shares a color with one of the objects the
/// word names. Mana, a cast of this turn and a bare word stay gaps.
fn shares_color(ctx: &Ctx, word: &str) -> Tr<Vec<Value>> {
    let word = word.trim();
    let named = word.starts_with("Valid ") || matches!(word, "Equipped" | "Enchanted" | "Self");
    if !named {
        return unmapped(format!("valid:SharesColorWith {word}"));
    }
    let objects = super::valid::objects(super::valid::defined(ctx, word)?)?;
    Ok(vec![json!({ "SharesColor": objects })])
}

/// `powerLTCount$CardPower`: a stat of the object against the same or another stat of the card the ability belongs to.
fn against_source(prop: &str) -> Option<Value> {
    const LEFT: [(&str, &str); 3] = [
        ("power", "Power"),
        ("toughness", "Toughness"),
        ("cmc", "ManaValue"),
    ];
    const RIGHT: [(&str, &str); 3] = [
        ("Count$CardPower", "Power"),
        ("Count$CardToughness", "Toughness"),
        ("Count$CardManaCost", "ManaValue"),
    ];
    let (left, rest) = LEFT
        .iter()
        .find_map(|(word, stat)| prop.strip_prefix(word).map(|rest| (*stat, rest)))?;
    let op = super::valid::cmp(rest.get(..2)?)?;
    let right = RIGHT
        .iter()
        .find(|(word, _)| rest.get(2..) == Some(*word))
        .map(|(_, stat)| *stat)?;
    Some(json!({"Test": {
        "left": {"Stat": {"of": "Candidate", "stat": left}},
        "op": op,
        "right": {"Stat": {"of": "This", "stat": right}},
    }}))
}

/// `numColorsEQX`, `counters_GEX_CHARGE`: a characteristic or a counter count against a named part. A color count takes
/// any amount; a counter count only a part that is a written number.
fn counted_bound(ctx: &Ctx, prop: &str) -> Tr<Option<Value>> {
    let named = |bound: &str| {
        bound.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            && bound.chars().all(|c| c.is_ascii_alphanumeric())
    };
    if let Some(rest) = prop.strip_prefix("numColors")
        && let Some((op, bound)) = rest.get(..2).and_then(super::valid::cmp).zip(rest.get(2..))
        && named(bound)
    {
        let to = amount(ctx, bound)?;
        return Ok(Some(json!({"Compare": {"stat": "Colors", "op": op, "to": to}})));
    }
    if let Some(rest) = prop.strip_prefix("counters_")
        && let Some((bound, kind)) = rest.split_once('_')
        && let Some((op, part)) = bound.get(..2).and_then(super::valid::cmp).zip(bound.get(2..))
        && named(part)
    {
        let Some(to) = amount(ctx, part)?.as_u64().and_then(|n| u32::try_from(n).ok()) else {
            return unmapped(format!("valid:{prop}"));
        };
        let Some(kind) = super::amount::counter_kind(kind) else {
            return Ok(None);
        };
        return Ok(Some(
            json!({"CounterCount": {"kind": kind, "op": op, "to": to}}),
        ));
    }
    Ok(None)
}

/// `_Creature` after `greatestCMC`: the card type an extreme compares among and the rest of the word (a controller word,
/// `ControlledByRemembered`), `None` for a word that names no card type.
pub(super) fn extreme_kind(rest: &str) -> Option<(Vec<Value>, &str)> {
    let after = rest.strip_prefix('_')?;
    let end = after.find("ControlledBy").unwrap_or(after.len());
    let (kind, tail) = after.split_at(end);
    CARD_TYPES
        .contains(&kind)
        .then(|| (vec![json!({"Type": kind})], tail))
}

/// An atom that stays a gap: the word (or its head, before a space) and the fact of the game it needs.
pub struct Unmapped {
    /// The atom, matched whole or as the head before an argument.
    pub word: &'static str,
    /// What the engine or the selector vocabulary lacks.
    pub needs: &'static str,
}

/// The atoms in use in Forge's scripts that no reader maps, with the reason each stays a gap (`valid:<atom>`).
pub const UNMAPPED: &[Unmapped] = &[
    Unmapped { word: "hasManaAbility", needs: "the abilities an object has, as a selector" },
    Unmapped { word: "hasNonManaActivatedAbility", needs: "the abilities an object has, as a selector" },
    Unmapped { word: "NoAbilities", needs: "the abilities an object has, as a selector" },
    Unmapped { word: "hasAbility", needs: "the abilities an object has, as a selector" },
    Unmapped { word: "firstTurnControlled", needs: "the turn control of a permanent began" },
    Unmapped { word: "attackedLastTurn", needs: "the records of the previous turn" },
    Unmapped { word: "dealtDamagetoAny", needs: "damage records over the whole game" },
    Unmapped { word: "dealtCombatDamagetoAny", needs: "damage records over the whole game" },
    Unmapped { word: "Damaged", needs: "damage records to a source that has left the battlefield" },
    Unmapped { word: "Above", needs: "positions in a graveyard" },
    Unmapped { word: "DirectlyAbove", needs: "positions in a graveyard" },
    Unmapped { word: "BottomGraveyard", needs: "positions in a graveyard" },
    Unmapped { word: "BottomLibrary_Creature", needs: "the bottommost card of a kind in a library" },
    Unmapped { word: "withFlanking", needs: "the keyword flanking" },
    Unmapped { word: "withoutFlanking", needs: "the keyword flanking" },
    Unmapped { word: "withoutRampage", needs: "the keyword rampage" },
    Unmapped { word: "withWarp", needs: "the keyword warp" },
    Unmapped { word: "withAugment", needs: "the keyword augment" },
    Unmapped { word: "kicked 1", needs: "which of two kicker costs was paid" },
    Unmapped { word: "kicked 2", needs: "which of two kicker costs was paid" },
    Unmapped { word: "ManaSpent LTX", needs: "a bound read against the spell or the source by script" },
    Unmapped { word: "ManaSpent GTX", needs: "a bound read against the spell or the source by script" },
    Unmapped { word: "ManaFromCard.StrictlySelf", needs: "mana spent from one named source" },
    Unmapped { word: "canProduceManaColor", needs: "the mana an object could produce" },
    Unmapped { word: "CostsHybridMana", needs: "the symbols of a mana cost" },
    Unmapped { word: "CostsPhyrexianMana", needs: "the symbols of a mana cost" },
    Unmapped { word: "SharesCMCWith", needs: "a shared mana value predicate" },
    Unmapped { word: "sharesLandTypeWith", needs: "a shared land type predicate" },
    Unmapped { word: "sharesCreatureTypeWithAll", needs: "a type shared with each of several objects" },
    Unmapped { word: "sharesControllerWith", needs: "a shared controller predicate" },
    Unmapped { word: "sharesOwnerWith", needs: "a shared owner predicate" },
    Unmapped { word: "sharesBlockingAssignmentWith", needs: "the blocks of one attacker as a selector" },
    Unmapped { word: "attackingSame", needs: "the attacked player or planeswalker of another attacker" },
    Unmapped { word: "WithSameName", needs: "a group constraint of a cost" },
    Unmapped { word: "WithDifferentNames", needs: "a group constraint of a cost" },
    Unmapped { word: "withTypesGE4", needs: "a group constraint of a cost" },
    Unmapped { word: "withTotalCMCGE30", needs: "a group constraint of a cost" },
    Unmapped { word: "HasDevoured", needs: "what a permanent devoured" },
    Unmapped { word: "exploited", needs: "what a permanent exploited" },
    Unmapped { word: "foretold", needs: "foretold cards in exile" },
    Unmapped { word: "prepared", needs: "a prepared spell (the spell, not the permanent)" },
    Unmapped { word: "copiedSpell", needs: "copies of spells as a selector" },
    Unmapped { word: "activated", needs: "the loyalty activations of the turn" },
    Unmapped { word: "enlistedThisCombat", needs: "the enlist records of the combat" },
    Unmapped { word: "hadToAttackThisCombat", needs: "attack requirements as a selector" },
    Unmapped { word: "noName", needs: "nameless objects" },
    Unmapped { word: "EnemyColor", needs: "pairs of enemy colors" },
    Unmapped { word: "ColorsYouCtrl", needs: "the colors among your permanents" },
    Unmapped { word: "OwnerDoesntControl", needs: "the owner of the object itself as a reference" },
    Unmapped { word: "ControllerControlsMoreLand", needs: "counts of the candidate's controller" },
    Unmapped { word: "ControllerControlsLand_LE3", needs: "counts of the candidate's controller" },
    Unmapped { word: "OppProtect", needs: "the protector of a battle" },
    Unmapped { word: "EnteredUnder", needs: "the controller a permanent entered under" },
    Unmapped { word: "numTypesGE2", needs: "the number of card types of an object" },
    Unmapped { word: "hasANonBasicLandType", needs: "the nonbasic land types" },
    Unmapped { word: "yardGreatestPower", needs: "an extreme among graveyard cards" },
];

/// The row of an atom, matched whole or by the head before a space.
pub(super) fn unmapped_row(prop: &str) -> Option<&'static Unmapped> {
    let head = prop.split(' ').next().unwrap_or(prop);
    UNMAPPED.iter().find(|row| row.word == prop || row.word == head)
}
