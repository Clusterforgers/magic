/** The Delves run page's pure rules of display: room states and votes, boss life, life bars, stages and ready heroes. */
import type { DeckEntry } from '@/features/game/deckEntries'
import type {
  Brain,
  CustomCardView,
  DelveBoard,
  DelveBoardEntry,
  DelveContent,
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

/** The hero who breaks a tied vote: one seat on with every room cleared. */
export const torchOf = (run: Run): number =>
  run.heroes.length > 0 ? run.cleared.length % run.heroes.length : 0

/** One hero's part in the vote: the room they voted for, still to vote, or a bot ally that follows the players. */
export type Ballot =
  | { hero: number; name: string; state: 'voted'; node: number }
  | { hero: number; name: string; state: 'pending' | 'follows' }

/** Each hero's ballot, in party order; a bot that has not voted follows the players. */
export function ballots(run: Run): Ballot[] {
  return run.heroes.map((h, hero) => {
    const node = run.votes[hero]
    if (node !== undefined) return { hero, name: h.name, state: 'voted', node }
    return { hero, name: h.name, state: h.bot === null ? 'pending' : 'follows' }
  })
}

/** What the vote waits for, in one line for the player `me`. */
export function voteWaiting(run: Run, me: number | undefined): string {
  const pending = ballots(run).filter((b) => b.state === 'pending')
  const mine = me === undefined ? undefined : run.votes[me]
  const others = pending.filter((b) => b.hero !== me).map((b) => b.name)
  if (me !== undefined && mine === undefined) {
    return others.length === 0
      ? 'Everyone else voted: your vote decides.'
      : `Your vote is needed. Also waiting for ${listOf(others)}.`
  }
  if (others.length === 0) return 'Every player voted: the party moves on.'
  return `Waiting for ${listOf(others)} to vote. You can still change your vote.`
}

/** Names joined as prose: "Ana", "Ana and Bo", "Ana, Bo and Cy". */
export function listOf(names: readonly string[]): string {
  if (names.length <= 1) return names.join('')
  return `${names.slice(0, -1).join(', ')} and ${names.at(-1)}`
}

/** A Heat's difficulty tier (`docs/delves.md`, Difficulty): its name and what it asks of the party. */
export interface HeatTier {
  heat: number
  name: string
  text: string
}

const CASUAL: HeatTier = { heat: 0, name: 'Casual', text: 'any deck can win with some luck' }

/** The four Heats, from the gentlest. */
export const HEAT_TIERS: readonly HeatTier[] = [
  CASUAL,
  { heat: 1, name: 'Seasoned', text: 'needs a deck that suits the format' },
  { heat: 2, name: 'Heroic', text: 'needs a team of decks that work together and suit the format' },
  { heat: 3, name: 'Legendary', text: 'needs a tuned team, and opens the Ember Foundry' },
]

/** The tier of `heat`; a Heat above the table reads as the highest. */
export function heatTier(heat: number): HeatTier {
  return HEAT_TIERS[Math.min(Math.max(0, heat), HEAT_TIERS.length - 1)] ?? CASUAL
}

/** `heat` in words: "Heat 0 — Casual: any deck can win with some luck". */
export function heatWords(heat: number): string {
  const tier = heatTier(heat)
  return `Heat ${heat} — ${tier.name}: ${tier.text}`
}

const plural = (count: number, one: string, many: string) => `${count} ${count === 1 ? one : many}`

/** How deep a leaderboard run went: the floors cleared, then the bosses beaten below them. */
export function depthWords(entry: Pick<DelveBoardEntry, 'floors' | 'bosses'>): string {
  const parts = [
    entry.floors > 0 ? plural(entry.floors, 'floor', 'floors') : null,
    entry.bosses > 0 || entry.floors === 0 ? plural(entry.bosses, 'boss', 'bosses') : null,
  ]
  return parts.filter((part) => part !== null).join(' + ')
}

/** The leaderboard's boards from the greatest Heat to the lowest, whatever order they came in. */
export function orderBoards(boards: readonly DelveBoard[]): DelveBoard[] {
  return [...boards].sort((a, b) => b.heat - a.heat)
}

/** A boss's starting life against this party at its place on the ladder and the run's Heat, if the view lists it. */
export function bossLife(view: Pick<DelveView, 'boss_lives'>, boss: string): number | undefined {
  return view.boss_lives.find(([id]) => id === boss)?.[1]
}

/** The most life a hero carries without relic bonuses (`docs/delves.md`, Shape). */
export const MAX_LIFE = 40
/** The hearts a party starts with. */
export const HEARTS = 3

/** A hero's life cap: the base moved by the events that cut or raised it. */
export const maxLifeOf = (hero: Pick<Hero, 'life_cap'>): number => MAX_LIFE + hero.life_cap

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
  if (status === 'Room') return view.run.descending ? 'Loot before the descent' : 'At camp'
  if (status === 'FloorCleared') return `Floor ${view.run.floor} cleared`
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

/** A card of the content by id; a plain common relic of that name when the content lacks it. */
export function customCard(content: Pick<DelveContent, 'cards'>, id: string): CustomCardView {
  return (
    content.cards.find((c) => c.id === id) ?? {
      id,
      name: id,
      kind: 'Relic',
      type_line: 'Relic',
      text: '',
      art: '',
      role: '',
      price: 0,
      rarity: 'Common',
    }
  )
}

/**
 * How the last fight went, while its outcome is still the news: won (the party is at camp after a boss), lost in the
 * game, retreated from by the host, or lost because its table closed or expired without a result. The server words
 * the last chronicle line of a lost fight.
 */
export function lastFight(run: Run): 'won' | 'lost' | 'retreated' | 'closed' | null {
  const room = run.map.nodes.find((node) => node.id === run.at)
  if (run.status === 'Room' && room?.kind === 'boss' && run.cleared.includes(room.id)) return 'won'
  if (typeof run.status !== 'object' || !('Fight' in run.status)) return null
  const line = run.log.at(-1) ?? ''
  if (line.startsWith('The party was beaten back')) return 'lost'
  if (line.startsWith('The party retreated')) return 'retreated'
  if (line.startsWith('The party lost its table')) return 'closed'
  return null
}

/** Whether the party's room is the first boss of the ladder: no boss room lies in an earlier row, as `Run::boss_rank` is 0. */
export function atFirstBoss(run: Run): boolean {
  const row = run.map.nodes.find((node) => node.id === run.at)?.row ?? 0
  return !run.map.nodes.some((node) => node.boss != null && node.row < row)
}

/** How much of their life every hero regroups with after a lost fight: all of it before the first boss, half later. */
export const regroupLife = (run: Run): 'full' | 'half' => (atFirstBoss(run) ? 'full' : 'half')

/** What the party does next, in one line of help for the stage the run is in. */
export function nextStep(view: DelveView): string {
  const status = view.run.status
  if (status === 'Lobby')
    return 'Each hero chooses a Commander deck once, for the whole run; rewards later swap cards into it. The host descends when everyone is ready.'
  if (status === 'Map')
    return 'The boss is behind you. Choose the camp the party rests at before the next boss.'
  if (status === 'Room' && view.run.descending)
    return "Take the final boss's loot below. When every hero has answered, the party descends to the next floor."
  if (status === 'Room')
    return 'Take your pick below. When every hero has answered, the party moves on: after a boss it votes for a camp, after a camp it faces the next boss.'
  if (status === 'FloorCleared')
    return 'The final boss fell. Bank the win to end the run in victory, or descend to a harder floor.'
  if (status === 'Victory' || status === 'Defeat' || status === 'Abandoned') return ''
  return 'Enter the fight: it starts once every hero is at the table. Afterwards you come straight back here for the loot.'
}

/** One hero's part in the floor vote: descend, bank, still choosing, or a bot ally that votes after the players. */
export type FloorBallot = { hero: number; name: string; state: 'descend' | 'bank' | 'pending' | 'follows' }

/** Each hero's floor ballot, in party order. */
export function floorBallots(run: Run): FloorBallot[] {
  return run.heroes.map((h, hero) => {
    const vote = run.floor_votes[hero]
    if (vote !== undefined) return { hero, name: h.name, state: vote ? 'descend' : 'bank' }
    return { hero, name: h.name, state: h.bot === null ? 'pending' : 'follows' }
  })
}

/** The id of the theme of the floor the party is on, which picks the run page's look. */
export function floorTheme(run: Run): string {
  return run.floor_theme ?? run.theme
}

/**
 * The words of the notice that a descent led the party into another theme ("The party descends into the Drowned
 * Spire"), while none of the new floor's rooms is cleared and the run goes on; else null.
 */
export function descentNotice(view: DelveView): string | null {
  const run = view.run
  if (run.theme_from === undefined || run.cleared.length > 0) return null
  if (run.status === 'Victory' || run.status === 'Defeat' || run.status === 'Abandoned') return null
  return `The party descends into ${view.theme.replace(/^The /, 'the ')}`
}
