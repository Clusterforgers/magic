/** A relic's rarity as the run page colours it: the word in its tier's colour, the colour also edging the relic. */
import type { Rarity } from '@/protocol/generated'

/** Each tier's text and edge colour: common in stone, rare in relic gold, mythic in the torch's ember. */
export const RARITY_TONE: Record<Rarity, { text: string; border: string }> = {
  Common: { text: 'text-muted-foreground', border: 'border-border-strong' },
  Rare: { text: 'text-relic', border: 'border-relic' },
  Mythic: { text: 'text-delve', border: 'border-delve' },
}
