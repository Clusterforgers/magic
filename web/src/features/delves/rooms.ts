/** The Delves rooms' names, icons and one-line blurbs, shared by the descent and the vote. */
import { FlameIcon, GemIcon, ScrollTextIcon, StoreIcon, SwordsIcon } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import type { RoomKind } from '@/protocol/generated'

/** Each room kind's label, icon and what it holds. */
export const ROOM: Record<RoomKind, { label: string; icon: LucideIcon; blurb: string }> = {
  boss: { label: 'Boss', icon: SwordsIcon, blurb: '' },
  treasure: { label: 'Treasure', icon: GemIcon, blurb: 'A relic for each hero' },
  shop: { label: 'Shop', icon: StoreIcon, blurb: 'Cards, relics and a draught' },
  event: { label: 'Event', icon: ScrollTextIcon, blurb: 'A choice with a price' },
  campfire: { label: 'Campfire', icon: FlameIcon, blurb: 'Rest or reforge' },
}
