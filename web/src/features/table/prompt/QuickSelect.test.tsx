/** Goal: the quick selection buttons of a choice of several objects: "Select all", "Select all owned" (the viewer's) and "Select all allied" (teammates' alone, shown only in a team mode) add their group to the choice so far, "Unselect all" empties it; each is off with a reason when it would pass the most or a rule, or adds nothing; none show for a choice of one; and in the target and card prompts a press chooses what Confirm sends. */
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { Fixture } from '@/features/table/dev/fixture'
import type { ObjectId, Target, Teams } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderDecision } from '@/test-utils/combatTable'
import { quickButtons } from './quickSelect'
import type { QuickInput, QuickKind } from './quickSelect'

/** Choices 1 to 6: seat 0 controls 1 and 2, seat 1 controls 3 and 4, seat 2 controls 5 and 6. */
const LEGAL = [1, 2, 3, 4, 5, 6]
const seatOf = (choice: number): number => Math.floor((choice - 1) / 2)

function input(over: Partial<QuickInput<number>> = {}): QuickInput<number> {
  return {
    legal: LEGAL,
    chosen: [],
    max: 6,
    same: (a, b) => a === b,
    seatOf,
    me: 0,
    allies: [],
    ...over,
  }
}

const byKind = (over: Partial<QuickInput<number>>, kind: QuickKind) =>
  quickButtons(input(over)).find((button) => button.kind === kind)

describe('quickButtons', () => {
  it.each([
    ['all', {}, [1, 2, 3, 4, 5, 6]],
    ['owned', {}, [1, 2]],
    ['allied', { allies: [2] }, [5, 6]],
    ['owned', { chosen: [5] }, [5, 1, 2]],
    ['allied', { allies: [2], chosen: [1, 2] }, [1, 2, 5, 6]],
    ['all', { chosen: [4] }, [4, 1, 2, 3, 5, 6]],
    ['none', { chosen: [4, 1] }, []],
  ] as const)('%s with %o makes %o', (kind, over, next) => {
    const button = must(byKind(over, kind))
    expect(button.next).toEqual(next)
    expect(button.reason).toBeUndefined()
  })

  it.each([
    ['all', { max: 5 }, 'Only 5 can be chosen'],
    ['owned', { max: 3, chosen: [3, 4] }, 'Only 3 can be chosen'],
    ['allied', { allies: [2], chosen: [5, 6] }, 'All of them are chosen'],
    ['none', {}, 'Nothing is chosen'],
    ['all', { breaks: () => 'Too much power' }, 'Too much power'],
  ] as const)('%s with %o is off: %s', (kind, over, reason) => {
    expect(must(byKind(over, kind)).reason).toBe(reason)
  })

  it.each([
    ['a choice of one', { max: 1 }],
    ['a single legal choice', { legal: [1] }],
  ])('draws none for %s', (_, over) => {
    expect(quickButtons(input(over))).toEqual([])
  })

  it('shows allied only in a team mode, and never for the viewer', () => {
    const kinds = (over: Partial<QuickInput<number>>) =>
      quickButtons(input(over)).map((button) => button.kind)
    expect(kinds({})).toEqual(['all', 'owned', 'none'])
    expect(kinds({ allies: [2] })).toEqual(['all', 'owned', 'allied', 'none'])
    expect(kinds({ allies: [0] })).toEqual(['all', 'owned', 'none'])
    expect(kinds({ me: 3 })).toEqual(['all', 'none'])
  })
})

const TEAMS: Teams = { mode: 'teams', seats: [0, 1, 0, 1] }

/** The 4-seat fixture, with seats 0 and 2 on one team when `teams` is set. */
function table(teams: Teams | undefined): Fixture {
  const fixture = structuredClone(fixtureOf(4))
  if (teams !== undefined) fixture.view.house_rules = { teams }
  return fixture
}

const objects = (ids: readonly ObjectId[]): Target[] => ids.map((id) => ({ Object: { id, counter: 0 } }))

function mountTargets(teams: Teams | undefined, max: number) {
  const fixture = table(teams)
  const own = creaturesOf(fixture, 0).slice(0, 2)
  const foe = creaturesOf(fixture, 1).slice(0, 2)
  const ally = creaturesOf(fixture, 2).slice(0, 2)
  const legal = objects([...own, ...foe, ...ally])
  const source = must(
    fixture.view.objects.find((object) => object.zone === 'Stack' && object.controller === 0),
  ).id
  const rendered = renderDecision(
    { ChooseTargets: { source, spec: 0, legal, min: 0, max, can_cancel: true, wording: 'target creature' } },
    { fixture },
  )
  const quick = (): HTMLElement => screen.getByRole('group', { name: 'Quick selection' })
  const press = (name: string): HTMLElement => within(quick()).getByRole('button', { name })
  const confirm = async (): Promise<unknown> => {
    await rendered.user.click(screen.getByRole('button', { name: 'Confirm targets' }))
    return rendered.answer.mock.calls[0]?.[0]
  }
  return { ...rendered, own, foe, ally, quick, press, confirm }
}

const sent = (ids: readonly ObjectId[]) => ({ ChooseTargets: { Targets: objects(ids) } })

describe('in the target prompt', () => {
  it('adds own and then allied, so both together take the team', async () => {
    const { press, user, confirm, own, ally } = mountTargets(TEAMS, 6)
    await user.click(press('Select all owned'))
    await user.click(press('Select all allied'))
    expect(await confirm()).toEqual(sent([...own, ...ally]))
  })

  it('selects all, and Unselect all takes everything back', async () => {
    const { press, user, confirm, own, foe, ally } = mountTargets(TEAMS, 6)
    await user.click(press('Select all'))
    expect(press('Select all')).toHaveAttribute('aria-disabled', 'true')
    await user.click(press('Unselect all'))
    expect(press('Unselect all')).toHaveAttribute('aria-disabled', 'true')
    await user.click(press('Select all'))
    expect(await confirm()).toEqual(sent([...own, ...foe, ...ally]))
  })

  it('keeps "Select all" off with its reason when all pass the most', async () => {
    const { press, user, answer } = mountTargets(TEAMS, 4)
    const all = press('Select all')
    expect(all).toHaveAttribute('aria-disabled', 'true')
    expect(all).toHaveAccessibleDescription('Only 4 can be chosen')
    await user.click(all)
    await user.click(screen.getByRole('button', { name: 'Confirm targets' }))
    expect(answer.mock.calls[0]?.[0]).toEqual(sent([]))
  })

  it('has no allied button outside a team mode', () => {
    const { quick } = mountTargets(undefined, 6)
    expect(within(quick()).queryByRole('button', { name: 'Select all allied' })).toBeNull()
    expect(
      within(quick())
        .getAllByRole('button')
        .map((button) => button.textContent),
    ).toEqual(['Select all', 'Select all owned', 'Unselect all'])
  })

  it('draws no quick buttons for a choice of one', () => {
    mountTargets(TEAMS, 1)
    expect(screen.queryByRole('group', { name: 'Quick selection' })).toBeNull()
  })
})

describe('in the card prompt', () => {
  it('chooses every own card for a sacrifice of any number', async () => {
    const fixture = table(TEAMS)
    const own = creaturesOf(fixture, 0).slice(0, 3)
    const ally = creaturesOf(fixture, 2).slice(0, 2)
    const { user, answer } = renderDecision(
      { ChooseCards: { source: must(own[0]), from: [...own, ...ally], min: 0, max: 5 } },
      { fixture },
    )
    const quick = screen.getByRole('group', { name: 'Quick selection' })
    await user.click(within(quick).getByRole('button', { name: 'Select all owned' }))
    await user.click(screen.getByRole('button', { name: 'Confirm' }))
    expect(answer.mock.calls[0]?.[0]).toEqual({ ChooseCards: own })
  })
})
