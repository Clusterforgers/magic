import type { CSSProperties, ReactNode, Ref } from 'react'
import { CardBack } from '@/features/cards/CardImage'
import type { ObjectView } from '@/protocol/generated'
import { CardFace } from '../CardFace'
import { CardWidthContext } from '../ImageFace'
import type { Size } from './travel'

/** A face this wide or wider draws the type line and the rest, not only the name and cost. */
const NORMAL_FACE_PX = 76

const pictureStyle = ({ width, height }: Size): CSSProperties =>
  ({ '--card-w': `${width}px`, '--card-h': `${height}px` }) as CSSProperties

/**
 * A card drawn at `size` over the table: the face (`look`), the back (`back`), or the back with the face laid over it
 * (`face` is the face's element, for the animation that turns it up). Without a `look` it is the back alone.
 */
export function CardPicture({
  look,
  size,
  back,
  face,
  children,
}: {
  look: ObjectView | null
  size: Size
  /** Draw the back (under the face, when there is one). */
  back: boolean
  /** The face's element, for the animation that turns it up. */
  face?: Ref<HTMLSpanElement> | undefined
  children?: ReactNode
}) {
  const layer =
    'relative col-start-1 row-start-1 grid h-(--card-h) w-(--card-w) place-items-center rounded-md'
  const showFace = look !== null
  return (
    <span className="relative grid shadow-lg" style={pictureStyle(size)}>
      {back ? <CardBack className={layer} /> : null}
      {showFace ? (
        <span ref={face} className={`${layer} bg-background`}>
          <CardWidthContext.Provider value={size.width}>
            <CardFace
              object={look}
              role={null}
              size={size.width < NORMAL_FACE_PX ? 'compact' : 'normal'}
              fit
              upright
            />
          </CardWidthContext.Provider>
        </span>
      ) : null}
      {children}
    </span>
  )
}
