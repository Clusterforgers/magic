/** Pure words and figures of a seat pod: what each zone chip is called, what a tile says, the state tokens and a seat's colour. */
import type { PlayerId, PlayerView } from '@/protocol/generated'

/** The zones a pod counts. */
export type ZoneKey = 'hand' | 'library' | 'graveyard' | 'exile' | 'command'

/** A zone as the chips and dialogs name it. */
export const ZONE_NAME: Record<ZoneKey, string> = {
  hand: 'Hand',
  library: 'Library',
  graveyard: 'Graveyard',
  exile: 'Exile',
  command: 'Command zone',
}

/** The name of a zone in a picture too narrow for the whole word. */
export const ZONE_SHORT: Record<ZoneKey, string> = {
  hand: 'Hand',
  library: 'Lib',
  graveyard: 'Grave',
  exile: 'Exile',
  command: 'Cmd',
}

/** From this picture width (px) the label of an empty zone is written out; under it the short form fits. */
export const FULL_LABEL_MIN_PX = 60

/** The label of an empty zone's outline: its name, or the short form in a picture too narrow for it. */
export function emptyLabel(zone: ZoneKey, width: number): string {
  return width >= FULL_LABEL_MIN_PX ? ZONE_NAME[zone] : ZONE_SHORT[zone]
}

/** "1 card" or "7 cards". */
export function cardCount(count: number): string {
  return count === 1 ? '1 card' : `${count} cards`
}

/** What a chip is called: "Graveyard 5 cards". */
export function zoneLabel(zone: ZoneKey, count: number): string {
  return `${ZONE_NAME[zone]} ${cardCount(count)}`
}

/** What a chip adds to its name when `legal` cards of the zone can be played or used now: "2 can be played or used"; empty for none. */
export function legalWords(legal: number): string {
  return legal === 0 ? '' : `${legal} can be played or used`
}

/** The tooltip of a zone chip or picture: its name, how many cards in it can be played or used, and its top card when it shows one. */
export function zoneTitle(zone: ZoneKey, count: number, legal: number, top?: string): string {
  return [zoneLabel(zone, count), legalWords(legal), top === undefined ? '' : `Top card: ${top}`]
    .filter((part) => part !== '')
    .join('. ')
}

/** A zone's name for a screen reader, the list it opens and, when cards in it can be played or used now, how many. */
export function zoneOpenLabel(zone: ZoneKey, count: number, legal: number): string {
  const label = zoneLabel(zone, count)
  return legal === 0 ? `${label}, opens the list` : `${label}, ${legalWords(legal)}, opens the list`
}

/** The cards in each zone of a pod. */
export type PodCounts = Record<ZoneKey, number>

/** The counts of a seat; exile is one shared zone, so its count is the cards `seat` owns (`exileOf`). */
export function podCounts(player: PlayerView, exile: number): PodCounts {
  return {
    hand: player.hand.count,
    library: player.library.count,
    graveyard: player.graveyard.count,
    exile,
    command: player.command.count,
  }
}

/** The zones a pod draws as chips: the four that always count, and the command zone once it holds a card. */
export function chipZones(counts: PodCounts): ZoneKey[] {
  return counts.command > 0
    ? ['hand', 'library', 'graveyard', 'exile', 'command']
    : ['hand', 'library', 'graveyard', 'exile']
}

/**
 * The `data-state` of a pod: `active` and `priority` as space-separated tokens (match one with `[data-state~='active']`),
 * or undefined while the seat is neither. The turn cues style them; the pod only carries them.
 */
export function podState(active: boolean, priority: boolean): string | undefined {
  const tokens = [active ? 'active' : null, priority ? 'priority' : null].filter(
    (token): token is string => token !== null,
  )
  return tokens.length === 0 ? undefined : tokens.join(' ')
}

/** Whether a seat draws the priority dot: it holds priority and is not the active seat, whose edge marks the turn. */
export function showsPriorityDot(active: boolean, priority: boolean): boolean {
  return priority && !active
}

/** What the priority dot is called: "Bo has priority", or "You have priority" on the viewer's own pod. */
export function priorityName(name: string, you: boolean): string {
  return you ? 'You have priority' : `${name} has priority`
}

/**
 * The sentence for a screen reader that says what a pod's marks say beyond the priority dot, which has its own name:
 * "Active turn." or "Active turn. Has priority."; empty for a seat that has no turn.
 */
export function podStateWords(active: boolean, priority: boolean): string {
  if (!active) return ''
  return priority ? 'Active turn. Has priority.' : 'Active turn.'
}

/** What a tile of the strip is called: its action and then everything the tile shows, in words. */
export interface TileFacts {
  name: string
  life: number
  counts: PodCounts
  active: boolean
  priority: boolean
  eliminated: boolean
  /** No client is connected to the seat (it is still in the game). */
  away?: boolean
  focused: boolean
  /** The commander tax and damage chips in words; none by default. */
  commander?: readonly string[]
  /** The Auras and Curses attached to the player, in words; none by default. */
  enchanted?: readonly string[]
  /** What the seat shows to every seat, in words ("revealed 2 cards"); left out while it shows nothing. */
  revealed?: string | undefined
}

/** The accessible name of a tile: "Show Bo's board. Life 38, 6 cards in hand, ..., active turn". */
export function tileName(facts: TileFacts): string {
  const { counts } = facts
  const figures = [
    `Life ${facts.life}`,
    `${cardCount(counts.hand)} in hand`,
    `${counts.library} in library`,
    `${counts.graveyard} in graveyard`,
    `${counts.exile} in exile`,
    counts.command > 0 ? `${counts.command} in the command zone` : null,
    facts.revealed ?? null,
    ...(facts.commander ?? []),
    ...(facts.enchanted ?? []),
    facts.active ? 'active turn' : null,
    facts.priority ? 'has priority' : null,
    facts.eliminated ? 'eliminated' : null,
    facts.away === true ? 'away' : null,
    facts.focused ? 'shown now' : null,
  ]
  return `Show ${facts.name}'s board. ${figures.filter((part) => part !== null).join(', ')}`
}

/** The angle of the golden ratio in degrees: neighbours on the colour wheel land far apart however many seats sit. */
const GOLDEN_ANGLE = 137.508

/** The hue (0 to 360 degrees) of a seat's dot, the same for a seat wherever it is drawn. */
export function seatHue(seat: PlayerId): number {
  const turns = Number.isFinite(seat) ? Math.trunc(seat) : 0
  return Math.round(((((turns * GOLDEN_ANGLE + 262) % 360) + 360) % 360) * 10) / 10
}
