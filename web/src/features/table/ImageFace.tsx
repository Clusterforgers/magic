import { Hand } from 'lucide-react'
import { createContext, useContext } from 'react'
import type { ReactNode } from 'react'
import { CardBack, CardImage } from '@/features/cards/CardImage'
import { cardImageUrl, faceOf } from '@/features/cards/images'
import type { ImageSize, ImageSource } from '@/features/cards/images'
import { cn } from '@/lib/utils'
import type { ObjectView } from '@/protocol/generated'
import { counterLabels, loyaltyOf, statusLabels } from './cardModel'
import type { CombatRole, Links } from './cardModel'
import { SIZE } from './cardSize'
import type { CardSize } from './cardSize'
import { usePrinted } from './printed'
import { statBadge } from './overlay'
import type { CardNote } from './prompt/marks'

/** The width in px of the cards of a fitted row or hand, from the fit engine; null outside one. */
export const CardWidthContext = createContext<number | null>(null)

/** The width in px of a card face of a fixed size (`SIZE`, at 16 px to the rem). */
export const SIZE_PX: Record<CardSize, number> = { compact: 60, normal: 96, medium: 160, large: 240 }

/** The drawn width of a face: the row's fit, else its size's. */
export function useDrawnWidth(size: CardSize, fit: boolean): number {
  const row = useContext(CardWidthContext)
  return fit && row !== null ? row : SIZE_PX[size]
}

// The edges stay inside the 6 px between boxes of a line and the 4 px between lines.
const PILE = 'shadow-[2px_2px_0_0_var(--color-border-strong),4px_4px_0_0_var(--color-border)]'

// State chips sit in the art box of an upright picture, clear of the name line, the type line, the rules text and the
// artist line; under a turned one they use the room its upright box leaves, since its art box is too low for them.
const ART_BOX = {
  upright: 'top-[11%] right-[7%] left-[7%] max-h-[44%]',
  turned: 'top-full right-0 left-0 h-[calc(var(--card-h)-var(--card-w)*5/7)] justify-center overflow-hidden pt-0.5 [&>span]:py-px [&>span]:leading-none',
}
// The power/toughness or loyalty box at the bottom right, which lies at the bottom left once turned.
const STAT_BOX = { upright: 'right-[3%] bottom-[2.5%]', turned: 'bottom-[3%] left-[2.5%]' }

const CHIP = 'h-fit max-w-full rounded-sm px-1 leading-snug break-words shadow-sm'

interface OverlayProps {
  object: ObjectView
  role: CombatRole | null
  size: CardSize
  turned: boolean
  note?: CardNote | undefined
  links?: Links | undefined
}

/** The state of a permanent over its picture: solid chips in the art box, a badge on the stat box, a support mark. */
function Overlay({ object, role, size, turned, note, links }: OverlayProps) {
  const text = SIZE[size].text
  const printed = usePrinted(object)
  const badge = statBadge(object, printed, loyaltyOf(object))
  const manual = object.manual ?? []
  const labels = statusLabels(object, role).filter((label) => !(turned && label === 'Tapped'))
  return (
    <span data-slot="card-overlay" className="pointer-events-none absolute inset-0">
      <span
        className={cn(
          'absolute flex flex-wrap content-start gap-0.5',
          turned ? ART_BOX.turned : cn(ART_BOX.upright, 'overflow-hidden'),
        )}
      >
        {labels.map((label) => (
          <span key={label} className={cn(CHIP, 'bg-foreground font-medium text-background', text)}>
            {label}
          </span>
        ))}
        {counterLabels(object).map((label) => (
          <span
            key={`counter-${label}`}
            data-slot="card-counter"
            className={cn(CHIP, 'border border-foreground bg-background font-semibold text-foreground', text)}
          >
            {label}
          </span>
        ))}
        {links?.host === undefined ? null : (
          <span
            data-slot="card-attached-to"
            className={cn(CHIP, 'border border-dashed border-foreground bg-background text-foreground', text)}
          >
            On {links.host}
          </span>
        )}
        {(links?.attached ?? []).map((name, index) => (
          <span
            // eslint-disable-next-line react/no-array-index-key -- names repeat and the list never reorders
            key={`attached-${index}`}
            data-slot="card-attachment"
            className={cn(CHIP, 'border border-dashed border-foreground bg-background text-foreground', text)}
          >
            + {name}
          </span>
        ))}
        {note === undefined ? null : (
          <span
            data-slot="card-note"
            className={cn(CHIP, 'bg-primary font-medium text-primary-foreground', text)}
          >
            {note.label}
          </span>
        )}
      </span>
      {manual.length === 0 ? null : (
        <span
          data-slot="card-support"
          title={`Played by hand: ${manual.map((clause) => `${clause.text} (${clause.reason})`).join('; ')}`}
          className={cn(
            'pointer-events-auto absolute grid size-5 place-items-center rounded-full border border-background bg-support-manual text-background shadow-sm',
            turned ? 'top-[4%] left-[4%]' : 'top-[11%] right-[4%]',
          )}
        >
          <Hand aria-hidden="true" className="size-3" />
          <span className="sr-only">Some rules are played by hand</span>
        </span>
      )}
      {badge === null ? null : (
        <span
          data-slot={badge.kind === 'loyalty' ? 'card-loyalty' : 'card-pt'}
          data-modified={badge.modified ?? undefined}
          className={cn(
            'absolute rounded-md border px-1.5 leading-snug font-bold tabular-nums shadow-sm',
            SIZE[size].name,
            turned ? STAT_BOX.turned : STAT_BOX.upright,
            badge.modified === 'up'
              ? 'border-background bg-support-verified text-background'
              : badge.modified === 'down'
                ? 'border-background bg-destructive text-background'
                : 'border-background bg-foreground text-background',
          )}
        >
          {badge.text}
        </span>
      )}
    </span>
  )
}

interface ImageFaceProps extends Omit<OverlayProps, 'turned'> {
  source: ImageSource
  /** The face box classes (size variables) the text face would have. */
  box: string | null
  turned: boolean
  stacked: boolean
  imageSize: ImageSize
  /** The text face, the skeleton under the picture. */
  children: ReactNode
}

/** A card drawn as its picture over its text face, with the state over both. */
export function ImageFace({ source, box, turned, stacked, imageSize, children, ...overlay }: ImageFaceProps) {
  const src = cardImageUrl(source, faceOf(overlay.object), imageSize)
  return (
    <span
      data-slot="card-picture"
      data-turned={turned || undefined}
      className={cn(
        'relative block w-(--card-w) rounded-md',
        // A turned picture stands at the top of its box, so its chips have the room below.
        turned ? 'h-[calc(var(--card-w)*5/7)] self-start' : 'h-(--card-h)',
        box,
        stacked ? PILE : null,
      )}
    >
      <CardImage
        src={src}
        alt={overlay.object.name}
        turned={turned}
        className="size-full"
        overlay={<Overlay {...overlay} turned={turned} />}
      >
        {children}
      </CardImage>
    </span>
  )
}

/** A card the viewer may not see, or one lying face down: the neutral back, turned when tapped. */
export function BackFace({ box, turned, label }: { box: string | null; turned: boolean; label: string }) {
  return (
    <span
      data-slot="card-picture"
      data-back
      className={cn(
        'relative grid w-(--card-w) place-items-center',
        turned ? 'h-[calc(var(--card-w)*5/7)]' : 'h-(--card-h)',
        box,
      )}
    >
      <CardBack
        label={label}
        className={turned ? 'h-(--card-w) w-[calc(var(--card-w)*5/7)] rotate-90' : 'size-full'}
      />
    </span>
  )
}
