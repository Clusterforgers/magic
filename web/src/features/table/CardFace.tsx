import { cn } from '@/lib/utils'
import type { ObjectView } from '@/protocol/generated'
import { costPips, frameOf, powerToughness, statusLabels, typeLine } from './cardModel'
import type { CombatRole } from './cardModel'
import { FRAME, SIZE } from './cardSize'
import type { CardSize } from './cardSize'
import { ManaPips } from './ManaPips'
import type { CardNote } from './prompt/marks'

interface CardFaceProps {
  object: ObjectView
  role: CombatRole | null
  size: CardSize
  /** Keep a tapped card upright, as the preview does: it says "tapped" in words instead. */
  upright?: boolean
  /** What the active prompt writes on the card, such as "At Ana"; drawn after the status labels. */
  note?: CardNote | undefined
}

/**
 * What a card looks like: name, cost, a place for art, type line and power/toughness, in its colour. Plain
 * `span`s so it can sit inside a button. Tapped is a quarter turn plus the word "Tapped", never colour alone.
 */
export function CardFace({ object, role, size, upright = false, note }: CardFaceProps) {
  const classes = SIZE[size]
  const tapped = object.permanent?.tapped === true
  const frame = frameOf(object)
  const full = size !== 'compact'
  const labels = statusLabels(object, role)
  const pt = powerToughness(object)
  return (
    <span
      data-slot="card-face"
      data-frame={frame}
      className={cn(
        'relative flex h-(--card-h) w-(--card-w) flex-col overflow-hidden rounded-md border-2 text-left text-card-foreground shadow-sm ring-1 ring-foreground/15 motion-safe:transition-transform',
        classes.vars,
        FRAME[frame],
        tapped && !upright && 'rotate-90',
      )}
    >
      <span className={cn('px-1 pt-0.5 leading-tight font-semibold', classes.name)}>
        <span className={cn('block break-words hyphens-auto', full ? 'line-clamp-2' : 'line-clamp-3')}>
          {object.name}
        </span>
      </span>
      {full && object.mana_cost !== null ? (
        <ManaPips
          pips={costPips(object.mana_cost)}
          sizeClass={classes.pip}
          label="Mana cost"
          className="px-1 pt-0.5"
        />
      ) : null}
      {/* The place for card art: fill this slot when ObjectView carries an image id. */}
      <span
        data-slot="card-art"
        className="mx-1 my-0.5 flex min-h-0 flex-1 flex-wrap content-start gap-0.5 overflow-hidden rounded-sm bg-foreground/5 p-0.5"
      >
        {labels.map((label) => (
          <span
            key={label}
            className={cn(
              'h-fit rounded-sm bg-foreground px-1 leading-snug font-medium text-background',
              classes.text,
            )}
          >
            {label}
          </span>
        ))}
        {note === undefined ? null : (
          <span
            data-slot="card-note"
            className={cn(
              'h-fit max-w-full rounded-sm bg-primary px-1 leading-snug font-medium break-words text-primary-foreground',
              classes.text,
            )}
          >
            {note.label}
          </span>
        )}
      </span>
      {full ? (
        <span className={cn('line-clamp-2 px-1 leading-tight text-muted-foreground', classes.text)}>
          {typeLine(object)}
        </span>
      ) : null}
      {pt === null ? (
        <span className="h-1" />
      ) : (
        <span
          className={cn(
            'self-end rounded-tl-md bg-foreground px-1.5 leading-snug font-bold text-background tabular-nums',
            classes.name,
          )}
        >
          {pt}
        </span>
      )}
    </span>
  )
}
