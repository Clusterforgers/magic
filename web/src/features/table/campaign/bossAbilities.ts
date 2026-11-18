/**
 * A boss's abilities as the table explains them. Which abilities are in force comes from the game: the emblems in the
 * boss's command zone, its phases to come, its scheme in motion and its Omen (`PlayerView.boss`). Their rules text
 * comes from the campaign content (`GET /api/delves/content`): the same records of `mtg-campaign/content/cards.json`
 * whose abilities the engine plays, so the words and the rules cannot drift apart.
 */
import type {
  CustomCardView,
  DelveContent,
  ObjectId,
  ObjectView,
  PlayerId,
  PlayerView,
} from '@/protocol/generated'
import { BULWARK_NAME, COMMANDER_TEXT, commanderLabel } from './bossPhase'
import { isReworked, reworkedAbilities } from './reworkedBoss'

/**
 * What an ability is to the fight: always on, its Enrage, this fight's modifier, the boss's relic, the phase in force,
 * a phase to come, the scheme now or next.
 */
export type BossAbilityKind =
  'passive' | 'innate' | 'enrage' | 'modifier' | 'relic' | 'phase' | 'next-phase' | 'scheme' | 'omen'

/** The kind of an always-on custom card by its content kind, and when an Enrage begins. */
export function standingKind(card: CustomCardView | undefined): { kind: BossAbilityKind; when?: string } {
  switch (card?.kind) {
    case 'Enrage':
      return { kind: 'enrage', when: `From the heroes' turn ${card.turn}` }
    case 'Modifier':
    case 'Encounter':
      return { kind: 'modifier' }
    case 'BossRelic':
      return { kind: 'relic' }
    default:
      return { kind: 'passive' }
  }
}

/** When the Omen comes, from the boss turns still to begin before it. */
export function omenWhen(omenIn: number | undefined): string {
  if (omenIn === undefined || omenIn <= 1) return 'As its next turn begins'
  return `In ${omenIn} of its turns`
}

/** One boss ability: its name, its whole rules text and a one-line summary. */
export interface BossAbility {
  key: string
  kind: BossAbilityKind
  name: string
  /** The whole rules text; empty while the content is not known. */
  text: string
  /** The first sentence of the rules, without reminder text. */
  summary: string
  /** When a phase to come begins ("At 66% of its life"), or when the Omen comes; none for the others. */
  when?: string
}

/** The first sentence of `text` without reminder text in parentheses, for a line that is always visible. */
export function summaryOf(text: string): string {
  const plain = text.replace(/\s*\([^)]*\)/g, '').trim()
  const first = plain.split('\n')[0] ?? ''
  const sentence = /^(.+?[.!?])(\s|$)/.exec(first)
  return (sentence?.[1] ?? first).trim()
}

/** The content's card named `name`. */
function cardNamed(content: DelveContent | null, name: string): CustomCardView | undefined {
  return content?.cards.find((card) => card.name === name)
}

function ability(
  key: string,
  kind: BossAbilityKind,
  name: string,
  content: DelveContent | null,
  when?: string,
): BossAbility {
  const text = cardNamed(content, name)?.text ?? ''
  return { key, kind, name, text, summary: summaryOf(text), ...(when === undefined ? {} : { when }) }
}

/** The share of the boss's life at which each of its content phases begins, by the phase card's name. */
function phaseShares(content: DelveContent | null, boss: string): Map<string, number> {
  const shares = new Map<string, number>()
  const entry = content?.bosses.find((candidate) => candidate.name === boss)
  for (const [id, percent] of entry?.phases ?? []) {
    const card = content?.cards.find((candidate) => candidate.id === id)
    if (card !== undefined) shares.set(card.name, percent)
  }
  return shares
}

/**
 * The abilities of the boss `player` (named `boss`), in the order a player reads them: what is always on (its
 * boss card, Escalation, Enrage and what the party's size added), the phase in force, the phases to come with the
 * share of its life at which each begins, the scheme in motion and the Omen.
 */
export function bossAbilities(
  player: PlayerView,
  boss: string,
  objects: Readonly<Record<ObjectId, ObjectView>>,
  content: DelveContent | null,
): BossAbility[] {
  const view = player.boss
  if (view === undefined || view === null) return []
  // A reworked boss's view carries its parts with their words; its emblems would only repeat them.
  if (isReworked(view)) return reworkedAbilities(view)
  const phaseNames = new Set(view.phases)
  const out: BossAbility[] = []
  const seen = new Map<string, number>()
  for (const id of player.command.cards ?? []) {
    const object = objects[id]
    if (object === undefined) continue
    const card = cardNamed(content, object.name)
    // The command zone also holds the boss's commander; only its boss abilities are listed here.
    if (card === undefined && !phaseNames.has(object.name)) continue
    // An emblem given twice (Reinforcements against a big party) is listed once with its count.
    const count = (seen.get(object.name) ?? 0) + 1
    seen.set(object.name, count)
    if (count > 1) continue
    if (phaseNames.has(object.name) || card?.kind === 'Phase') {
      out.push(ability(`rule-${object.name}`, 'phase', object.name, content))
      continue
    }
    const { kind, when } = standingKind(card)
    out.push(ability(`rule-${object.name}`, kind, object.name, content, when))
  }
  for (const entry of out) {
    const count = seen.get(entry.name) ?? 1
    if (count > 1) entry.name = `${entry.name} ×${count}`
  }
  const shares = phaseShares(content, boss)
  const last = view.phases.length - 1
  view.phases.forEach((name, at) => {
    if (at < view.begun) return
    const share = shares.get(name)
    const when =
      share !== undefined ? `At ${share}% of its life` : at === last ? 'With its last phase' : undefined
    out.push(ability(`phase-${name}`, 'next-phase', name, content, when))
  })
  const own = view.phases.filter((name) => name !== BULWARK_NAME).length
  if (view.commander !== undefined && view.begun < own) {
    const name = commanderLabel(view.phases, view.commander)
    out.push({
      key: `commander-${view.commander}`,
      kind: 'next-phase',
      name,
      text: COMMANDER_TEXT,
      summary: summaryOf(COMMANDER_TEXT),
      when: 'With its last phase',
    })
  }
  if (view.scheme !== undefined) out.push(ability(`scheme-${view.scheme}`, 'scheme', view.scheme, content))
  if (view.omen !== undefined)
    out.push(ability(`omen-${view.omen}`, 'omen', view.omen, content, omenWhen(view.omen_in)))
  return out
}

/** The order of the table's row, which keeps what changes the fight now in view: the phase in force, the scheme, the Omen, the phases to come, then what is always on. */
const BAR_RANK: Record<BossAbilityKind, number> = {
  phase: 0,
  scheme: 1,
  omen: 2,
  'next-phase': 3,
  innate: 4,
  enrage: 5,
  modifier: 6,
  relic: 7,
  passive: 8,
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
