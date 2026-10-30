/** The dev table's `?panel=`: opens a row of the Commands panel, or fills its Card group, once the table has loaded, to look at it. */
import { useEffect } from 'react'
import type { ObjectId } from '@/protocol/generated'
import { rightClickFirst } from '../devMenu'
import type { CommandId } from './commands'

/** How many times, 100 ms apart, the table gets to draw the panel before the request gives up. */
const TRIES = 100

/** What `?panel=` names: a row of the panel, `top` for the look-at-the-top row with three cards looked at, or `card` for the Card group with a card picked. */
export type DevPanel = CommandId | 'top' | 'card'

const COMMANDS: readonly CommandId[] = [
  'life',
  'poison',
  'energy',
  'experience',
  'draw',
  'look',
  'mill',
  'exileTop',
  'bottomTop',
  'shuffle',
  'revealTop',
  'token',
  'mana',
]

/** The row `?panel=` opens: `1` the life row, a command by its id, `top` or `card`; `undefined` for anything else. */
export function panelOf(value: unknown): DevPanel | undefined {
  if (value === 'top') return 'top'
  if (value === 1 || value === true || value === '1') return 'life'
  if (value === 'card') return 'card'
  return COMMANDS.find((id) => id === value)
}

/** Opens the side area onto the panel when it is shut, from the rail's first icon or the top bar's sheet button; true once the panel is drawn. */
export function showPanel(root: ParentNode = document): boolean {
  if (root.querySelector('[data-slot="commands-panel"]') !== null) return true
  root
    .querySelector<HTMLElement>(
      'aside[data-side="rail"] [role="tab"], button[aria-label^="Commands, log, chat"]',
    )
    ?.click()
  return false
}

/** Presses the row of `command` in the panel when it is drawn and closed; true once it is open. */
export function openRow(command: CommandId, root: ParentNode = document): boolean {
  const row = root.querySelector<HTMLElement>(`[data-slot="commands-panel"] [data-command="${command}"]`)
  if (row === null) return false
  if (row.dataset.open === undefined) row.querySelector<HTMLButtonElement>('button[aria-expanded]')?.click()
  return true
}

/**
 * Picks one of `ids` the way a player does (a right click, then "In the panel") and opens the counters row of the
 * Card group; one step per call, true once the row is open.
 */
export function openCard(ids: readonly ObjectId[], root: ParentNode = document): boolean {
  const group = root.querySelector<HTMLElement>('[data-slot="commands-panel"] [data-group="card"]')
  if (group === null) {
    showPanel(root)
    return false
  }
  if (group.querySelector('[data-slot="card-picked"]') !== null) {
    const row = group.querySelector<HTMLElement>('[data-command="card:counters"]')
    if (row === null) return false
    if (row.dataset.open === undefined) row.querySelector<HTMLButtonElement>('button[aria-expanded]')?.click()
    return true
  }
  const entry = root.querySelector<HTMLElement>('[data-slot="manual-menu"] [data-item="in-panel"]')
  if (entry === null) rightClickFirst(ids, root)
  else entry.click()
  return false
}

const NO_CARDS: readonly ObjectId[] = []

/** Opens the row of `command`, or fills the Card group from one of `cards`, once the panel is drawn; nothing for `undefined`. */
export function useOpenDevPanel(command: DevPanel | undefined, cards: readonly ObjectId[] = NO_CARDS): void {
  useEffect(() => {
    if (command === undefined) return
    let timer = 0
    let left = TRIES
    // A timer and not a frame: a page that is not shown draws no frames.
    const tick = (): void => {
      const done =
        command === 'card' ? openCard(cards) : showPanel() && openRow(command === 'top' ? 'look' : command)
      if (done || (left -= 1) <= 0) return
      timer = window.setTimeout(tick, 100)
    }
    timer = window.setTimeout(tick, 100)
    return () => window.clearTimeout(timer)
  }, [command, cards])
}
