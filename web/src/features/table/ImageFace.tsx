import { createContext, useContext } from 'react'
import type { ReactNode } from 'react'
import { CardBack, CardImage } from '@/features/cards/CardImage'
import { cardImageFallbackUrl, cardImageUrl, faceOf } from '@/features/cards/images'
import type { ImageSize, ImageSource } from '@/features/cards/images'
import { cn } from '@/lib/utils'
import { Overlay } from './CardOverlay'
import type { OverlayProps } from './CardOverlay'
import type { CardSize } from './cardSize'

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
  const fallback = cardImageFallbackUrl(source, faceOf(overlay.object), imageSize)
  return (
    <span
      data-slot="card-picture"
      data-turned={turned || undefined}
      className={cn(
        'relative block w-(--card-w) rounded-md',
        // A turned picture stands at the top of its box.
        turned ? 'h-[calc(var(--card-w)*5/7)] self-start' : 'h-(--card-h)',
        box,
        stacked ? PILE : null,
      )}
    >
      <CardImage
        src={src}
        fallback={fallback}
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

interface BackFaceProps extends Omit<OverlayProps, 'turned'> {
  box: string | null
  turned: boolean
  /** The spoken name of the back: "Face-down card" or "Hidden card". */
  label: string
}

/**
 * A card the viewer may not see, or one lying face down: the neutral back, turned when tapped. A face-down permanent
 * carries its public state (damage, counters, attachments, the 2/2 of a morph) in the same small marks as a picture.
 */
export function BackFace({ box, turned, label, ...overlay }: BackFaceProps) {
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
      {overlay.object.face_down === true ? <Overlay {...overlay} turned={turned} /> : null}
    </span>
  )
}
