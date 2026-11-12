import { useId } from 'react'
import { Button } from '@/components/ui/button'
import { useGame, usePlay } from '@/features/game/gameContext'
import { quoted } from '../../eventText'

/**
 * "To do by hand": the seat's by-hand lines that came due since it last passed priority (`PlayState.dueLines`), each as
 * its card and its words, read-only, opening the Commands panel. Done forgets them and sends nothing; a pass of the
 * seat's clears them too. Nothing is drawn while none is due.
 */
export function DueGroup() {
  const lines = usePlay((state) => state.dueLines)
  const { session } = useGame()
  const heading = useId()
  if (lines.length === 0) return null
  return (
    <div
      role="group"
      aria-labelledby={heading}
      data-group="due"
      className="grid gap-1.5 rounded-md border border-support-manual px-2 py-1.5"
    >
      <h3 id={heading} className="text-xs font-medium text-support-manual">
        To do by hand
      </h3>
      <ul className="grid gap-1 text-sm">
        {lines.map((line, at) => (
          // The same line can come due twice, so the place in the list is part of the key.
          <li key={`${at}:${line.card}`} data-slot="due-line">
            <span className="font-medium">{line.card}</span>: {quoted(line.text)}
          </li>
        ))}
      </ul>
      <Button
        type="button"
        size="sm"
        variant="outline"
        className="justify-self-start"
        onClick={() => session.clearDueLines()}
      >
        Done
      </Button>
    </div>
  )
}
