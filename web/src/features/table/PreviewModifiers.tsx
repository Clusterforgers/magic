import { useEffect, useRef } from 'react'
import type { ObjectView } from '@/protocol/generated'
import { cn } from '@/lib/utils'
import { modifierLines, useSourceLit } from './modifiers'
import type { ModifierLine } from './modifiers'

/** The words of one line: the effect, then its source and duration in the quieter colour. */
function LineWords({ line }: { line: ModifierLine }) {
  return (
    <span aria-hidden="true">
      <span data-slot="preview-modifier-effect" className="font-medium tabular-nums">
        {line.effect}
      </span>
      <span className="text-muted-foreground">
        {' · '}
        <span data-slot="preview-modifier-source">{line.source}</span>
        {line.duration === null ? null : `, ${line.duration}`}
      </span>
    </span>
  )
}

/**
 * The preview's list of what changes the card now, each line with its source and duration (`ObjectView.modifiers`).
 * Hovering, focusing or tapping a line whose source is on the table points at that card there; nothing is drawn while
 * nothing changes the card.
 */
export function PreviewModifiers({ object }: { object: ObjectView }) {
  const light = useSourceLit((state) => state.light)
  const lit = useSourceLit((state) => state.ids)
  const mouse = useRef(false)
  // A line that pointed at a card stops pointing once the preview closes or shows another card.
  useEffect(() => () => light([]), [light, object.id])
  const lines = modifierLines(object)
  if (lines.length === 0) return null
  return (
    <>
      <p
        aria-hidden="true"
        className="mt-1 text-xs font-semibold tracking-wide text-muted-foreground uppercase"
      >
        Modified by
      </p>
      <ul aria-label="Modified by" data-slot="preview-modifiers" className="grid gap-0.5">
        {lines.map((line) => {
          const on = line.sources.length > 0 && line.sources.every((id) => lit.includes(id))
          return (
            <li
              key={line.key}
              data-slot="preview-modifier"
              data-lit={on || undefined}
              title={line.words}
              className={cn(
                'rounded-sm border-s-2 ps-2',
                on ? 'border-primary bg-muted' : 'border-transparent',
              )}
            >
              {line.sources.length === 0 ? (
                <>
                  <span className="sr-only">{line.words}</span>
                  <LineWords line={line} />
                </>
              ) : (
                <button
                  type="button"
                  aria-label={`${line.words}. Show on the table`}
                  aria-pressed={on}
                  className="w-full cursor-pointer text-start outline-offset-1 outline-primary hover:underline focus-visible:outline-2"
                  // A mouse points while it hovers; a touch or a key press toggles, as a touch has no hover.
                  onPointerEnter={(event) => {
                    if (event.pointerType === 'mouse') light(line.sources)
                  }}
                  onPointerLeave={(event) => {
                    if (event.pointerType === 'mouse') light([])
                  }}
                  onPointerDown={(event) => {
                    mouse.current = event.pointerType === 'mouse'
                  }}
                  onFocus={() => light(line.sources)}
                  onBlur={() => light([])}
                  onClick={() => {
                    if (mouse.current) light(line.sources)
                    else light(on ? [] : line.sources)
                    mouse.current = false
                  }}
                >
                  <LineWords line={line} />
                </button>
              )}
            </li>
          )
        })}
      </ul>
    </>
  )
}
