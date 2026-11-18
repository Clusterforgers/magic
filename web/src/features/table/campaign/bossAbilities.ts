/**
 * A boss's abilities as the table explains them, from the public `PlayerView.boss`: its innate with its level, the
 * fight's modifier, its relics and its two phases by round, each card carrying its own rules text.
 */
import type { ObjectView, PlayerId, PlayerView } from '@/protocol/generated'
import { reworkedAbilities } from './reworkedBoss'

/** What an ability is to the fight: its innate, this fight's modifier, a relic it brings, the phase in force, a phase to come. */
export type BossAbilityKind = 'innate' | 'modifier' | 'relic' | 'phase' | 'next-phase'

/** One boss ability: its name, its whole rules text and a one-line summary. */
export interface BossAbility {
  key: string
  kind: BossAbilityKind
  name: string
  /** The whole rules text. */
  text: string
  /** The first sentence of the rules, without reminder text. */
  summary: string
  /** The innate's level, or when a phase begins ("Phase 1 at round 8"); none for the others. */
  when?: string
}

/** The first sentence of `text` without reminder text in parentheses, for a line that is always visible. */
export function summaryOf(text: string): string {
  const plain = text.replace(/\s*\([^)]*\)/g, '').trim()
  const first = plain.split('\n')[0] ?? ''
  const sentence = /^(.+?[.!?])(\s|$)/.exec(first)
  return (sentence?.[1] ?? first).trim()
}

/** The abilities of the boss `player` in reading order; none for a seat that is no boss. */
export function bossAbilities(player: PlayerView): BossAbility[] {
  const view = player.boss
  return view === undefined || view === null ? [] : reworkedAbilities(view)
}

/** The order of the table's row, which keeps what changes the fight now in view: the phase in force, the phases to come, then what is always on. */
const BAR_RANK: Record<BossAbilityKind, number> = {
  phase: 0,
  'next-phase': 1,
  innate: 2,
  modifier: 3,
  relic: 4,
}

/** `abilities` in the order of the table's row (`BAR_RANK`), each kind keeping its own order. */
export function barOrder(abilities: readonly BossAbility[]): BossAbility[] {
  return abilities
    .map((ability, at) => ({ ability, at }))
    .sort((a, b) => BAR_RANK[a.ability.kind] - BAR_RANK[b.ability.kind] || a.at - b.at)
    .map(({ ability }) => ability)
}

/** The name of the boss whose command zone holds `object`, one of its boss abilities; undefined otherwise. */
export function bossOwner(
  object: ObjectView | undefined,
  players: readonly PlayerView[] | undefined,
  seat: (seat: PlayerId) => string,
): string | undefined {
  const zone = object?.zone
  if (zone === undefined || typeof zone !== 'object' || !('Command' in zone)) return undefined
  const owner = players?.find((player) => player.seat === zone.Command)
  return owner?.boss === undefined || owner.boss === null ? undefined : seat(owner.seat)
}
