// Goal: on the crowded dev table (`?crowd=1`: a board of triggers and twelve attackers at four seats) the strip folds each run of one source's same ability into one counted chip, keeps only the chips that fit and gives the rest one "+N more" button that holds their arrows, and the Stack tab folds the same runs into a row with a count and a button that lists the others.
import { screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { CROWD_ATTACKERS, withCrowd } from './dev/crowdScene'
import { buildFixture } from './dev/fixture'
import { StackChips } from './StackChips'
import { StackPanel } from './StackPanel'

const crowd = () => withCrowd(buildFixture({ seats: 4 }))
const chips = () => screen.getByRole('list', { name: 'Spells on the stack, top first' })
const counts = (root: HTMLElement): string[] =>
  [...root.querySelectorAll('[data-slot="chip-count"], [data-slot="row-count"]')].map((count) => count.textContent ?? '')

afterEach(() => {
  vi.restoreAllMocks()
})

describe('the crowded scene', () => {
  it('puts thirteen abilities on the three spells and has twelve attackers at the three other seats', () => {
    const { view } = crowd()
    const stack = view.stack.cards ?? []
    expect(stack).toHaveLength(16)
    expect(view.objects.filter((object) => object.zone === 'Stack' && object.kind === 'Ability')).toHaveLength(13)
    expect(view.combat.attackers).toHaveLength(CROWD_ATTACKERS)
    const targets = new Set(view.combat.attackers.map((attacker) => JSON.stringify(attacker.target)))
    expect(targets.size).toBe(3)
  })
})

describe('the chips of a crowded stack', () => {
  it('fold each run of one ability into one chip with its count, top first', () => {
    renderTable({ fixture: crowd(), ui: <StackChips /> })
    const items = within(chips()).getAllByRole('listitem')
    expect(items).toHaveLength(7)
    expect(counts(chips())).toEqual(['×2', '×4', '×6'])
    expect(within(chips()).getAllByRole('button').map((chip) => chip.getAttribute('aria-label'))).toEqual([
      'Top of the stack: Ability of Grizzly Bears (Draw a card.), controlled by Alice',
      'Ability of Air Elemental (Deal 1 damage to any target.), controlled by Bo, 2 times, targeting Cy',
      'Ability of Goblin Chieftain (Whenever a creature dies, each opponent loses 1 life.), controlled by Dee, 4 times',
      'Ability of Serra Angel (Whenever another creature enters, you gain 1 life.), controlled by Bo, 6 times',
      'Lightning Bolt, cost red, Instant, cast by Alice',
      'Giant Growth, cost green, Instant, cast by Bo',
      'Counterspell, cost blue, blue, Instant, cast by Cy',
    ])
  })

  it.each<[string, number, number, string | null]>([
    ['all fit', 2000, 7, null],
    ['three fit', 350, 3, '+9 more'],
    ['only the top fits', 50, 1, '+15 more'],
  ])('keep the chips that fit the strip and fold the rest into one button: %s', (_case, width, shown, more) => {
    // Each chip is 100 px wide from the start of the row.
    vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockImplementation(function (this: HTMLElement) {
      return this.tagName === 'OL' ? width : 0
    })
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
      const index = this.tagName === 'LI' ? [...(this.parentElement?.children ?? [])].indexOf(this) : -1
      return new DOMRect(0, 0, (index + 1) * 100, 32)
    })
    renderTable({ fixture: crowd(), ui: <StackChips /> })
    const visible = within(chips())
      .getAllByRole('listitem', { hidden: true })
      .filter((item) => !item.hasAttribute('aria-hidden'))
    expect(visible).toHaveLength(shown)
    const button = screen.queryByRole('button', { name: /more on the stack/ })
    expect(button?.textContent ?? null).toBe(more)
    if (button !== null) {
      // An arrow to a folded item ends at the button.
      const members = must(button.getAttribute('data-stack-members')).split(' ')
      expect(members).toHaveLength(Number(more?.match(/\d+/)?.[0]))
    }
  })
})

describe('the Stack tab of a crowded stack', () => {
  it('shows each run as its first item with the count, and lists the others on request', async () => {
    const { user } = renderTable({ fixture: crowd(), ui: <StackPanel /> })
    const panel = screen.getByRole('region', { name: 'Stack' })
    expect(counts(panel)).toEqual(['×2', '×4', '×6'])
    expect(within(panel).queryByRole('list', { name: 'The same, in order' })).toBeNull()
    await user.click(within(panel).getByRole('button', { name: 'Show 5 more the same' }))
    const listed = within(panel).getByRole('list', { name: 'The same, in order' })
    expect(within(listed).getAllByRole('listitem')).toHaveLength(5)
    expect(within(panel).getByRole('button', { name: 'Hide 5 more the same' })).toHaveAttribute(
      'aria-expanded',
      'true',
    )
  })
})
