/**
 * The keys of a decision in its home (`docs/ui-design.md`, Keys): A moves the focus to the answers, Enter with nothing
 * focused presses the primary answer, and during a board choice Tab runs from the answers through the detail to the lit
 * objects of the table (opponents, your board, the hand) and back, skipping what cannot be chosen.
 */
import { useEffect } from 'react'
import { isFocusFree } from './usePrimaryKey'
import { usePromptKeys } from './usePromptKeys'

/** What a Tab can land on. */
const FOCUSABLE =
  ':is(button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex="0"]):not([aria-disabled="true"])'

/** The home of the decision on show. */
const home = (): HTMLElement | null => document.querySelector<HTMLElement>('[data-coach="ask"]')

/** The home's answers that can be pressed, the primary first. */
export function answerButtons(): HTMLElement[] {
  return [
    ...(home()?.querySelectorAll<HTMLElement>('[data-slot="prompt-actions"] > button:not(:disabled)') ?? []),
  ]
}

/** Moves the focus to the home's primary answer, as the A key does; false when there is none. */
export function focusAnswers(): boolean {
  const [first] = answerButtons()
  first?.focus()
  return first !== undefined
}

/** The objects a board choice lights, in reading order: the opponents, your board, then the hand. */
export function litObjects(): HTMLElement[] {
  return ['opponents', 'own', 'hand'].flatMap((area) => [
    ...document.querySelectorAll<HTMLElement>(
      `[data-area="${area}"] button[data-object-id]:is([data-highlight="legal"], [data-highlight="selected"])`,
    ),
  ])
}

/** The ring a Tab follows during a board choice: the home in the page's order, then the lit objects. */
export function tabRing(): HTMLElement[] {
  return [...(home()?.querySelectorAll<HTMLElement>(FOCUSABLE) ?? []), ...litObjects()]
}

/** The element after (or before, `back`) `current` in `ring`, round to the start; null when `current` is not in it. */
export function nextInRing<T>(ring: readonly T[], current: T, back: boolean): T | null {
  const at = ring.indexOf(current)
  if (at < 0 || ring.length === 0) return null
  return ring[(at + (back ? ring.length - 1 : 1)) % ring.length] ?? null
}

/** Whether the table waits for a press on it (`isBoardChoice`). */
const boardChoice = (): boolean => document.querySelector('[data-board-choice]') !== null

/** The keys of the decision in its home while `enabled`; `enter`: Enter presses the primary answer (a card's question; the game's have keys of their own). */
export function useHomeKeys(enabled: boolean, enter: boolean): void {
  usePromptKeys({ a: () => void focusAnswers() }, enabled)
  useEffect(() => {
    if (!enabled) return undefined
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey) return
      if (event.key === 'Enter' && enter && !event.repeat && isFocusFree(event.target)) {
        const [primary] = answerButtons()
        if (primary === undefined) return
        event.preventDefault()
        primary.click()
        return
      }
      if (event.key !== 'Tab' || !boardChoice() || !(document.activeElement instanceof HTMLElement)) return
      const ring = tabRing()
      const from = document.activeElement
      if (!ring.includes(from)) return
      // Inside the home the page's own order holds (detail, then answers); the ring takes over only at its edges and
      // among the lit objects, so the keyboard crosses from the home to them and back and skips what is not lit.
      const own = ring.filter((element) => home()?.contains(element) === true)
      const edge = event.shiftKey ? own[0] : own.at(-1)
      if (own.includes(from) && from !== edge) return
      event.preventDefault()
      // The next element that takes the focus: one in a hidden part of the home does not.
      let at: HTMLElement | null = from
      for (let step = 0; step < ring.length; step += 1) {
        at = at === null ? null : nextInRing(ring, at, event.shiftKey)
        if (at === null || at === from) return
        at.focus()
        if (document.activeElement === at) return
      }
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [enabled, enter])
}
