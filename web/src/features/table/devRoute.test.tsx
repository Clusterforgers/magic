// Goal: /dev/table serves the real table over the fixture in development, reads seats, mode and focus from the address (and shrugs off nonsense), and answers "not found" with no fixture when the build is a production one.
import { createMemoryHistory, createRouter, RouterProvider } from '@tanstack/react-router'
import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { maybePrimaryButton, primaryButton } from '@/test-utils/passControls'
import { FIXTURE_MARKER } from './dev/fixture'

/** The real route tree, loaded fresh so the route sees the `import.meta.env.DEV` the test has set. */
async function visit(url: string) {
  vi.resetModules()
  const { routeTree } = await import('@/routeTree.gen')
  const router = createRouter({ routeTree, history: createMemoryHistory({ initialEntries: [url] }) })
  render(<RouterProvider router={router} />)
  await act(async () => {
    await router.load()
  })
  return router
}

const panels = (): number[] =>
  [...document.querySelectorAll('[data-seat]')].map((element) => Number(element.getAttribute('data-seat')))

afterEach(() => {
  cleanup()
  vi.unstubAllEnvs()
})

describe('/dev/table in development', () => {
  it('shows a four-seat grid table when the address says nothing', async () => {
    await visit('/dev/table')
    // The first visit pays for importing the whole route tree again, which can pass 1 s under a loaded runner.
    await screen.findByRole('region', { name: 'Game table' }, { timeout: 8000 })
    expect(panels()).toEqual([1, 2, 3, 0])
    expect(document.querySelector('[data-layout]')).toHaveAttribute('data-layout', 'grid')
    expect(document.querySelector(`[data-fixture="${FIXTURE_MARKER}"]`)).not.toBeNull()
  }, 10_000)

  it.each([
    ['?seats=2', 'grid', [1, 0]],
    ['?seats=12', 'focus', [1, 0]],
    ['?seats=12&mode=grid', 'grid', [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 0]],
    ['?seats=4&mode=focus&focus=3', 'focus', [3, 0]],
    ['?seats=6&focus=5', 'focus', [5, 0]],
  ])('reads %s', async (search, mode, seats) => {
    await visit(`/dev/table${search}`)
    await screen.findByRole('region', { name: 'Game table' })
    expect(document.querySelector('[data-layout]')).toHaveAttribute('data-layout', mode)
    expect(panels()).toEqual(seats)
  })

  it.each(['?seats=99', '?seats=1', '?seats=abc', '?mode=sideways&seats=3'])(
    'ignores the nonsense in %s',
    async (search) => {
      await visit(`/dev/table${search}`)
      await screen.findByRole('region', { name: 'Game table' })
      expect(panels().length).toBeGreaterThanOrEqual(2)
      expect(panels().length).toBeLessThanOrEqual(4)
    },
  )

  it('is the real table, with its toolbar, banner and drawer', async () => {
    await visit('/dev/table?seats=2')
    await screen.findByRole('region', { name: 'Game table' })
    expect(screen.getByRole('toolbar', { name: 'Table layout' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Turn 5 · Bo · Declare blockers' })).toBeInTheDocument()
    expect(screen.getByRole('complementary', { name: 'Game log and chat' })).toBeInTheDocument()
  })
})

const findPrimary = (): Promise<HTMLElement> => waitFor(() => primaryButton())

describe('/dev/table with a prompt', () => {
  it('shows no decision by default', async () => {
    await visit('/dev/table?seats=2')
    await screen.findByRole('region', { name: 'Game table' })
    expect(document.querySelector('[data-decision]')).toBeNull()
    expect(maybePrimaryButton()).toBeNull()
  })

  it.each(['?seats=2&prompt=priority', '?seats=4&mode=grid&prompt=priority'])(
    'shows the priority prompt for %s, with legal cards marked',
    async (search) => {
      await visit(`/dev/table${search}`)
      await screen.findByRole('region', { name: 'Game table' })
      expect(document.querySelector('[data-decision="Priority"]')).not.toBeNull()
      expect(primaryButton()).toBeEnabled()
      expect(document.querySelectorAll('[data-highlight="legal"]').length).toBeGreaterThan(3)
      expect(document.querySelectorAll('[data-highlight="dimmed"]').length).toBeGreaterThan(3)
    },
  )

  it('lets the prompt be answered, which clears it', async () => {
    await visit('/dev/table?seats=2&prompt=priority')
    const pass = await findPrimary()
    await act(async () => pass.click())
    expect(document.querySelector('[data-decision]')).toBeNull()
    expect(document.querySelectorAll('[data-highlight]')).toHaveLength(0)
  })

  it('ignores a prompt it does not know', async () => {
    await visit('/dev/table?seats=2&prompt=nonsense')
    await screen.findByRole('region', { name: 'Game table' })
    expect(document.querySelector('[data-decision]')).toBeNull()
  })
})

describe('/dev/table with the controls of the Priority prompt', () => {
  it.each([
    ['priority', 'Resolve Lightning Bolt', true],
    ['priority-main', 'Next: Combat', true],
    ['priority-end', 'End turn', false],
    ['priority-wait', 'Continue', true],
  ])('?prompt=%s reads "%s" (an End turn beside it: %s)', async (prompt, label, hasSecondary) => {
    await visit(`/dev/table?seats=2&prompt=${prompt}`)
    await screen.findByRole('region', { name: 'Game table' })
    expect(await findPrimary()).toHaveTextContent(label)
    expect(document.querySelector('[data-slot="secondary-action"]') !== null).toBe(hasSecondary)
    expect(document.querySelector('[data-slot="yield-indicator"]')).toBeNull()
  })

  it('starts a yield from the prompt as the server would: the prompt gives way to the line, which can be stopped', async () => {
    await visit('/dev/table?seats=2&prompt=priority')
    await findPrimary()
    await act(async () => document.querySelector<HTMLElement>('[data-slot="secondary-action"]')?.click())
    expect(document.querySelector('[data-decision]')).toBeNull()
    expect(await screen.findByText('Passing until the end of turn')).toBeInTheDocument()
    expect(document.querySelectorAll('[data-highlight]')).toHaveLength(0)

    await act(async () => screen.getByRole('button', { name: 'Stop passing' }).click())
    expect(document.querySelector('[data-slot="yield-indicator"]')).toBeNull()
  })

  it.each([
    ['yield', 'Passing until the end of turn'],
    ['yield-resolve', 'Passing until the stack is empty'],
    ['yield-turn', 'Passing until your next turn'],
  ])(
    '?prompt=%s starts passing: "%s", no decision and nothing to press but Stop passing',
    async (prompt, words) => {
      await visit(`/dev/table?seats=2&prompt=${prompt}`)
      await screen.findByRole('region', { name: 'Game table' })
      expect(await screen.findByText(words)).toBeInTheDocument()
      expect(document.querySelector('[data-decision]')).toBeNull()
      expect(maybePrimaryButton()).toBeNull()
      expect(screen.getByRole('button', { name: 'Stop passing' })).toBeEnabled()
    },
  )
})

describe('/dev/table with the viewer’s own turn', () => {
  it.each(['?seats=2&turn=you', '?seats=4&turn=you&prompt=priority', '?seats=12&turn=you'])(
    'lights the your-turn cue for %s',
    async (search) => {
      await visit(`/dev/table${search}`)
      await screen.findByRole('region', { name: 'Game table' })
      expect(screen.getByRole('heading', { name: /^Turn 5 · Alice · / })).toBeInTheDocument()
      expect([...document.querySelectorAll('[data-your-turn]')].map((element) => element.tagName)).toEqual([
        'SECTION',
        'SECTION',
      ])
      expect(document.querySelector('section[data-seat="0"]')).toHaveAttribute('data-your-turn', 'true')
    },
  )

  it.each(['?seats=2', '?seats=2&turn=them', '?seats=2&turn=1'])('shows no cue for %s', async (search) => {
    await visit(`/dev/table${search}`)
    await screen.findByRole('region', { name: 'Game table' })
    expect(document.querySelector('[data-your-turn]')).toBeNull()
  })
})

describe('/dev/table with table effects', () => {
  const buttons = (): string[] =>
    [...screen.getByRole('group', { name: 'Table effects (development)' }).querySelectorAll('button')].map(
      (button) => button.textContent ?? '',
    )

  it('offers a button for every move, with or without `fx`', async () => {
    await visit('/dev/table?seats=2')
    await screen.findByRole('region', { name: 'Game table' })
    expect(buttons()).toEqual(['attack', 'block', 'damage', 'death', 'life', 'enter'])
    expect(document.querySelector('[data-fx]')).toBeNull()
  })

  it('plays the move named by `fx` once the page has loaded, with the effect on', async () => {
    await visit('/dev/table?seats=2&fx=damage&slow=4')
    await screen.findByRole('region', { name: 'Game table' })
    expect(document.querySelector('[data-fx]')).toBeNull()
    await waitFor(() => expect(document.querySelector('[data-fx="damaged"]')).not.toBeNull(), {
      timeout: 4000,
    })
    expect(document.querySelectorAll('[data-slot="fx-float"]').length).toBeGreaterThan(0)
  })

  it('plays a move when its button is pressed, which changes the table either way', async () => {
    await visit('/dev/table?seats=2')
    await screen.findByRole('region', { name: 'Game table' })
    const bo = (): string =>
      document.querySelector('section[data-seat="1"] [data-slot="life"]')?.textContent ?? ''
    expect(bo()).toContain('38')
    await act(async () => screen.getByRole('button', { name: 'life' }).click())
    expect(bo()).toContain('33')
    // Effects follow the build: off under vitest unless the address asks for one.
    expect(document.querySelector('[data-fx]')).toBeNull()
  })

  it('draws the effect of a pressed button once the address has asked for effects', async () => {
    await visit('/dev/table?seats=2&fx=enter')
    await screen.findByRole('region', { name: 'Game table' })
    await act(async () => screen.getByRole('button', { name: 'life' }).click())
    expect(document.querySelector('[data-slot="life"][data-fx]')).not.toBeNull()
  })

  it('stretches the effects by `slow` and leaves the table alone without it', async () => {
    await visit('/dev/table?seats=2&slow=4')
    const table = await screen.findByRole('region', { name: 'Game table' })
    expect(table.style.getPropertyValue('--fx-slow')).toBe('4')
    cleanup()
    await visit('/dev/table?seats=2')
    expect((await screen.findByRole('region', { name: 'Game table' })).getAttribute('style')).toBeNull()
  })

  it.each(['?seats=2&fx=zap', '?seats=2&slow=500', '?seats=2&slow=0.5', '?seats=2&slow=fast'])(
    'ignores the nonsense in %s',
    async (search) => {
      await visit(`/dev/table${search}`)
      const table = await screen.findByRole('region', { name: 'Game table' })
      expect(table.getAttribute('style')).toBeNull()
      await new Promise((resolve) => setTimeout(resolve, 900))
      expect(document.querySelector('[data-fx]')).toBeNull()
    },
  )
})

describe('/dev/table with stops', () => {
  const lit = (): string[] =>
    [...document.querySelectorAll<HTMLElement>('[data-slot="stop"][aria-pressed="true"]')].map(
      (stop) => `${stop.dataset['side']}:${stop.dataset['step']}:${stop.dataset['keep']}`,
    )
  const stop = (side: string, step: string): HTMLElement => {
    const found = document.querySelector<HTMLElement>(
      `[data-slot="stop"][data-side="${side}"][data-step="${step}"]`,
    )
    if (found === null) throw new Error(`no stop at ${side} ${step}`)
    return found
  }

  it('shows the ladder with nothing lit when the address says nothing', async () => {
    await visit('/dev/table?seats=2')
    await screen.findByRole('group', { name: 'Stops' })
    expect(document.querySelectorAll('[data-slot="stop"]')).toHaveLength(20)
    expect(lit()).toEqual([])
    expect(screen.getByRole('button', { name: 'Full Control: off' })).toHaveAttribute('aria-pressed', 'false')
  })

  it('lights the stops of ?stops=, a lasting one with its pin', async () => {
    await visit('/dev/table?seats=2&stops=own:main1,opp:end:keep,own:upkeep:keep')
    await screen.findByRole('group', { name: 'Stops' })
    expect(lit()).toEqual(['Own:Upkeep:lasting', 'Own:Main1:turn', 'Opponent:End:lasting'])
    expect(stop('Own', 'Upkeep')).toHaveAccessibleName(
      'Your upkeep stop: on every turn. Press to turn it off',
    )
    expect(stop('Own', 'Main1')).toHaveAccessibleName(
      'Your precombat main stop: on for one turn. Press to turn it off',
    )
  })

  it('lets a stop be set and cleared as the server would: the icon follows the answer', async () => {
    await visit('/dev/table?seats=2&stops=own:main1')
    await screen.findByRole('group', { name: 'Stops' })
    await act(async () => stop('Own', 'Draw').click())
    expect(lit()).toEqual(['Own:Draw:turn', 'Own:Main1:turn'])
    await act(async () => stop('Own', 'Main1').click())
    expect(lit()).toEqual(['Own:Draw:turn'])
  })

  it.each([
    ['held', 'Full Control: held'],
    ['next', 'Full Control: next window'],
  ])('shows ?fc=%s as "%s"', async (fc, name) => {
    await visit(`/dev/table?seats=2&fc=${fc}`)
    expect(await screen.findByRole('button', { name })).toHaveAttribute('aria-pressed', 'true')
  })

  it('has no yield controls in the Priority prompt under ?fc=held', async () => {
    await visit('/dev/table?seats=2&prompt=priority&fc=held')
    await screen.findByRole('region', { name: 'Game table' })
    await waitFor(() => primaryButton())
    expect(document.querySelector('[data-slot="secondary-action"]')).toBeNull()
    expect(document.querySelector('[data-slot="more-passing"]')).toBeNull()
  })

  it.each(['?seats=2&stops=', '?seats=2&stops=nonsense', '?seats=2&stops=own:untap', '?seats=2&fc=on'])(
    'ignores the nonsense in %s',
    async (search) => {
      await visit(`/dev/table${search}`)
      await screen.findByRole('group', { name: 'Stops' })
      expect(lit()).toEqual([])
      expect(screen.getByRole('button', { name: 'Full Control: off' })).toBeInTheDocument()
    },
  )
})

describe('/dev/table in a production build', () => {
  it('answers not found and draws no fixture', async () => {
    vi.stubEnv('DEV', false)
    await visit('/dev/table?seats=4')
    expect(await screen.findByText('Not Found')).toBeInTheDocument()
    expect(document.querySelector('[data-seat]')).toBeNull()
    expect(document.querySelector('[data-fixture]')).toBeNull()
  })
})
