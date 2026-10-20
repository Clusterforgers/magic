import { cn } from '@/lib/utils'
import type { ObjectView } from '@/protocol/generated'
import {
  costPips,
  counterLabels,
  frameOf,
  keywordLine,
  loyaltyOf,
  powerToughness,
  statusLabels,
  typeLine,
} from './cardModel'
import type { CombatRole, Links } from './cardModel'
import { FRAME, SIZE } from './cardSize'
import type { CardSize } from './cardSize'
import { ManaPips } from './ManaPips'
import type { CardNote } from './prompt/marks'

interface CardFaceProps {
  object: ObjectView
  role: CombatRole | null
  size: CardSize
  /** The box comes from an enclosing row (`Card` `fit`); `size` then only picks which parts of the face are drawn. */
  fit?: boolean
  /** Keep a tapped card upright, as the preview does: it says "tapped" in words instead. */
  upright?: boolean
  /** What the active prompt writes on the card, such as "At Ana"; drawn after the status labels. */
  note?: CardNote | undefined
  /** The face tops a pile of equal permanents: two edges peek out behind it. */
  stacked?: boolean
  /** What it is attached to and what is attached to it, by name: a cue beside the counters. */
  links?: Links | undefined
}

/** The lines of name a face can spare: a face on its side is low, and a compact one has no cost or type line. */
function nameLinesOf(turned: boolean, full: boolean, size: CardSize): string {
  if (turned) return size === 'compact' ? 'line-clamp-1' : 'line-clamp-2'
  return full ? 'line-clamp-2' : 'line-clamp-3'
}

// The edges stay inside the 6 px between boxes of a line and the 4 px between lines.
const PILE = 'shadow-[2px_2px_0_0_var(--color-border-strong),4px_4px_0_0_var(--color-border)]'

/**
 * What a card looks like: name, cost, a place for art, type line and power/toughness, in its colour. Plain
 * `span`s so it can sit inside a button. A tapped card lies on its side inside the box of an upright one: a landscape
 * face as wide as the box and five sevenths as high, its text upright and as large as ever, plus the word "Tapped"
 * (never colour alone). A row therefore counts one box for it, tapped or not.
 */
export function CardFace({
  object,
  role,
  size,
  fit = false,
  upright = false,
  note,
  stacked = false,
  links,
}: CardFaceProps) {
  const classes = SIZE[size]
  const turned = object.permanent?.tapped === true && !upright
  const frame = frameOf(object)
  const full = size !== 'compact' && !turned
  const labels = statusLabels(object, role)
  const keywords = keywordLine(object)
  const pt = powerToughness(object)
  const counters = counterLabels(object)
  const loyalty = loyaltyOf(object)
  const nameLines = nameLinesOf(turned, full, size)
  return (
    <span
      data-slot="card-face"
      data-frame={frame}
      data-turned={turned || undefined}
      className={cn(
        'relative flex w-(--card-w) flex-col overflow-hidden rounded-md border-2 text-left text-card-foreground ring-1 ring-foreground/15',
        stacked ? PILE : 'shadow-sm',
        turned ? 'h-[calc(var(--card-w)*5/7)]' : 'h-(--card-h)',
        fit ? null : classes.vars,
        FRAME[frame],
      )}
    >
      <span className={cn('px-1 pt-0.5 leading-tight font-semibold', classes.name)}>
        <span className={cn('block break-words hyphens-auto', nameLines)}>{object.name}</span>
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
        {counters.map((label) => (
          <span
            key={`counter-${label}`}
            data-slot="card-counter"
            className={cn(
              'h-fit rounded-sm border border-foreground bg-background px-1 leading-snug font-semibold text-foreground',
              classes.text,
            )}
          >
            {label}
          </span>
        ))}
        {links?.host === undefined ? null : (
          <span
            data-slot="card-attached-to"
            className={cn(
              'h-fit max-w-full rounded-sm border border-dashed border-foreground bg-background px-1 leading-snug break-words text-foreground',
              classes.text,
            )}
          >
            On {links.host}
          </span>
        )}
        {(links?.attached ?? []).map((name, index) => (
          <span
            // eslint-disable-next-line react/no-array-index-key -- names repeat and the list never reorders
            key={`attached-${index}`}
            data-slot="card-attachment"
            className={cn(
              'h-fit max-w-full rounded-sm border border-dashed border-foreground bg-background px-1 leading-snug break-words text-foreground',
              classes.text,
            )}
          >
            + {name}
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
      {full && keywords !== null ? (
        <span data-slot="card-keywords" className={cn('line-clamp-2 px-1 leading-tight', classes.text)}>
          {keywords}
        </span>
      ) : null}
      {pt === null && loyalty === null ? (
        <span className="h-1" />
      ) : (
        <span
          data-slot={pt === null ? 'card-loyalty' : undefined}
          className={cn(
            'self-end rounded-tl-md bg-foreground px-1.5 leading-snug font-bold text-background tabular-nums',
            classes.name,
          )}
        >
          {pt ?? loyalty}
        </span>
      )}
    </span>
  )
}
