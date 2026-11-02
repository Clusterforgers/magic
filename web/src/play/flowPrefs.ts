/**
 * The viewer's pace preferences, shared by every game: the pinned stops a new game starts with, and whether the seat
 * ends its own turns by hand. A signed-in account keeps them on the server (`attachAccount`); a guest keeps them in
 * `localStorage`. A session applies them at `Welcome` (`welcomeRequests`).
 */
import { useSyncExternalStore } from 'react'
import type { ClientMessage, Stop } from '@/protocol/generated'
import { canHoldStop } from './stops'

/** The `localStorage` key of the default stops. */
export const DEFAULT_STOPS_KEY = 'mtg.defaultStops'
/** The `localStorage` key of the end-turn-by-hand choice. */
export const END_TURN_KEY = 'mtg.endTurnByHand'
/** The `localStorage` key of the games the default stops were given to. */
export const APPLIED_KEY = 'mtg.defaultStopsApplied'
/** How many games `APPLIED_KEY` remembers. */
export const APPLIED_LIMIT = 50

/** The preferences at one moment. */
export interface FlowPrefs {
  /** The pinned stops a new game starts with; empty when none were saved. */
  defaultStops: readonly Stop[]
  /** Whether the seat ends its own turns by hand. */
  endTurnByHand: boolean
}

const listeners = new Set<() => void>()
let current: FlowPrefs | null = null
/** Where the preferences are saved while an account is signed in; none keeps them in `localStorage`. */
let account: ((prefs: FlowPrefs) => void) | null = null

// Storage can throw on access (blocked site data) or be missing; the preferences then live for the page only.
function readItem(key: string): string | null {
  try {
    return window.localStorage.getItem(key)
  } catch {
    return null
  }
}

function writeItem(key: string, value: string | null): void {
  try {
    if (value === null) window.localStorage.removeItem(key)
    else window.localStorage.setItem(key, value)
  } catch {
    // Kept for this page only.
  }
}

function parseJson(text: string | null): unknown {
  if (text === null) return null
  try {
    return JSON.parse(text) as unknown
  } catch {
    return null
  }
}

/** The stored stops that are still well formed: a side, a step that can hold a stop, and lasting. */
function readStops(): Stop[] {
  const raw = parseJson(readItem(DEFAULT_STOPS_KEY))
  if (!Array.isArray(raw)) return []
  return raw.flatMap((item: unknown) => {
    if (typeof item !== 'object' || item === null) return []
    const { side, step } = item as Record<string, unknown>
    if (
      (side !== 'Own' && side !== 'Opponent') ||
      typeof step !== 'string' ||
      !canHoldStop(step as Stop['step'])
    )
      return []
    return [{ side, step: step as Stop['step'], keep: true }]
  })
}

/** The preferences now: the stored ones, none by default. */
export function flowPrefs(): FlowPrefs {
  current ??= { defaultStops: readStops(), endTurnByHand: readItem(END_TURN_KEY) === 'on' }
  return current
}

function change(next: FlowPrefs): void {
  current = next
  for (const listener of listeners) listener()
}

/** The preferences this browser's storage holds. */
function readLocal(): FlowPrefs {
  return { defaultStops: readStops(), endTurnByHand: readItem(END_TURN_KEY) === 'on' }
}

/** Keeps `next` where the viewer's preferences live now and shows it. */
function store(next: FlowPrefs): void {
  if (account === null) {
    writeItem(DEFAULT_STOPS_KEY, JSON.stringify(next.defaultStops))
    writeItem(END_TURN_KEY, next.endTurnByHand ? 'on' : null)
  } else account(next)
  change(next)
}

/** Whether `prefs` are what a new account has. */
function isDefault(prefs: FlowPrefs): boolean {
  return prefs.defaultStops.length === 0 && !prefs.endTurnByHand
}

/**
 * Moves the preferences to the signed-in account: `server` replaces them, and `save` gets every later change. An
 * account that holds none yet takes the ones this browser kept.
 */
export function attachAccount(server: FlowPrefs, save: (prefs: FlowPrefs) => void): void {
  const local = readLocal()
  account = save
  if (isDefault(server) && !isDefault(local)) {
    save(local)
    change(local)
  } else change(server)
}

/** Returns the preferences to this browser's storage, as for a guest. */
export function detachAccount(): void {
  account = null
  change(readLocal())
}

/** Saves the lasting stops of `stops` as the default of every new game; one-turn stops are left out. */
export function saveDefaultStops(stops: readonly Stop[]): void {
  const pinned = stops.filter((stop) => stop.keep).map(({ side, step }) => ({ side, step, keep: true }))
  store({ ...flowPrefs(), defaultStops: pinned })
}

/** Forgets the default stops: a new game starts with none. */
export function resetDefaultStops(): void {
  store({ ...flowPrefs(), defaultStops: [] })
}

/** Stores whether the seat ends its own turns by hand. */
export function saveEndTurnByHand(on: boolean): void {
  store({ ...flowPrefs(), endTurnByHand: on })
}

/** Forgets the cached preferences, so the next read goes to storage again (tests). */
export function resetFlowPrefs(): void {
  current = null
  account = null
}

/** Whether the default stops were already given to `game`; the first call for a game also notes that they now are. */
function firstWelcome(game: string): boolean {
  const raw = parseJson(readItem(APPLIED_KEY))
  const games = Array.isArray(raw) ? raw.filter((id): id is string => typeof id === 'string') : []
  if (games.includes(game)) return false
  writeItem(APPLIED_KEY, JSON.stringify([...games, game].slice(-APPLIED_LIMIT)))
  return true
}

/** What a `Welcome` states that the preferences care about. */
export interface WelcomeFacts {
  game: string
  stops: readonly Stop[]
  endTurnByHand: boolean
}

/**
 * The requests that bring the server in line with the preferences at a `Welcome`: the default stops once per game,
 * the first time this browser joins it and only while the seat holds none, and the end-turn-by-hand choice whenever
 * the server holds the other one.
 */
export function welcomeRequests(welcome: WelcomeFacts, prefs: FlowPrefs = flowPrefs()): ClientMessage[] {
  const requests: ClientMessage[] = []
  const first = firstWelcome(welcome.game)
  if (first && welcome.stops.length === 0) {
    for (const { side, step } of prefs.defaultStops)
      requests.push({ type: 'SetStop', side, step, on: true, keep: true })
  }
  if (prefs.endTurnByHand !== welcome.endTurnByHand)
    requests.push({ type: 'EndTurnByHand', on: prefs.endTurnByHand })
  return requests
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

/** The preferences, redrawn when they change. */
export function useFlowPrefs(): FlowPrefs {
  return useSyncExternalStore(subscribe, flowPrefs, flowPrefs)
}
