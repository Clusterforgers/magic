import { RefreshCw } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { CardImage } from '@/features/cards/CardImage'
import { useImageChoice } from '@/features/cards/imageSetting'
import { cardImageFallbackUrl, cardImageUrl, faceOf, imageSourceOf } from '@/features/cards/images'
import { useCardDetail } from '@/features/builder/useCardDetail'
import { cn } from '@/lib/utils'
import { cardsApi } from '@/net/cards'
import type { CardsApi } from '@/net/cards'
import type { CardFaceView, ObjectView } from '@/protocol/generated'
import { CardFace } from './CardFace'
import type { CombatRole } from './cardModel'
import { SIZE } from './cardSize'
import type { CardSize } from './cardSize'

interface PreviewCardProps {
  object: ObjectView
  role: CombatRole | null
  size: CardSize
  /** The cards API that gives the oracle text; the server's by default. */
  api?: CardsApi
}

/** The faces of a card as printed, or none while they are asked for or cannot be read. */
function usePrintedFaces(object: ObjectView, api: CardsApi): readonly CardFaceView[] {
  const { state } = useCardDetail(api, object.identity?.oracle_id ?? null)
  return state.status === 'ready' ? state.detail.faces : []
}

/**
 * The preview's card: the `large` picture of the face in use with the object's state over it, a control that turns
 * a card of several faces over, and the oracle text of the face shown as plain, selectable text.
 */
export function PreviewCard({ object, role, size, api = cardsApi }: PreviewCardProps) {
  const choice = useImageChoice()
  const faces = usePrintedFaces(object, api)
  const own = faceOf(object)
  const [shown, setShown] = useState(own)
  const source = imageSourceOf(object)
  const flips = faces.length > 1 && source !== null && choice === 'on'
  const face = faces[shown] ?? faces[0]
  const other = shown === own ? (own === 0 ? 1 : 0) : own
  return (
    <div className="grid gap-2">
      {flips && shown !== own ? (
        <CardImage
          src={cardImageUrl(source, shown, 'large')}
          fallback={cardImageFallbackUrl(source, shown, 'large')}
          alt={faces[shown]?.name ?? object.name}
          eager
          className={cn('block aspect-[5/7] w-(--card-w)', SIZE[size].vars)}
        >
          <CardFace object={object} role={role} size={size} upright />
        </CardImage>
      ) : (
        <CardFace object={object} role={role} size={size} upright imageSize="large" />
      )}
      {flips ? (
        <Button variant="outline" size="sm" className="w-fit" onClick={() => setShown(other)}>
          <RefreshCw aria-hidden="true" />
          Show {faces[other]?.name ?? 'the other face'}
        </Button>
      ) : null}
      {face === undefined || face.oracle_text === '' ? null : (
        <p data-slot="preview-oracle" className="text-sm whitespace-pre-line select-text">
          {face.oracle_text}
        </p>
      )}
    </div>
  )
}
