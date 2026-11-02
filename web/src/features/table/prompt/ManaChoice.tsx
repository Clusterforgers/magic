import { X } from 'lucide-react'
import { useEffect, useRef } from 'react'
import type { KeyboardEvent } from 'react'
import { Button } from '@/components/ui/button'
import { usePlay } from '@/features/game/gameContext'
import type { ManaOption, ObjectId } from '@/protocol/generated'
import { givesOfOptions } from '../manaMade'
import { ManaMade, madeWords } from '../ManaMade'

/** Props of `ManaChoice`. */
export interface ManaChoiceProps {
  /** The permanent whose mana is being chosen. */
  card: ObjectId
  /** Its legal mana abilities, two or more (one or more for a switch). */
  options: readonly ManaOption[]
  onPick(option: ManaOption): void
  onCancel(): void
  /** Drawn inline beside a source chip: symbols only, the words for a screen reader. */
  compact?: boolean
  /** The first word of each button's name: "Add", or "Switch to" for a source already chosen. */
  verb?: string
}

/**
 * Which mana a permanent with several mana abilities should make: one button per ability, Cancel, and Escape to cancel.
 * Focus moves to the first button on open, since the player chose the card elsewhere on the table.
 */
export function ManaChoice({
  card,
  options,
  onPick,
  onCancel,
  compact = false,
  verb = 'Add',
}: ManaChoiceProps) {
  const name = usePlay((state) => state.objectsById[card]?.name ?? 'the permanent')
  const first = useRef<HTMLButtonElement>(null)
  useEffect(() => first.current?.focus(), [])
  const same = (option: ManaOption): number => options.filter((o) => o.produces === option.produces).length
  return (
    <div
      role="group"
      aria-label={`Mana from ${name}`}
      data-slot="mana-choice"
      className={
        compact
          ? 'flex shrink-0 items-center gap-1 rounded-md border border-primary/60 bg-primary/10 px-1 py-0.5'
          : 'flex flex-wrap items-center gap-2'
      }
      onKeyDown={(event: KeyboardEvent) => {
        if (event.key !== 'Escape') return
        event.preventDefault()
        onCancel()
      }}
    >
      <span className={compact ? 'sr-only' : 'text-sm'}>
        {verb === 'Add' ? `Tap ${name} for` : `${name} makes instead`}
      </span>
      {options.map((option, index) => {
        // The mana of this ability with its amount: "red", "2 colorless", "white and black".
        const gives = { ...givesOfOptions([option]), cost: [], sacrifice: false }
        const colour = madeWords(gives)
        // Two abilities of one colour are told apart by their place on the card.
        const label = same(option) > 1 ? `${colour} (ability ${option.source.ability + 1})` : colour
        return (
          <Button
            key={option.source.ability}
            ref={index === 0 ? first : undefined}
            variant="outline"
            className={compact ? 'h-7 gap-0.5 px-1' : 'h-10 px-3'}
            aria-label={`${verb} ${label} mana`}
            title={compact ? `${verb} ${label} mana` : undefined}
            onClick={() => onPick(option)}
          >
            <ManaMade gives={gives} sizeClass="size-4 text-xs" />
            {compact ? null : label}
          </Button>
        )
      })}
      {compact ? (
        <Button variant="ghost" className="h-7 px-1" aria-label="Cancel" title="Cancel" onClick={onCancel}>
          <X aria-hidden="true" className="size-4" />
        </Button>
      ) : (
        <Button variant="ghost" className="h-10 px-3" onClick={onCancel}>
          Cancel
        </Button>
      )}
    </div>
  )
}
