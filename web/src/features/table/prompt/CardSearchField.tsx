import { useEffect, useRef } from 'react'
import type { KeyboardEvent } from 'react'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'
import { filterCount } from './cardsText'
import { isTyping } from './usePromptKeys'

/** Props of `CardSearchField`. */
export interface CardSearchFieldProps {
  /** The words typed. */
  value: string
  /** A change of the words. */
  onChange(value: string): void
  /** The cards in view and on offer, and the chosen ones the words hide, for the count beside the field. */
  shown: number
  total: number
  hiddenChosen?: number
  /** Arrow Down or Enter: focus goes to the first card in view. */
  onLeave(): void
  /** The field takes focus when it appears (not on a touch screen, where it would open the keyboard over the cards). */
  focusOnOpen?: boolean
  /** Classes of the box around the field and the count. */
  className?: string
}

/** Whether the main pointer is a finger. */
function coarsePointer(): boolean {
  return typeof window.matchMedia === 'function' && window.matchMedia('(pointer: coarse)').matches
}

/**
 * The search box over a list of cards: the words match a card's name, type line and rules text, the count beside it
 * says how many of the cards are in view, Arrow Down or Enter goes on to the cards, and Escape clears the words before
 * it does anything else.
 */
export function CardSearchField({
  value,
  onChange,
  shown,
  total,
  hiddenChosen = 0,
  onLeave,
  focusOnOpen = false,
  className,
}: CardSearchFieldProps) {
  const field = useRef<HTMLInputElement>(null)
  useEffect(() => {
    if (!focusOnOpen || coarsePointer() || isTyping(document.activeElement)) return
    field.current?.focus({ preventScroll: true })
  }, [focusOnOpen])

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>): void => {
    if (event.altKey || event.ctrlKey || event.metaKey) return
    if (event.key === 'Escape' && value !== '') {
      event.preventDefault()
      event.stopPropagation()
      onChange('')
    } else if ((event.key === 'ArrowDown' || event.key === 'Enter') && shown > 0) {
      event.preventDefault()
      onLeave()
    }
  }

  return (
    <div className={cn('flex min-w-0 items-center gap-2', className)}>
      <Input
        ref={field}
        type="search"
        aria-label="Filter cards"
        placeholder="Search name, type or text"
        autoComplete="off"
        spellCheck={false}
        enterKeyHint="search"
        value={value}
        className="h-9 min-w-0 flex-1"
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={onKeyDown}
      />
      <span
        data-slot="filter-count"
        aria-live="polite"
        className="shrink-0 text-xs text-muted-foreground tabular-nums"
      >
        {filterCount(shown, total, hiddenChosen)}
      </span>
    </div>
  )
}
