import { useEffect, useRef } from 'react'
import type { CSSProperties, KeyboardEvent } from 'react'
import { createPortal } from 'react-dom'
import { usePlay } from '@/features/game/gameContext'
import type { MenuTarget } from '../manual/menuStore'
import { containerOf, usePlaced, useReturnFocus } from '../manual/popup'
import type { Play } from './activations'

/** Props of `AbilityChooser`. */
export interface AbilityChooserProps {
  /** The card and the corner it opens at; its element gets the focus back when the chooser goes. */
  target: MenuTarget
  /** What the card can do, two or more. */
  plays: readonly Play[]
  onPick(play: Play): void
  onClose(): void
}

const items = (panel: HTMLElement): HTMLElement[] => [
  ...panel.querySelectorAll<HTMLElement>('[role="menuitem"]'),
]

/**
 * Which play a card with several should make: a small menu at the card, the manual menu's look. Focus moves to the first
 * line on open and back to the card on close; arrows, Home and End move along the lines, Escape, Tab and a press outside
 * close it. It draws inside the dialog its card sits in (a zone's list), else on the page.
 */
export function AbilityChooser({ target, plays, onPick, onClose }: AbilityChooserProps) {
  const panel = useRef<HTMLDivElement>(null)
  const name = usePlay((state) => state.objectsById[target.id]?.name ?? 'the card')
  useReturnFocus(target)
  usePlaced(panel, target)
  useEffect(() => {
    const element = panel.current
    if (element !== null) items(element)[0]?.focus({ preventScroll: true })
  }, [])
  // A press on the card itself is the card's own click (it closes the chooser), so only the rest of the page closes it.
  useEffect(() => {
    const away = (event: PointerEvent): void => {
      const { opener } = target
      const inside = event.target instanceof Node && panel.current?.contains(event.target) === true
      const onCard = event.target instanceof Node && opener?.contains(event.target) === true
      if (!inside && !onCard) onClose()
    }
    document.addEventListener('pointerdown', away, true)
    return () => document.removeEventListener('pointerdown', away, true)
  }, [target, onClose])

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    event.stopPropagation()
    if (event.key === 'Escape' || event.key === 'Tab') {
      event.preventDefault()
      onClose()
      return
    }
    const lines = items(event.currentTarget)
    const at = lines.indexOf(document.activeElement as HTMLElement)
    const next =
      event.key === 'ArrowDown'
        ? (at + 1) % lines.length
        : event.key === 'ArrowUp'
          ? (at - 1 + lines.length) % lines.length
          : event.key === 'Home'
            ? 0
            : event.key === 'End'
              ? lines.length - 1
              : -1
    if (next < 0) return
    event.preventDefault()
    lines[next]?.focus({ preventScroll: true })
  }

  return createPortal(
    <div
      ref={panel}
      data-slot="ability-chooser"
      data-card={target.id}
      onKeyDown={onKeyDown}
      style={{ '--mx': `${target.x}px`, '--my': `${target.y}px` } as CSSProperties}
      className="fixed z-[70] flex max-h-[min(36rem,calc(100dvh-1rem))] w-72 max-w-[calc(100vw-1rem)] flex-col overflow-hidden rounded-lg border bg-popover text-popover-foreground shadow-e2 max-sm:inset-x-2 max-sm:bottom-2 max-sm:w-auto max-sm:max-h-[72dvh] sm:top-(--my) sm:left-(--mx)"
    >
      <div className="grid gap-0.5 border-b px-3 py-2">
        <p className="truncate text-sm font-medium">{name}</p>
        <p className="text-xs text-muted-foreground">Choose what to do</p>
      </div>
      <div
        role="menu"
        aria-label={`What to do with ${name}`}
        className="min-h-0 overflow-y-auto overscroll-contain px-1 py-1"
      >
        {plays.map((play) => (
          <button
            key={play.key}
            type="button"
            role="menuitem"
            tabIndex={-1}
            aria-label={play.words}
            data-play={play.key}
            onClick={() => onPick(play)}
            className="flex w-full min-w-0 flex-col rounded-md px-3 py-1.5 text-left text-sm outline-none select-none hover:bg-accent hover:text-accent-foreground focus-visible:bg-accent focus-visible:text-accent-foreground pointer-coarse:min-h-11 pointer-coarse:justify-center"
          >
            <span className="break-words">{play.label}</span>
            {play.detail === undefined ? null : (
              <span className="text-xs break-words text-muted-foreground">{play.detail}</span>
            )}
          </button>
        ))}
      </div>
    </div>,
    containerOf(target),
  )
}
