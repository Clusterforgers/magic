import { Lock, LockOpen, RefreshCw } from 'lucide-react'
import { useContext, useState } from 'react'
import { Button } from '@/components/ui/button'
import { CardImage } from '@/features/cards/CardImage'
import { useImageChoice } from '@/features/cards/imageSetting'
import {
  cardImageFallbackUrl,
  cardImageUrl,
  faceOf,
  imageSizeFor,
  imageSourceOf,
} from '@/features/cards/images'
import { useCardDetail } from '@/features/builder/useCardDetail'
import { cn } from '@/lib/utils'
import type { CardsApi } from '@/net/cards'
import type { CardFaceView, ObjectView } from '@/protocol/generated'
import { CardFace } from './CardFace'
import { CardsApiContext } from './cardsContext'
import { SIZE_PX } from './ImageFace'
import type { CombatRole } from './cardModel'
import { SIZE } from './cardSize'
import type { CardSize } from './cardSize'
import { useDoors } from './useDoors'
import { doorWord } from './rooms'

interface PreviewCardProps {
  object: ObjectView
  role: CombatRole | null
  size: CardSize
  /** The card is face up to every seat: its face carries the "Revealed" mark. */
  revealed?: boolean
  /** The cards API that gives the oracle text; the one of the table's context (the server's) by default. */
  api?: CardsApi
}

/** The faces of a card as printed, or none while they are asked for or cannot be read. */
function usePrintedFaces(object: ObjectView, api: CardsApi): readonly CardFaceView[] {
  const { state } = useCardDetail(api, object.identity?.oracle_id ?? null)
  return state.status === 'ready' ? state.detail.faces : []
}

/**
 * The preview's card: the picture of the face in use with the object's state over it, a control that turns a card of
 * several faces over, and the oracle text of the face shown as plain, selectable text. The picture is the size the table
 * draws the card at (`normal` at 160 and 240 px, sharp at twice the density): the server's warm-up and the table
 * already hold it, whereas a `large` one is fetched from the CDN behind every other picture of a filling table and
 * leaves the text face, an empty frame, up until it comes.
 */
export function PreviewCard({ object, role, size, revealed = false, api: given }: PreviewCardProps) {
  const inherited = useContext(CardsApiContext)
  const api = given ?? inherited
  const choice = useImageChoice()
  const faces = usePrintedFaces(object, api)
  const doors = useDoors(object)
  const own = faceOf(object)
  const [shown, setShown] = useState(own)
  const source = imageSourceOf(object)
  const picture = imageSizeFor(SIZE_PX[size])
  // The halves of a Room are doors listed with their state, not faces to turn over.
  const flips = doors === undefined && faces.length > 1 && source !== null && choice === 'on'
  const face = faces[shown] ?? faces[0]
  const other = shown === own ? (own === 0 ? 1 : 0) : own
  return (
    <div className="grid gap-2">
      {flips && shown !== own ? (
        <CardImage
          src={cardImageUrl(source, shown, picture)}
          fallback={cardImageFallbackUrl(source, shown, picture)}
          alt={faces[shown]?.name ?? object.name}
          eager
          className={cn('block aspect-[5/7] w-(--card-w)', SIZE[size].vars)}
        >
          <CardFace object={object} role={role} size={size} upright />
        </CardImage>
      ) : (
        <CardFace
          object={object}
          role={role}
          size={size}
          upright
          imageSize={picture}
          revealed={revealed}
          handNote={object.note}
        />
      )}
      {flips ? (
        <Button variant="outline" size="sm" className="w-fit" onClick={() => setShown(other)}>
          <RefreshCw aria-hidden="true" />
          Show {faces[other]?.name ?? 'the other face'}
        </Button>
      ) : null}
      {doors !== undefined ? (
        <ul aria-label="Doors" data-slot="preview-doors" className="grid gap-2 text-sm">
          {doors.map((door, at) => (
            <li key={at} data-door={at} data-unlocked={door.unlocked} className="grid gap-0.5">
              <p className="inline-flex items-center gap-1 font-medium">
                {door.unlocked ? (
                  <LockOpen aria-hidden="true" className="size-3.5 shrink-0" />
                ) : (
                  <Lock aria-hidden="true" className="size-3.5 shrink-0" />
                )}
                <span>{doorWord(door)}</span>
              </p>
              {(faces[at]?.oracle_text ?? '') === '' ? null : (
                <p data-slot="preview-oracle" className="whitespace-pre-line select-text">
                  {faces[at]?.oracle_text}
                </p>
              )}
            </li>
          ))}
        </ul>
      ) : face === undefined || face.oracle_text === '' ? null : (
        <p data-slot="preview-oracle" className="text-sm whitespace-pre-line select-text">
          {face.oracle_text}
        </p>
      )}
    </div>
  )
}
