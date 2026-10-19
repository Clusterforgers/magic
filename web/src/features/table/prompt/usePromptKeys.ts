import { useEffect, useRef } from 'react'
import type { RefObject } from 'react'

/** What takes letters and digits for itself: fields, editable text, widgets that read typing, menus and dialogs. */
const TAKES_TEXT = [
  'input',
  'textarea',
  'select',
  '[contenteditable]:not([contenteditable="false"])',
  '[role="textbox"]',
  '[role="combobox"]',
  '[role="searchbox"]',
  '[role="spinbutton"]',
  '[role="listbox"]',
  '[role="menu"]',
  '[role="dialog"]',
  '[role="alertdialog"]',
].join(',')

/** Whether a letter or digit pressed with `target` focused is typed text, or belongs to a menu or dialog, not a hotkey. */
export function isTyping(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest(TAKES_TEXT) !== null
}

/** The hotkey a press is: its lower-case letter or digit, once and with no Ctrl, Alt or Meta; null for anything else. */
export function hotkeyOf(event: KeyboardEvent): string | null {
  if (event.repeat || event.ctrlKey || event.altKey || event.metaKey) return null
  return event.key.length === 1 ? event.key.toLowerCase() : null
}

/**
 * Letter and digit hotkeys while `enabled`: a plain press of a key in `keys` calls its function. A focused button
 * does not take letters, so the hotkeys work with focus on a button, but not while a field, menu or dialog has the
 * keyboard. The default action is suppressed only when a hotkey fires.
 */
export function usePromptKeys(keys: Readonly<Record<string, () => void>>, enabled = true): void {
  const latest = useRef(keys)
  useEffect(() => {
    latest.current = keys
  })
  useEffect(() => {
    if (!enabled) return
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.defaultPrevented || isTyping(event.target)) return
      const key = hotkeyOf(event)
      if (key === null || !Object.hasOwn(latest.current, key)) return
      event.preventDefault()
      latest.current[key]?.()
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [enabled])
}

/**
 * Puts focus on `target` when the prompt appears, unless the player is typing somewhere (a message to the table must
 * not lose its cursor to a question), and without scrolling the page.
 */
export function useDefaultFocus(target: RefObject<HTMLElement | null>): void {
  useEffect(() => {
    if (isTyping(document.activeElement)) return
    target.current?.focus({ preventScroll: true })
  }, [target])
}
