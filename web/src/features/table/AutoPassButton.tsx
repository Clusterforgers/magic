import { FastForward } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { useGame, usePlay } from '@/features/game/gameContext'
import type { ObjectView } from '@/protocol/generated'
import { cn } from '@/lib/utils'
import { yieldKeyOf } from './StackYieldMenu'

/**
 * "Auto-pass this trigger" for an ability on the stack (its chip and its preview): the server passes for the seat at
 * once whenever an ability with the same name and words is on top, for the rest of the game; pressed again it takes the
 * yield back. Nothing for a spell, an unnamed ability, under Full Control (which asks anyway) or while offline. It
 * changes when the server says so.
 */
export function AutoPassButton({ object, compact = false }: { object: ObjectView; compact?: boolean }) {
  const { session } = useGame()
  const key = yieldKeyOf(object)
  const held = usePlay((state) =>
    key === null
      ? false
      : state.autoYields.some((other) => other.name === key.name && other.text === key.text),
  )
  const available = usePlay((state) => state.fullControl === 'Off' && state.status === 'open')
  if (key === null || !available) return null
  const label = held ? `Stop auto-passing ${object.name}` : `Auto-pass this trigger: ${object.name}`
  return (
    <Button
      type="button"
      data-slot="auto-pass"
      data-held={held || undefined}
      size={compact ? 'icon-sm' : 'sm'}
      variant={held ? 'default' : 'outline'}
      aria-pressed={held}
      aria-label={label}
      title={held ? 'Stop auto-passing this trigger' : 'Auto-pass this trigger for the rest of the game'}
      className={cn(!compact && 'w-fit gap-1.5 text-xs')}
      onClick={(event) => {
        event.stopPropagation()
        if (held) session.clearAutoYield(key)
        else session.autoYield(object.id)
      }}
    >
      <FastForward aria-hidden="true" />
      {compact ? null : <span>{held ? 'Auto-passing' : 'Auto-pass this trigger'}</span>}
    </Button>
  )
}
