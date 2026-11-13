/**
 * The Restless Spirit at the table (docs/delves.md, Spirits): the haunt actions of a fallen hero's emblem in plain
 * words, the ectoplasm it holds and which actions it can take now. Pure; the panel and the log read it.
 */
import type { AbilityRef, ObjectId, ObjectView, PlayerView, PriorityOptions } from '@/protocol/generated'

/** The emblem's name, as the content and the engine give it. */
export const SPIRIT_EMBLEM = 'Restless Spirit'

/** A haunt action: its name, its ectoplasm price and what it does, in plain words. */
export interface HauntAction {
  /** Its index among the emblem's abilities. */
  index: number
  name: string
  ectoplasm: number
  /** What it does, in one plain sentence. */
  plain: string
  /** Its effect as a short preview: "Boss −2 life". */
  preview: string
  /** Only in the Spirit's own main phase. */
  sorcery?: boolean
}

/** The haunt actions of the Restless Spirit, in the order of its abilities (ability 0 gives ectoplasm). */
export const HAUNT_ACTIONS: readonly HauntAction[] = [
  {
    index: 1,
    name: 'Wail',
    ectoplasm: 1,
    plain: 'Shriek at an enemy: they lose 2 life and their creatures hit for 1 less this turn.',
    preview: 'Enemy −2 life · creatures −1/−0',
  },
  {
    index: 2,
    name: 'Ward',
    ectoplasm: 1,
    plain: 'Shield your living allies: the next 3 damage to each of them this turn is prevented.',
    preview: 'Each ally: prevent 3 damage',
  },
  {
    index: 3,
    name: 'Whisper',
    ectoplasm: 1,
    plain:
      'Whisper to your allies: each of them looks at the top 2 cards of their library and keeps the best on top.',
    preview: 'Each ally scries 2',
  },
  {
    index: 4,
    name: 'Hinder',
    ectoplasm: 1,
    plain: 'Pin down an enemy creature: it taps and stays tapped through its next untap.',
    preview: 'Tap 1 enemy creature · stays tapped',
  },
  {
    index: 5,
    name: 'Howl',
    ectoplasm: 3,
    plain: 'A great howl: tap up to two enemy creatures and every enemy loses 2 life.',
    preview: 'Tap 2 enemy creatures · enemies −2 life',
  },
  {
    index: 6,
    name: 'Rise Again',
    ectoplasm: 5,
    plain: 'Return to the fight with 10 life and a fresh hand of 5 cards.',
    preview: 'Back to life · 10 life · draw 5',
    sorcery: true,
  },
]

/** The few lines the moment of falling shows. */
export const SPIRIT_LINES: readonly string[] = [
  'You fell, but the fight goes on: the party loses only when every hero has fallen.',
  'Enemies can no longer attack or target you. Each of your turns you gain 1 ectoplasm.',
  'Spend it on haunt actions to help your allies, or save 5 to Rise Again with 10 life.',
]

/** The haunt action with this ability index of the Restless Spirit. */
export function hauntAction(index: number): HauntAction | undefined {
  return HAUNT_ACTIONS.find((action) => action.index === index)
}

/** The Restless Spirit emblem in `player`'s command zone. */
export function spiritEmblem(
  player: PlayerView,
  objects: Readonly<Record<ObjectId, ObjectView>>,
): ObjectView | undefined {
  return (player.command.cards ?? [])
    .map((id) => objects[id])
    .find((object) => object?.kind === 'Emblem' && object.name === SPIRIT_EMBLEM)
}

/** The ectoplasm counters a Spirit holds. */
export function ectoplasmOf(player: PlayerView | undefined): number {
  const found = player?.counters?.find(
    ([kind]) => typeof kind === 'object' && 'Other' in kind && kind.Other === 'ectoplasm',
  )
  return found?.[1] ?? 0
}

/** A haunt action as the panel shows it now. */
export interface HauntOption extends HauntAction {
  /** The ability to activate, when it can be taken now. */
  ready?: AbilityRef
  /** Why it cannot be taken now, in words. */
  why?: string
}

/** Each haunt action with whether it can be taken now and, if not, why. */
export function hauntOptions(
  player: PlayerView | undefined,
  emblem: ObjectView | undefined,
  options: PriorityOptions | null,
): HauntOption[] {
  const ectoplasm = ectoplasmOf(player)
  return HAUNT_ACTIONS.map((action) => {
    const ready =
      emblem === undefined
        ? undefined
        : (options?.abilities ?? []).find((ref) => ref.object === emblem.id && ref.ability === action.index)
    if (ready !== undefined) return { ...action, ready }
    const why =
      ectoplasm < action.ectoplasm
        ? `Needs ${action.ectoplasm} ectoplasm`
        : options === null
          ? 'Wait for your priority'
          : action.sorcery === true
            ? 'Only in your main phase'
            : 'Not now'
    return { ...action, why }
  })
}

/** The log's words for a haunt action: "Bo haunts: Wail". `undefined` for any other ability. */
export function hauntWords(sourceName: string, ability: number): string | undefined {
  if (sourceName !== SPIRIT_EMBLEM) return undefined
  const action = hauntAction(ability)
  return action === undefined ? undefined : action.name
}

/** Whether the viewer's seat became a Spirit between the two seat lists. */
export function fellBetween(
  before: readonly PlayerView[],
  after: readonly PlayerView[],
  seat: number | null,
): boolean {
  if (seat === null) return false
  const was = before.find((player) => player.seat === seat)
  const now = after.find((player) => player.seat === seat)
  return was?.spirit !== true && now?.spirit === true
}

/** What the Spirit mark's tooltip says. */
export const SPIRIT_TIP =
  "Restless Spirit: fell in this fight. Enemies can't attack or target them; they spend ectoplasm on haunt actions and can rise again."
