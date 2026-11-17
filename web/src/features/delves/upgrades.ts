/** Pure words of commander upgrades and the Satchel, as the run page shows them (`mtg-campaign`, upgrade.rs). */
import type { Gift, Upgrade } from '@/protocol/generated'

/** The price of a commander upgrade at the shop (`mtg-campaign`, run.rs). */
export const UPGRADE_PRICE = 50

const GIFT_WORDS: Record<Gift, string> = {
  Flying: 'flying',
  Trample: 'trample',
  Vigilance: 'vigilance',
  Lifelink: 'lifelink',
  Haste: 'haste',
  Menace: 'menace',
  FirstStrike: 'first strike',
  Deathtouch: 'deathtouch',
  Hexproof: 'hexproof',
}

/** An upgrade in words, as the server says it: "Atraxa costs {1} less to cast", "Atraxa has flying", "Ruric gets −1/−1". */
export function upgradeWords({ commander, boon }: Upgrade): string {
  if (boon === 'Counter') return `${commander} enters with an additional +1/+1 counter`
  if (boon === 'Loyalty') return `${commander} enters with an additional loyalty counter`
  if ('Cheaper' in boon) return `${commander} costs {${boon.Cheaper}} less to cast`
  if ('Pump' in boon) return `${commander} gets +${boon.Pump}/+${boon.Pump}`
  if ('Tax' in boon) return `${commander} costs {${boon.Tax}} more to cast`
  if ('Shrink' in boon) return `${commander} gets −${boon.Shrink}/−${boon.Shrink}`
  return `${commander} has ${GIFT_WORDS[boon.Keyword]}`
}

/** Whether an upgrade is a bane an event cost: a tax or a shrink. */
export const isBane = ({ boon }: Upgrade): boolean =>
  typeof boon === 'object' && ('Tax' in boon || 'Shrink' in boon)

/** What a Satchel holds in words: "Satchel: Sol Ring, Opt", or "Satchel empty". */
export function satchelWords(cards: readonly string[]): string {
  return cards.length === 0 ? 'Satchel empty' : `Satchel: ${cards.join(', ')}`
}
