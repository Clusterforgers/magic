/** The viewer's "Card images" choice (On or Text only), kept in `localStorage` and shared by every page. */
import { useSyncExternalStore } from 'react'

/** On draws pictures; Text only draws the text face everywhere. */
export type ImageChoice = 'on' | 'text'

/** The `localStorage` key of the choice. */
export const IMAGE_CHOICE_KEY = 'mtg.cardImages'

const listeners = new Set<() => void>()
let current: ImageChoice | null = null

// Storage can throw on access (blocked site data) or be missing; the choice then lives for the page only.
function readStored(): ImageChoice {
  try {
    return window.localStorage.getItem(IMAGE_CHOICE_KEY) === 'text' ? 'text' : 'on'
  } catch {
    return 'on'
  }
}

/** The choice now: the stored one, On by default. */
export function imageChoice(): ImageChoice {
  current ??= readStored()
  return current
}

/** Changes the choice, stores it and redraws every subscriber. */
export function setImageChoice(choice: ImageChoice): void {
  current = choice
  try {
    window.localStorage.setItem(IMAGE_CHOICE_KEY, choice)
  } catch {
    // Kept for this page only.
  }
  for (const listener of listeners) listener()
}

/** Forgets the cached choice, so the next read goes to storage again (tests). */
export function resetImageChoice(): void {
  current = null
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

/** The viewer's choice, redrawn when it changes. */
export function useImageChoice(): ImageChoice {
  return useSyncExternalStore(subscribe, imageChoice, imageChoice)
}
