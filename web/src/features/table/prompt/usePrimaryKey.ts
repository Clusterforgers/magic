import { useEffect, useRef } from 'react'

/** Anything that takes Space or Enter for itself: controls, links, fields, menus and the like. */
const TAKES_KEYS = [
  'button',
  'a[href]',
  'input',
  'textarea',
  'select',
  'summary',
  '[contenteditable]:not([contenteditable="false"])',
  '[role="button"]',
  '[role="link"]',
  '[role="menuitem"]',
  '[role="menu"]',
  '[role="checkbox"]',
  '[role="switch"]',
  '[role="tab"]',
  '[role="option"]',
  '[role="dialog"]',
  '[role="alertdialog"]',
].join(',')

/**
 * Whether a key pressed with `target` focused is the table's to use: nothing is focused, or what is focused takes no
 * keys of its own. A card, a button, the chat box or an open dialog keeps its Space and Enter.
 */
export function isFocusFree(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return true
  if (target === document.body || target === document.documentElement) return true
  return target.closest(TAKES_KEYS) === null
}

/** The two keys of the primary controls. */
export type PrimaryKey = 'space' | 'enter'

/** What each primary key does. */
export type PrimaryKeys = Record<PrimaryKey, () => void>

/** Which primary key a press is, or null: a plain Space or Enter, once (a key held down repeats), no modifier. */
export function primaryKeyOf(event: KeyboardEvent): PrimaryKey | null {
  if (event.repeat || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return null
  if (event.key === ' ') return 'space'
  if (event.key === 'Enter') return 'enter'
  return null
}

/**
 * The keys of the primary controls while `enabled`: a plain Space or Enter that no control or field has focus for
 * calls its function in `keys`. The default action (scrolling for Space) is suppressed only when it does.
 */
export function usePrimaryKey(keys: PrimaryKeys, enabled: boolean): void {
  const latest = useRef(keys)
  useEffect(() => {
    latest.current = keys
  })
  useEffect(() => {
    if (!enabled) return
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.defaultPrevented || !isFocusFree(event.target)) return
      const key = primaryKeyOf(event)
      if (key === null) return
      event.preventDefault()
      latest.current[key]()
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [enabled])
}
