import { useEffect, useRef } from 'react'
import type { KeyboardEvent } from 'react'
import { Button } from '@/components/ui/button'
import { usePlay } from '@/features/game/gameContext'
import type { ManaOption, ObjectId } from '@/protocol/generated'
import { ManaDot } from '../ManaPool'

/** Props of `ManaChoice`. */
export interface ManaChoiceProps {
  /** The permanent whose mana is being chosen. */
  card: ObjectId
  /** Its legal mana abilities, two or more. */
  options: readonly ManaOption[]
  onPick(option: ManaOption): void
  onCancel(): void
}

/**
 * Which mana a permanent with several mana abilities should make: one button per ability, Cancel, and Escape to cancel.
 * Focus moves to the first button on open, since the player chose the card elsewhere on the table.
 */
export function ManaChoice({ card, options, onPick, onCancel }: ManaChoiceProps) {
  const name = usePlay((state) => state.objectsById[card]?.name ?? 'the permanent')
  const first = useRef<HTMLButtonElement>(null)
  useEffect(() => first.current?.focus(), [])
  const same = (option: ManaOption): number => options.filter((o) => o.produces === option.produces).length
  return (
    <div
      role="group"
      aria-label={`Mana from ${name}`}
      data-slot="mana-choice"
      className="flex flex-wrap items-center gap-2"
      onKeyDown={(event: KeyboardEvent) => {
        if (event.key !== 'Escape') return
        event.preventDefault()
        onCancel()
      }}
    >
      <span className="text-sm">Tap {name} for</span>
      {options.map((option, index) => {
        const colour = option.produces.toLowerCase()
        // Two abilities of one colour are told apart by their place on the card.
        const label = same(option) > 1 ? `${colour} (ability ${option.source.ability + 1})` : colour
        return (
          <Button
            key={option.source.ability}
            ref={index === 0 ? first : undefined}
            variant="outline"
            className="h-10 px-3"
            aria-label={`Add ${label} mana`}
            onClick={() => onPick(option)}
          >
            <ManaDot type={option.produces} />
            {label}
          </Button>
        )
      })}
      <Button variant="ghost" className="h-10 px-3" onClick={onCancel}>
        Cancel
      </Button>
    </div>
  )
}
