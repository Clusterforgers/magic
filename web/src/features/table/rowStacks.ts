/**
 * The stacks of a seat's battlefield: which permanents share a box (`group.ts`) read from the store as plain ids, so a
 * tap or a point of damage redraws no row, and how many boxes each row needs once stacks are open (`layoutSeat`).
 */
import type { PlayState } from '@/play/store'
import type { ObjectId, PlayerId, StateView } from '@/protocol/generated'
import { combatRoleOf } from './cardModel'
import type { RowKind } from './cardModel'
import type { FitBox } from './fit'
import { GROUP_OTHERS_BELOW, groupPermanents } from './group'
import type { Groupable, Stack } from './group'
import { fitLanes } from './laneFit'
import type { LaneCounts, LaneFit } from './laneFit'
import { splitBattlefield } from './selectors'
import type { BattlefieldRows } from './selectors'

/** A stack as the battlefield keeps it: the ids of its members in battlefield order, under the key `group.ts` gave it. */
export interface StackIds {
  key: string
  ids: readonly ObjectId[]
}

/** The stacks of one seat by row. */
export type RowStacks = Record<RowKind, readonly StackIds[]>

/**
 * A seat's stacks twice: `plain` merges lands and tokens, which merge at once, and `merged` every identical card
 * too, for a zone whose cards would fall under `GROUP_OTHERS_BELOW`.
 */
export interface SeatStacks {
  plain: RowStacks
  merged: RowStacks
}

const NO_STACKS: RowStacks = { creatures: [], other: [], lands: [] }

/** A seat with nothing on the battlefield. */
export const NO_SEAT_STACKS: SeatStacks = { plain: NO_STACKS, merged: NO_STACKS }

type Objects = PlayState['objectsById']

const idsOf = (stacks: readonly Stack[]): StackIds[] =>
  stacks.map((stack) => ({ key: stack.key, ids: stack.members.map((member) => member.id) }))

function stacksOfRows(rows: BattlefieldRows, objects: Objects, view: StateView): SeatStacks {
  const permanents = (row: RowKind): Groupable[] => {
    const list: Groupable[] = []
    for (const id of rows[row]) {
      const object = objects[id]
      if (object === undefined) continue
      const role = combatRoleOf(view.combat, id)
      list.push(role === null ? object : { ...object, role })
    }
    return list
  }
  const creatures = permanents('creatures')
  const other = permanents('other')
  // Lands merge whatever is asked, so both ways of grouping them are one list.
  const lands = idsOf(groupPermanents(permanents('lands')))
  return {
    plain: { creatures: idsOf(groupPermanents(creatures)), other: idsOf(groupPermanents(other)), lands },
    merged: {
      creatures: idsOf(groupPermanents(creatures, { groupOthers: true })),
      other: idsOf(groupPermanents(other, { groupOthers: true })),
      lands,
    },
  }
}

// The stacks of a view are worked out once however many rows ask, as the split of the battlefield is.
const cache = new WeakMap<StateView, Map<PlayerId, SeatStacks>>()

const alone = (ids: readonly ObjectId[]): StackIds[] => ids.map((id) => ({ key: `#${id}`, ids: [id] }))

/**
 * The stacks of one seat's battlefield, in the order of each row; with `grouped` false every permanent is a stack of
 * its own. Plain data, so wrap in `useStable`: a tap that moves no permanent from one stack to another changes no ids.
 */
export function battlefieldStacks(state: PlayState, seat: PlayerId, grouped = true): SeatStacks {
  const view = state.view
  if (view === null) return NO_SEAT_STACKS
  if (!grouped) {
    const rows = splitBattlefield(view, state.objectsById).get(seat)
    if (rows === undefined) return NO_SEAT_STACKS
    const single = { creatures: alone(rows.creatures), other: alone(rows.other), lands: alone(rows.lands) }
    return { plain: single, merged: single }
  }
  let seats = cache.get(view)
  if (seats === undefined) {
    seats = new Map()
    cache.set(view, seats)
  }
  let stacks = seats.get(seat)
  if (stacks === undefined) {
    const rows = splitBattlefield(view, state.objectsById).get(seat)
    stacks = rows === undefined ? NO_SEAT_STACKS : stacksOfRows(rows, state.objectsById, view)
    seats.set(seat, stacks)
  }
  return stacks
}

/** Every permanent of the rows, row after row. */
export function idsOfRows(rows: RowStacks): ObjectId[] {
  return [...rows.creatures, ...rows.other, ...rows.lands].flatMap((stack) => stack.ids)
}

/** The key of the stack that holds `id`, if any. */
export function keyOf(rows: RowStacks, id: ObjectId): string | undefined {
  for (const row of [rows.creatures, rows.other, rows.lands]) {
    const stack = row.find((candidate) => candidate.ids.includes(id))
    if (stack !== undefined) return stack.key
  }
  return undefined
}

/** A stack as a row draws it. */
export interface RowEntry {
  /** The members, the first being the one drawn while the stack is closed. */
  ids: readonly ObjectId[]
  /** The members are drawn one by one: the player opened the stack, or a prompt needs each of them. A lone permanent is never open. */
  open: boolean
  /** A prompt needs each member, so the player cannot close the stack. */
  held: boolean
}

function entryOf(stack: StackIds, opened: ReadonlySet<string>, marked: ReadonlySet<ObjectId>): RowEntry {
  if (stack.ids.length < 2) return { ids: stack.ids, open: false, held: false }
  const held = stack.ids.some((id) => marked.has(id))
  return { ids: stack.ids, open: held || opened.has(stack.key), held }
}

/** The boxes a row of entries takes: one for a closed stack or a lone permanent, one per member for an open stack. */
export function boxesOf(entries: readonly RowEntry[]): number {
  return entries.reduce((sum, entry) => sum + (entry.open ? entry.ids.length : 1), 0)
}

/** The ids a row draws, in order: the first member of a closed stack, every member of an open one. */
export function visibleIds(entries: readonly RowEntry[]): ObjectId[] {
  return entries.flatMap((entry) => (entry.open || entry.ids.length < 2 ? entry.ids : entry.ids.slice(0, 1)))
}

/** Whether two rows draw the same stacks in the same state; keys are no part of it, ids and openness are. */
export function sameEntries(a: readonly RowEntry[], b: readonly RowEntry[]): boolean {
  return (
    a === b ||
    (a.length === b.length &&
      a.every((entry, index) => {
        const other = b[index]
        return (
          other !== undefined &&
          entry.open === other.open &&
          entry.held === other.held &&
          entry.ids.length === other.ids.length &&
          entry.ids.every((id, at) => id === other.ids[at])
        )
      }))
  )
}

/** What `layoutSeat` needs. */
export interface SeatLayoutInput {
  /** The room inside the zone, or null where nothing is laid out (a test, a hidden zone). */
  box: FitBox | null
  stacks: SeatStacks
  /** `near` is the viewer's own board, `far` an opponent's: their bands mirror each other top to bottom. */
  facing: 'near' | 'far'
  /** The keys of the stacks the player opened. */
  opened: ReadonlySet<string>
  /** The permanents a prompt marks as legal or selected: a stack holding one is held open. */
  marked: ReadonlySet<ObjectId>
}

/** The rows of a seat to draw and the fit of its zone. */
export interface SeatLayout {
  rows: Record<RowKind, readonly RowEntry[]>
  /** The stacks the rows come from; their keys name the entries. */
  stacks: RowStacks
  /** True when identical cards that are no lands or tokens were merged too. */
  merged: boolean
  /** The bands and lanes of the zone and their card size; null where there is no box. */
  fit: LaneFit | null
}

function entriesOfRows(
  rows: RowStacks,
  opened: ReadonlySet<string>,
  marked: ReadonlySet<ObjectId>,
): Record<RowKind, RowEntry[]> {
  const entries = (row: readonly StackIds[]): RowEntry[] => row.map((stack) => entryOf(stack, opened, marked))
  return { creatures: entries(rows.creatures), other: entries(rows.other), lands: entries(rows.lands) }
}

/** The boxes of each lane: one for each stack and one for each member of an open one. */
export function laneCounts(rows: Record<RowKind, readonly RowEntry[]>): LaneCounts {
  return { creatures: boxesOf(rows.creatures), other: boxesOf(rows.other), lands: boxesOf(rows.lands) }
}

/**
 * Draws a seat's battlefield: fits the zone's lanes with lands and tokens merged, and when the cards would fall under
 * `GROUP_OTHERS_BELOW` merges identical cards too and fits again. Every permanent takes one box, tapped or not, and
 * the boxes of the fit are exactly the ones the lanes draw.
 */
export function layoutSeat({ box, stacks, facing, opened, marked }: SeatLayoutInput): SeatLayout {
  const plain = entriesOfRows(stacks.plain, opened, marked)
  if (box === null) return { rows: plain, stacks: stacks.plain, merged: false, fit: null }
  const plainFit = fitLanes(box, laneCounts(plain), facing)
  if (plainFit.cardWidth >= GROUP_OTHERS_BELOW) {
    return { rows: plain, stacks: stacks.plain, merged: false, fit: plainFit }
  }
  const merged = entriesOfRows(stacks.merged, opened, marked)
  return { rows: merged, stacks: stacks.merged, merged: true, fit: fitLanes(box, laneCounts(merged), facing) }
}
