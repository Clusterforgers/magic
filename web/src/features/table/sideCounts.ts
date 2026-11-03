/** Pure counts for the side area's tabs: what has been written since the reader last looked, and how a count is worded. */
import type { ChatLine } from '@/play/store'
import type { PlayerId } from '@/protocol/generated'

/** The tabs of the side area. */
export type SideTab = 'commands' | 'log' | 'chat' | 'stack'

/** The most a tab's count shows; beyond it the badge says "99+". */
export const COUNT_CAP = 99

/** The words of a count on a tab: the number, or "99+". */
export function countLabel(count: number): string {
  return count > COUNT_CAP ? `${COUNT_CAP}+` : String(count)
}

/** How many lines other seats said after `seen` (the last line read; null when none was); all of them if `seen` has left the buffer. */
export function unreadChat(chat: readonly ChatLine[], seen: ChatLine | null, me: PlayerId | null): number {
  const from = seen === null ? 0 : chat.lastIndexOf(seen) + 1
  let count = 0
  for (let at = from; at < chat.length; at += 1) if (chat[at]?.from !== me) count += 1
  return count
}
