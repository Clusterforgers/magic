/**
 * What a press on a card that cannot be played says (docs/ui-design.md, Playing is legible): one line at the card for a
 * moment, and the object whose rule blocks it lit on the table and in the boss's row. One tell at a time; a new press
 * replaces it.
 */
import { create } from 'zustand'
import type { PlayState } from '@/play/store'
import type { ObjectId } from '@/protocol/generated'
import { useSourceLit } from './modifiers'
import type { Blocker } from './prompt/marks'
import { waitingOn } from './turnText'

/** How long the line stays and the blocker is lit, in milliseconds. */
export const TELL_MS = 3200

/** Where the line points: the pressed card's box on screen. */
export interface TellAnchor {
  left: number
  top: number
  width: number
  height: number
}

/** One tell: the card, its line (or the wait for another seat's decision), where it stands and what blocks it. */
export interface Tell {
  card: ObjectId
  /** The reason in one line; undefined while the game waits on another seat, which the tell names itself. */
  line?: string
  /** The pressed card's element, measured again once a lift has settled. */
  element: HTMLElement
  anchor: TellAnchor
  blocker: Blocker
  /** Tells two presses of one card apart, so the second restarts the line. */
  token: number
}

interface TellState {
  tell: Tell | null
  say: (tell: Omit<Tell, 'token'>) => void
  hush: (token?: number) => void
}

let tokens = 0
let timer: ReturnType<typeof setTimeout> | undefined

/** The tell on screen, if any. */
export const useBlockedTell = create<TellState>()((set, get) => ({
  tell: null,
  say(next) {
    tokens += 1
    const token = tokens
    set({ tell: { ...next, token } })
    const source = next.blocker.source
    useSourceLit.getState().light(source === undefined ? [] : [source])
    if (timer !== undefined) clearTimeout(timer)
    timer = setTimeout(() => get().hush(token), TELL_MS)
  },
  hush(token) {
    const { tell } = get()
    if (tell === null || (token !== undefined && tell.token !== token)) return
    if (tell.blocker.source !== undefined) useSourceLit.getState().light([])
    set({ tell: null })
  },
}))

/** The box of the card `element` on screen: a lifted hand card draws its face above its box, so the higher of the two. */
export function anchorOf(element: HTMLElement): TellAnchor {
  const box = element.getBoundingClientRect()
  const face = element.querySelector('[data-slot="card-face"]')?.getBoundingClientRect()
  const top = Math.min(box.top, face?.top ?? box.top)
  const bottom = Math.max(box.bottom, face?.bottom ?? box.bottom)
  return { left: box.left, top, width: box.width, height: bottom - top }
}

/** Says why the card `card`, pressed at `element`, cannot be played: `line`, or the wait when it is undefined. */
export function tellBlocked(
  card: ObjectId,
  element: HTMLElement,
  line: string | undefined,
  blocker: Blocker,
): void {
  useBlockedTell.getState().say({ card, line, element, anchor: anchorOf(element), blocker })
}

/**
 * Whether a press on `card` should say the game waits on another seat: a card of the viewer's own hand while the viewer
 * has no decision and another seat has one (public: the table shows whom it waits for).
 */
export function waitsForOthers(state: PlayState, card: ObjectId): boolean {
  if (state.pending !== null || state.seat === null) return false
  const zone = state.objectsById[card]?.zone
  const inHand = zone !== undefined && typeof zone === 'object' && 'Hand' in zone && zone.Hand === state.seat
  return inHand && waitingOn(state).kind === 'others'
}

/** The line for the wait on another seat: "Waiting for Bo to act or pass: you can play once you have priority." */
export function waitingLine(waiting: string): string {
  return `${waiting}: you can play once you have priority.`
}

/** Whether the tell on screen names the rule of `name` (a boss's modifier chip, a relic). */
export function useIsBlockerNamed(name: string): boolean {
  return useBlockedTell((state) => state.tell !== null && state.tell.blocker.name === name)
}
