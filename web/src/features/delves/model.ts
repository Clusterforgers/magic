/** The Delves run page's pure rules of display: room states and votes, boss life, life bars, stages and ready heroes. */
import type { DeckEntry } from '@/features/game/deckEntries'
import { summaryOf } from '@/features/table/campaign/bossAbilities'
import type { BossAbility } from '@/features/table/campaign/bossAbilities'
import type {
  Brain,
  CustomCardView,
  DelveContent,
  DelveFightView,
  DelveView,
  Hero,
  Node,
  Run,
} from '@/protocol/generated'

/** Where a room stands for the party. */
export type NodeState = 'cleared' | 'current' | 'next' | 'locked'

/** The rooms the party votes between now: the next rooms of where it is, while it chooses a path. */
export function openChoices(run: Run): number[] {
  if (run.status !== 'Map' || run.at === null) return []
  return run.map.nodes.find((n) => n.id === run.at)?.next ?? []
}

/** The state of a room: the room the party is in, one it cleared, one it may vote for now, or one still out of reach. */
export function nodeState(node: Node, run: Run): NodeState {
  const over = run.status === 'Victory' || run.status === 'Defeat' || run.status === 'Abandoned'
  if (run.at === node.id && !over && run.status !== 'Map') return 'current'
  if (run.cleared.includes(node.id)) return 'cleared'
  if (openChoices(run).includes(node.id)) return 'next'
  if (run.at === node.id) return 'current'
  return 'locked'
}

/** The heroes who voted for a room, by name. */
export function votersOf(run: Run, node: number): string[] {
  return Object.entries(run.votes)
    .filter(([, v]) => v === node)
    .map(([hero]) => run.heroes[Number(hero)]?.name ?? hero)
}

/** A boss's life for this party and Heat (`docs/delves.md`, Bosses): a fifth more from Heat 1. */
export function bossLife(life: readonly [number, number], heroes: number, heat: number): number {
  const base = life[0] + life[1] * Math.max(0, heroes - 1)
  return heat >= 1 ? Math.round(base * 1.2) : base
}

/** The most life a hero carries without relic bonuses (`docs/delves.md`, Shape). */
export const MAX_LIFE = 40
/** The hearts a party starts with. */
export const HEARTS = 3

/** The share of a life bar a life fills, 0 to 1. */
export const lifeShare = (life: number, max = MAX_LIFE): number => Math.max(0, Math.min(1, life / max))

/** The tone of a life: low at a quarter or less, hurt below half. */
export function lifeTone(life: number, max = MAX_LIFE): 'low' | 'hurt' | 'fine' {
  if (life <= max / 4) return 'low'
  return life < max / 2 ? 'hurt' : 'fine'
}

/** Where the run is, in a few words. */
export function runStage(view: DelveView): string {
  const status = view.run.status
  if (status === 'Lobby') return 'Gathering the party'
  if (status === 'Map') return 'Choosing a path'
  if (status === 'Room') return 'At camp'
  if (status === 'Victory') return 'Victory'
  if (status === 'Defeat') return 'Defeated'
  if (status === 'Abandoned') return 'Turned back'
  return 'Boss fight'
}

/** A brain's name in words. */
export const brainText = (brain: Brain): string => (brain === 'VeryHard' ? 'Very hard' : brain)

/** Whether a hero is ready to descend: they bring a deck. */
export const isReady = (hero: Hero): boolean => hero.deck !== null

/** The identity letters of an entry as one string. */
export const lettersOf = (entry: DeckEntry): string => (entry.identity ?? []).join('')

/** Whether a card is a risky relic: its effect is symmetrical and helps the boss too. */
export const isRisky = (card: Pick<CustomCardView, 'role'>): boolean => card.role === 'Risky'

/**
 * The boss's abilities for the intro of a fight, from the plan its game is made with: each custom card in its command
 * zone (one given twice once, with its count), then its phases with the life at which each begins. The words are the
 * content's, the records the engine plays.
 */
export function introAbilities(content: DelveContent, fight: DelveFightView): BossAbility[] {
  const counts = new Map<string, number>()
  for (const id of fight.rules) counts.set(id, (counts.get(id) ?? 0) + 1)
  const out: BossAbility[] = []
  for (const [id, count] of counts) {
    const card = content.cards.find((entry) => entry.id === id)
    if (card === undefined) continue
    const name = count > 1 ? `${card.name} ×${count}` : card.name
    out.push({ key: `rule-${id}`, kind: 'passive', name, text: card.text, summary: summaryOf(card.text) })
  }
  for (const [id, life] of fight.phases) {
    const card = content.cards.find((entry) => entry.id === id)
    if (card === undefined) continue
    const share = fight.life > 0 ? Math.round((life / fight.life) * 100) : 0
    out.push({
      key: `phase-${id}`,
      kind: 'next-phase',
      name: card.name,
      text: card.text,
      summary: summaryOf(card.text),
      when: life >= fight.life ? 'From the start' : `At ${life} life (${share}%)`,
    })
  }
  return out
}

/** How the last fight went, while its outcome is still the news: won (the party is at camp after a boss) or lost. */
export function lastFight(run: Run): 'won' | 'lost' | null {
  const room = run.map.nodes.find((node) => node.id === run.at)
  if (run.status === 'Room' && room?.kind === 'boss' && run.cleared.includes(room.id)) return 'won'
  if (
    typeof run.status === 'object' &&
    'Fight' in run.status &&
    run.log.at(-1)?.startsWith('The party was beaten back')
  )
    return 'lost'
  return null
}

/** What the party does next, in one line of help for the stage the run is in. */
export function nextStep(view: DelveView): string {
  const status = view.run.status
  if (status === 'Lobby')
    return 'Each hero chooses a Commander deck once, for the whole run; rewards later swap cards into it. The host descends when everyone is ready.'
  if (status === 'Map')
    return 'Vote for the next camp on the descent below. When every hero has voted the party moves on; a tie goes to the torch bearer.'
  if (status === 'Room')
    return 'At camp: take your pick below. When every hero has answered, the party votes for the next room.'
  if (status === 'Victory' || status === 'Defeat' || status === 'Abandoned') return ''
  return 'Enter the fight: it starts once every hero is at the table. Afterwards you come straight back here for the loot.'
}
