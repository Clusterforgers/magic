// Goal: each always-on custom card kind maps to its boss-ability chip kind, an encounter modifier as a modifier.
import { describe, expect, it } from 'vitest'

import type { CustomCardView } from '../../../protocol/generated/CustomCardView'
import { standingKind } from './bossAbilities'

const card = (kind: CustomCardView['kind']) => ({ kind, turn: 7 }) as unknown as CustomCardView

describe('standingKind', () => {
  it.each([
    ['Modifier', 'modifier'],
    ['Encounter', 'modifier'],
    ['BossRelic', 'relic'],
    ['Innate', 'passive'],
    ['Boss', 'passive'],
    ['Enrage', 'enrage'],
  ] as const)('%s reads as %s', (kind, want) => {
    expect(standingKind(card(kind)).kind).toBe(want)
  })
})
