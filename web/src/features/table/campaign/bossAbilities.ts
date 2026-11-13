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

/** What an ability is to the fight: always on, the phase in force, a phase to come, the scheme now or next. */
export type BossAbilityKind = 'passive' | 'phase' | 'next-phase' | 'scheme' | 'omen'

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
    const kind = phaseNames.has(object.name) || card?.kind === 'Phase' ? 'phase' : 'passive'
    out.push(ability(`rule-${object.name}`, kind, object.name, content))
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
  if (view.scheme !== undefined) out.push(ability(`scheme-${view.scheme}`, 'scheme', view.scheme, content))
  if (view.omen !== undefined)
    out.push(ability(`omen-${view.omen}`, 'omen', view.omen, content, 'As its next turn begins'))
  return out
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
