import { Component } from 'react'
import type { ReactNode, RefObject } from 'react'
import type { LobbyGame } from '@/protocol/generated'
import { nextJoin } from './format'

interface Props {
  /** The rows on screen, in order. */
  rows: readonly LobbyGame[]
  /** The element that holds the rows and their Join buttons. */
  within: RefObject<HTMLElement | null>
  /** Takes focus when no row is left to give it to. */
  fallback: RefObject<HTMLElement | null>
  children: ReactNode
}

/** The row that had focus and what the list was when it did. */
interface Lost {
  game: string
  before: readonly LobbyGame[]
}

/**
 * Keeps a keyboard user's place when an update removes the row they stand on: focus goes to the next Join instead of
 * the page. It changes nothing otherwise, so an update that keeps the row (or finds focus elsewhere) never moves
 * focus. A class, because React offers no hook for the moment before the page changes, when the old focus is still
 * there to read.
 */
export class FocusKeeper extends Component<Props> {
  getSnapshotBeforeUpdate(before: Props): Lost | null {
    const root = this.props.within.current
    const active = document.activeElement
    if (root === null || !(active instanceof HTMLElement) || !root.contains(active)) return null
    const game = active.dataset.join
    if (game === undefined || this.props.rows.some((row) => row.game === game)) return null
    return { game, before: before.rows }
  }

  componentDidUpdate(_before: Props, _state: unknown, lost: Lost | null): void {
    if (lost === null) return
    const next = nextJoin(lost.before, lost.game, this.props.rows)
    const buttons = this.props.within.current?.querySelectorAll<HTMLElement>('[data-join]') ?? []
    const target = [...buttons].find((button) => button.dataset.join === next)
    ;(target ?? this.props.fallback.current)?.focus()
  }

  render(): ReactNode {
    return this.props.children
  }
}
