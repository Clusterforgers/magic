// Goal: an ability in the Stack tab offers "Always yield to" on its own menu whenever the seat can yield (asked or not), "Stop yielding to" while it does, and nothing under Full Control, offline or for a spell.
import { screen, within } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { resetFlowPrefs } from '@/play/flowPrefs'
import type { PlayState } from '@/play/store'
import { renderTable } from '@/test-utils/tableHarness'
import { withCrowd } from './dev/crowdScene'
import { buildFixture } from './dev/fixture'
import { StackPanel } from './StackPanel'
import { yieldKeyOf } from './StackYieldMenu'

const crowd = () => withCrowd(buildFixture({ seats: 4 }))
const withState =
  (patch: Partial<PlayState>) =>
  (state: PlayState): PlayState => ({ ...state, ...patch })
const menus = () =>
  within(screen.getByRole('region', { name: 'Stack' })).queryAllByRole('button', { name: /^More for / })

beforeEach(() => {
  window.localStorage.clear()
  resetFlowPrefs()
})

describe('the menu of a stack item', () => {
  it('offers Always yield to for the ability it sits on and asks the session for that object', async () => {
    const autoYield = vi.fn((_id: number) => true)
    const { user } = renderTable({ fixture: crowd(), ui: <StackPanel />, session: { autoYield } })
    const [first] = menus()
    expect(first).toBeDefined()
    await user.click(first as HTMLElement)
    await user.click(await screen.findByRole('menuitem', { name: /^Always yield to / }))
    expect(autoYield).toHaveBeenCalledOnce()
  })

  it('offers Stop yielding to for an ability the seat already yields to, and takes it back', async () => {
    const clearAutoYield = vi.fn(() => true)
    const fixture = crowd()
    const keys = fixture.view.objects
      .filter((o) => o.zone === 'Stack' && o.kind === 'Ability')
      .flatMap((o) => yieldKeyOf(o) ?? [])
    expect(keys.length).toBeGreaterThan(0)
    const { user } = renderTable({
      fixture,
      ui: <StackPanel />,
      session: { clearAutoYield },
      state: withState({ autoYields: keys }),
    })
    const [first] = menus()
    await user.click(first as HTMLElement)
    await user.click(await screen.findByRole('menuitem', { name: /^Stop yielding to / }))
    expect(clearAutoYield).toHaveBeenCalledOnce()
  })

  it.each<[string, Partial<PlayState>]>([
    ['Full Control is on', { fullControl: 'Held' }],
    ['the socket is closed', { status: 'closed' }],
  ])('has no menu while %s', (_, patch) => {
    renderTable({ fixture: crowd(), ui: <StackPanel />, state: withState(patch) })
    expect(menus()).toHaveLength(0)
  })

  it('gives a spell no key', () => {
    const spell = crowd().view.objects.find((o) => o.zone === 'Stack' && o.kind !== 'Ability')
    expect(yieldKeyOf(spell)).toBeNull()
  })
})
