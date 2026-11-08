//! Forge's defined references as one reader table: each `Defined$` or `RememberObjects$` word, the reference the engine
//! holds for it, the scopes it is legal in and a probe card. Unknown words and words outside their scopes stay gaps.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use super::memory;

/// Where a word is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// A `Defined$` word, read as the line resolves.
    Now,
    /// What an `Effect`'s statics remember: read once as the effect begins (CR 611.2c).
    Fixed,
    /// What an `Effect`'s triggers or shields remember, read again while they last: only a word that names the same
    /// objects throughout.
    Lasting,
    /// What a reflexive trigger is about (`ImmediateTrigger`), bound as it is set up.
    Reflexive,
    /// What `ImprintCards$` imprints.
    Imprint,
}

impl Scope {
    const fn bit(self) -> u8 {
        match self {
            Scope::Now => 1,
            Scope::Fixed => 2,
            Scope::Lasting => 4,
            Scope::Reflexive => 8,
            Scope::Imprint => 16,
        }
    }
}

const NOW: u8 = Scope::Now.bit();
const FIXED: u8 = Scope::Fixed.bit();
const LASTING: u8 = Scope::Lasting.bit();
const REFLEXIVE: u8 = Scope::Reflexive.bit();
const IMPRINT: u8 = Scope::Imprint.bit();
/// A word read as the line resolves: it is also what an effect's statics fix as they begin.
const RESOLVES: u8 = NOW | FIXED;
/// A word naming one player or the source, which no move of an object changes: also what a reflexive trigger binds.
const STABLE_PLAYER: u8 = NOW | FIXED | REFLEXIVE;

/// The reference a row reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reads {
    You,
    Opponents,
    EachPlayer,
    This,
    Paid,
    Convoked,
    ExiledWith,
    TopOfLibrary,
    Target,
    Parent,
    TargetController,
    TargetOrController,
    TargetOwner,
    ParentController,
    Attached,
    AttachedController,
    AttachedOwner,
    AttachedPlayer,
    Remembered,
    RememberedOwner,
    RememberedController,
    Imprinted,
    ImprintedController,
    SourceController,
    Defending,
    ActivePlayer,
}

/// One word of the table.
struct Row {
    words: &'static [&'static str],
    reads: Reads,
    scopes: u8,
    /// A card whose script reads the word this way.
    #[allow(dead_code)] // documentation of the row, read by the table test
    probe: &'static str,
}

const fn row(words: &'static [&'static str], reads: Reads, scopes: u8, probe: &'static str) -> Row {
    Row {
        words,
        reads,
        scopes,
        probe,
    }
}

/// The reader table.
const ROWS: &[Row] = &[
    row(&["You"], Reads::You, STABLE_PLAYER, "Divination"),
    // Teams do not exist at our tables: the other players are the opponents.
    row(
        &["Opponent", "Player.Opponent", "Player.Other"],
        Reads::Opponents,
        RESOLVES,
        "Syphon Mind",
    ),
    row(&["Player"], Reads::EachPlayer, RESOLVES, "Howling Mine"),
    row(
        &["Self", "Card.Self", "CorrectedSelf"],
        Reads::This,
        NOW | FIXED | LASTING | IMPRINT,
        "Giant Growth",
    ),
    row(&["Sacrificed", "Discarded"], Reads::Paid, NOW, "Fling"),
    row(&["Convoked"], Reads::Convoked, NOW, "Venerated Loxodon"),
    // The cards this permanent exiled and that are still in exile.
    row(&["ExiledWith"], Reads::ExiledWith, NOW, "Ixalan's Binding"),
    row(&["TopOfLibrary"], Reads::TopOfLibrary, NOW, "Mind's Eye"),
    row(
        &["Targeted"],
        Reads::Target,
        NOW | FIXED | LASTING | IMPRINT,
        "Act of Treason",
    ),
    row(
        &["ThisTargetedCard", "TargetedCard"],
        Reads::Target,
        RESOLVES | LASTING,
        "Reflector Mage",
    ),
    row(
        &["TargetedPlayer"],
        Reads::Target,
        RESOLVES | LASTING,
        "Autumn Willow",
    ),
    row(
        &["ParentTarget"],
        Reads::Parent,
        RESOLVES | LASTING | IMPRINT,
        "Ogre Geargrabber",
    ),
    row(
        &["TargetedController"],
        Reads::TargetController,
        RESOLVES,
        "Render Silent",
    ),
    row(
        &["TargetedOrController"],
        Reads::TargetOrController,
        RESOLVES,
        "Lightning Helix",
    ),
    row(
        &["TargetedOwner", "ThisTargetedOwner"],
        Reads::TargetOwner,
        RESOLVES,
        "Oust",
    ),
    row(
        &["ParentTargetedController"],
        Reads::ParentController,
        RESOLVES,
        "Draconic Roar",
    ),
    row(
        &["RememberedOwner"],
        Reads::RememberedOwner,
        RESOLVES,
        "Reflector Mage",
    ),
    row(
        &["Enchanted", "Equipped"],
        Reads::Attached,
        RESOLVES | IMPRINT,
        "Wings of Hubris",
    ),
    row(
        &["EnchantedController", "EquippedController"],
        Reads::AttachedController,
        RESOLVES,
        "Sleep Paralysis",
    ),
    row(
        &["EnchantedOwner", "EquippedOwner"],
        Reads::AttachedOwner,
        RESOLVES,
        "Enslave",
    ),
    row(
        &["EnchantedPlayer", "Player.EnchantedBy"],
        Reads::AttachedPlayer,
        RESOLVES,
        "Curse of Death's Hold",
    ),
    row(
        &["RememberedController"],
        Reads::RememberedController,
        NOW,
        "Acolyte Hybrid",
    ),
    row(
        &["ImprintedController"],
        Reads::ImprintedController,
        NOW,
        "Mimic Vat",
    ),
    row(
        &[
            "DelayTriggerRemembered",
            "DelayTriggerRememberedLKI",
            "DelayedTriggerRemembered",
            "RememberedLKI",
            "RememberedCard",
            "DirectRemembered",
            "Remembered",
        ],
        Reads::Remembered,
        NOW,
        "Mirror-Style Master",
    ),
    // The source's memory, or the name an earlier step of the chain bound: the same objects while the effect lasts.
    row(
        &["Remembered", "RememberedCard"],
        Reads::Remembered,
        FIXED | LASTING | REFLEXIVE | IMPRINT,
        "Ugin, the Ineffable",
    ),
    row(
        &["RememberedLKI"],
        Reads::Remembered,
        REFLEXIVE,
        "Extract a Confession",
    ),
    row(
        &["Imprinted", "Player.IsImprinted"],
        Reads::Imprinted,
        NOW,
        "Soul Foundry",
    ),
    row(
        &["Imprinted"],
        Reads::Imprinted,
        FIXED | IMPRINT,
        "Summoner's Sending",
    ),
    // The controller of the card the ability belongs to, who need not be the player who activated it.
    row(
        &["SourceController"],
        Reads::SourceController,
        RESOLVES,
        "Ashling's Prerogative",
    ),
    row(
        &[
            "DefendingPlayer",
            "TriggeredDefendingPlayer",
            "Spawner>TriggeredDefendingPlayer",
        ],
        Reads::Defending,
        NOW,
        "Raid Bombardment",
    ),
    row(
        &["ActivePlayer"],
        Reads::ActivePlayer,
        RESOLVES,
        "Blightwing Bandit",
    ),
];

fn find(word: &str, scope: Scope) -> Option<Reads> {
    let word = word.trim();
    ROWS.iter()
        .find(|row| row.scopes & scope.bit() != 0 && row.words.contains(&word))
        .map(|row| row.reads)
}

/// Whether the table has a row for the word in the scope.
pub fn knows(word: &str, scope: Scope) -> bool {
    find(word, scope).is_some()
}

/// The reference the word stands for in the scope; `None` when the table has no row for it there.
pub fn read(ctx: &Ctx, word: &str, scope: Scope) -> Option<Tr<Value>> {
    find(word, scope).map(|reads| reference(ctx, reads))
}

/// The reference of a word in a scope, or the gap `gap` names when the table has no row for it there.
pub fn read_or(ctx: &Ctx, word: &str, scope: Scope, gap: impl FnOnce() -> String) -> Tr<Value> {
    read(ctx, word, scope).unwrap_or_else(|| unmapped(gap()))
}

fn reference(ctx: &Ctx, reads: Reads) -> Tr<Value> {
    let controller = |inner: Value| Ok(json!({ "ControllerOf": inner }));
    match reads {
        Reads::You => Ok(json!("You")),
        Reads::Opponents => Ok(json!("Opponents")),
        Reads::EachPlayer => Ok(json!("EachPlayer")),
        Reads::This => Ok(json!("This")),
        Reads::Paid => Ok(json!("Paid")),
        Reads::Convoked => Ok(json!("Convoked")),
        Reads::ExiledWith => Ok(json!({"Each": [{"ExiledWith": "This"}, {"In": "Exile"}]})),
        Reads::TopOfLibrary => {
            Ok(json!({"Each": [{"In": "Library"}, {"OwnedBy": "You"}, "TopOfLibrary"]}))
        }
        Reads::Target => ctx.last_target(),
        Reads::Parent => ctx.parent(),
        Reads::TargetController => controller(ctx.last_target()?),
        Reads::TargetOrController => Ok(json!({"OrController": ctx.last_target()?})),
        Reads::TargetOwner => Ok(json!({ "OwnerOf": ctx.last_target()? })),
        Reads::ParentController => controller(ctx.parent()?),
        Reads::Attached => Ok(json!("Attached")),
        Reads::AttachedController => controller(json!("Attached")),
        Reads::AttachedOwner => Ok(json!({"OwnerOf": "Attached"})),
        Reads::AttachedPlayer => Ok(json!("AttachedPlayer")),
        Reads::Remembered => memory::remembered(ctx),
        Reads::RememberedOwner => Ok(json!({ "OwnerOf": memory::remembered(ctx)? })),
        Reads::RememberedController => memory::remembered_controller(ctx),
        Reads::Imprinted => memory::imprinted(ctx),
        Reads::ImprintedController => memory::imprinted_controller(ctx),
        Reads::SourceController => controller(json!("This")),
        Reads::Defending => Ok(json!("Defending")),
        Reads::ActivePlayer => Ok(json!("ActivePlayer")),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every row of the reader table names words, a scope and a probe, and no word reads two ways in one scope.
    use super::{ROWS, Scope};

    #[test]
    fn rows_are_whole_and_unique_per_scope() {
        for scope in [
            Scope::Now,
            Scope::Fixed,
            Scope::Lasting,
            Scope::Reflexive,
            Scope::Imprint,
        ] {
            let mut seen = std::collections::BTreeSet::new();
            for row in ROWS.iter().filter(|row| row.scopes & scope.bit() != 0) {
                assert!(!row.probe.is_empty() && !row.words.is_empty());
                for word in row.words {
                    assert!(seen.insert(*word), "{word} reads twice in {scope:?}");
                }
            }
        }
    }
}
