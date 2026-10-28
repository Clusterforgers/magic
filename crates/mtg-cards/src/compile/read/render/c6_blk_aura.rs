//! The sentences of an enchantment that becomes an Aura on a manifested card, as `compile/read/c6_blk_aura.rs` reads
//! them.

use super::{Effect, Writer};
use crate::compile::read::c6_blk_aura::c6_blk_aura_effects;

impl Writer<'_> {
    /// "it becomes an Aura with enchant creature" and "manifest the top card of your library and attach ~ to it", and
    /// the three effects they cover.
    pub(super) fn c6_blk_aura_manifest(effects: &[Effect]) -> Option<(Vec<String>, usize)> {
        let wanted = c6_blk_aura_effects();
        (effects.get(..wanted.len())? == wanted.as_slice()).then_some(())?;
        Some((
            vec![
                "it becomes an Aura with enchant creature".to_owned(),
                "manifest the top card of your library and attach ~ to it".to_owned(),
            ],
            wanted.len(),
        ))
    }
}
