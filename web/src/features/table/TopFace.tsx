import { memo } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { CardFace } from './CardFace'
import { CardWidthContext } from './ImageFace'

/** The face of a zone's top card. It reads its own object and is memoised, so a batch redraws it only when that card changes. */
export const TopFace = memo(function TopFace({ id, width }: { id: number; width: number }) {
  const object = usePlay((state) => state.objectsById[id])
  if (object === undefined) return null
  return (
    <CardWidthContext.Provider value={width}>
      <CardFace object={object} role={null} size="compact" fit imageSize="small" upright />
    </CardWidthContext.Provider>
  )
})
