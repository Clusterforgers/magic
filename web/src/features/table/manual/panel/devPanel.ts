/** The dev table's `?panel=`: opens a row of the Commands panel once the table has loaded, to look at it. */
import { useEffect } from 'react'
import type { CommandId } from './commands'

/** How many times, 100 ms apart, the table gets to draw the panel before the request gives up. */
const TRIES = 100

const COMMANDS: readonly CommandId[] = [
  'life',
  'poison',
  'energy',
  'experience',
  'draw',
  'mill',
  'exileTop',
  'bottomTop',
  'shuffle',
  'token',
  'mana',
]

/** The row `?panel=` opens: `1` the life row, or a command by its id; `undefined` for anything else. */
export function panelOf(value: unknown): CommandId | undefined {
  if (value === 1 || value === true || value === '1') return 'life'
  return COMMANDS.find((id) => id === value)
}

/** Presses the row of `command` in the panel when it is drawn and closed; true once it is open. */
export function openRow(command: CommandId, root: ParentNode = document): boolean {
  const row = root.querySelector<HTMLElement>(`[data-slot="commands-panel"] [data-command="${command}"]`)
  if (row === null) return false
  if (row.dataset.open === undefined) row.querySelector<HTMLButtonElement>('button[aria-expanded]')?.click()
  return true
}

/** Opens the row of `command` once the panel is drawn; nothing for `undefined`. */
export function useOpenDevPanel(command: CommandId | undefined): void {
  useEffect(() => {
    if (command === undefined) return
    let timer = 0
    let left = TRIES
    // A timer and not a frame: a page that is not shown draws no frames.
    const tick = (): void => {
      if (openRow(command) || (left -= 1) <= 0) return
      timer = window.setTimeout(tick, 100)
    }
    timer = window.setTimeout(tick, 100)
    return () => window.clearTimeout(timer)
  }, [command])
}
