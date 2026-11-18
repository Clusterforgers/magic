// Goal: a final boss's upgraded form is named as the boss it came from on the ladder and in the intro, and a boss of its own carries no such line.
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { DelveContent } from '@/protocol/generated'
import { BossIntro } from './BossIntro'
import { BossLadder } from './BossLadder'
import { CONTENT, delveView } from './fakeDelves'
import { upgradedFrom } from './model'

// The fake ladder ends with Grimgut; here it stands as the upgraded form of Skrix.
const UPGRADED: DelveContent = {
  ...CONTENT,
  bosses: CONTENT.bosses.map((boss) =>
    boss.id === 'grimgut' ? { ...boss, upgrade_of: 'skrix_nest' } : boss,
  ),
}
const WORDS = 'Upgraded form of Skrix, the Rat Queen'

describe('upgradedFrom', () => {
  it.each([
    ['grimgut', UPGRADED, WORDS],
    ['skrix_nest', UPGRADED, null],
    ['grimgut', CONTENT, null],
    [undefined, UPGRADED, null],
    ['nobody', UPGRADED, null],
  ] as const)('words %s', (boss, content, words) => {
    expect(upgradedFrom(content, boss)).toBe(words)
  })

  it('says nothing when the boss it came from is not listed', () => {
    const orphan = { bosses: [{ ...CONTENT.bosses[1]!, upgrade_of: 'gone' }] }
    expect(upgradedFrom(orphan, 'grimgut')).toBeNull()
  })
})

describe('an upgraded final boss', () => {
  it('is named as the boss it came from on the ladder', () => {
    render(<BossLadder view={delveView()} content={UPGRADED} />)
    const last = screen.getByRole('listitem', { name: /^Final boss: Grimgut/ })
    expect(last).toHaveAccessibleName(`Final boss: Grimgut, ${WORDS}, Locked`)
    expect(last.querySelector('[data-slot="upgraded-from"]')).toHaveTextContent(WORDS)
    expect(screen.getAllByText(WORDS)).toHaveLength(1)
  })

  it('is named as the boss it came from in the intro', () => {
    render(<BossIntro view={delveView({ boss: 'grimgut' })} content={UPGRADED} />)
    expect(screen.getByText(WORDS)).toBeInTheDocument()
  })

  it.each([
    ['ladder', CONTENT],
    ['intro', CONTENT],
  ] as const)('carries no such line for a boss of its own in the %s', (place, content) => {
    render(
      place === 'ladder' ? (
        <BossLadder view={delveView()} content={content} />
      ) : (
        <BossIntro view={delveView({ boss: 'grimgut' })} content={content} />
      ),
    )
    expect(document.querySelector('[data-slot="upgraded-from"]')).toBeNull()
  })
})
