import { EllipsisVertical } from 'lucide-react'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { useGame, usePlay } from '@/features/game/gameContext'
import type { AutoYield, ObjectView } from '@/protocol/generated'

/** The key an ability on the stack is yielded to by: what every player reads, the source's name and the ability's words. */
export function yieldKeyOf(object: ObjectView | undefined): AutoYield | null {
  if (object === undefined || object.kind !== 'Ability' || object.name === '') return null
  return { name: object.name, text: object.rules_text ?? '' }
}

/**
 * The menu of one ability on the stack: "Always yield to <ability>" whether or not the seat is asked, or "Stop yielding
 * to <ability>" while it does. Nothing for a spell, and nothing under Full Control, which the server does not let a
 * yield run beside.
 */
export function StackYieldMenu({ object }: { object: ObjectView }) {
  const { session } = useGame()
  const key = yieldKeyOf(object)
  const held = usePlay((state) =>
    key === null
      ? false
      : state.autoYields.some((other) => other.name === key.name && other.text === key.text),
  )
  const available = usePlay((state) => state.fullControl === 'Off' && state.status === 'open')
  if (key === null || !available) return null
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        data-slot="stack-item-menu"
        aria-label={`More for ${object.name}`}
        className="absolute top-1 right-1 grid size-7 place-items-center rounded-md text-muted-foreground outline-offset-2 outline-primary hover:text-foreground focus-visible:outline-2"
      >
        <EllipsisVertical aria-hidden="true" className="size-4" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        {held ? (
          <DropdownMenuItem data-yield="Stop" onClick={() => session.clearAutoYield(key)}>
            Stop yielding to {object.name}
          </DropdownMenuItem>
        ) : (
          <DropdownMenuItem
            data-yield="Always"
            aria-description="Lets this ability, and every later one with the same name and words, resolve without asking you for the rest of the game. Full Control still asks."
            onClick={() => session.autoYield(object.id)}
          >
            Always yield to {object.name}
          </DropdownMenuItem>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
