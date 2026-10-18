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

/** Whether a key press is a plain Space or Enter, once (a key held down repeats). */
export function isPrimaryKey(event: KeyboardEvent): boolean {
  if (event.repeat || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return false
  return event.key === ' ' || event.key === 'Enter'
}

/**
 * Presses the primary button from the keyboard: while `enabled`, a plain Space or Enter that no control or field has
 * focus for calls `press`. The default action (scrolling for Space) is suppressed only when it does.
 */
export function usePrimaryKey(press: () => void, enabled: boolean): void {
  const latest = useRef(press)
  useEffect(() => {
    latest.current = press
  })
  useEffect(() => {
    if (!enabled) return
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.defaultPrevented || !isPrimaryKey(event) || !isFocusFree(event.target)) return
      event.preventDefault()
      latest.current()
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [enabled])
}
