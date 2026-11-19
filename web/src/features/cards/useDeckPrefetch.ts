import { useEffect } from 'react'
import type { PictureCard } from '@/protocol/generated'
import { markLoaded } from './CardImage'
import { useImageChoice } from './imageSetting'
import {
  createPrefetcher,
  drawnSizes,
  loadPicture,
  mayPrefetch,
  onDrawnSize,
  prefetchUrls,
  whenIdle,
} from './prefetch'
import type { ConnectionHint } from './prefetch'

/**
 * Loads the pictures of `cards` ahead while the page is idle, in each size the table draws (`prefetch.ts`); stops when
 * the list changes or the game leaves the screen. Nothing at all for text faces, Save-Data or a 2G connection.
 */
export function useDeckPrefetch(cards: readonly PictureCard[]): void {
  const choice = useImageChoice()
  useEffect(() => {
    if (cards.length === 0) return
    const connection = (navigator as Navigator & { connection?: ConnectionHint }).connection
    if (!mayPrefetch(choice, connection)) return
    const prefetcher = createPrefetcher({ load: loadPicture, idle: whenIdle, loaded: markLoaded })
    const queue = () => prefetcher.add(prefetchUrls(cards, drawnSizes()))
    // The first idle moment comes after the table's first paint and its own pictures' requests.
    const cancel = whenIdle(queue)
    const off = onDrawnSize(queue)
    return () => {
      cancel()
      off()
      prefetcher.stop()
    }
  }, [cards, choice])
}
