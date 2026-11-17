// Goal: the run header shows the stage, Heat, the hearts left, the seed code to copy and each hero's life bar, gold and relics, a risky relic marked as such; a phone folds it to the theme and the hearts until opened.
import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { expect, it } from 'vitest'
import { TooltipProvider } from '@/components/ui/tooltip'
import { renderInRouter } from '@/test-utils/router'
import { CONTENT, delveView, hero } from './fakeDelves'
import { RunHeader } from './RunHeader'

it('shows the run and the party at a glance', async () => {
  const view = delveView(
    { seed: 'EMBER-7' },
    {
      status: 'Map',
      heat: 2,
      hearts: 1,
      heroes: [hero({ life: 8, gold: 35, relics: ['idol', 'pact'] }), hero({ name: 'Bo', life: 40 })],
    },
  )
  await renderInRouter(
    <TooltipProvider>
      <RunHeader view={view} content={CONTENT} />
    </TooltipProvider>,
  )
  expect(screen.getByText('Choosing a path')).toBeInTheDocument()
  expect(
    screen.getByLabelText('Heat 2 — Heroic: needs a team of decks that work together and suit the format'),
  ).toHaveTextContent('Heat 2 · Heroic')
  expect(screen.getByRole('img', { name: '1 of 3 hearts left' })).toBeInTheDocument()
  expect(screen.getByRole('button', { name: /Copy seed code/ })).toBeInTheDocument()
  const ana = screen.getByRole('img', { name: 'Ana: 8 life' })
  expect(ana.querySelector('[data-tone]')).toHaveAttribute('data-tone', 'low')
  expect(screen.getByRole('img', { name: 'Bo: 40 life' }).querySelector('[data-tone]')).toHaveAttribute(
    'data-tone',
    'fine',
  )
  const relics = within(screen.getByRole('group', { name: 'Relics of Ana' })).getAllByRole('button')
  expect(relics.map((b) => b.getAttribute('aria-label'))).toEqual([
    'Ember Idol, Rare, relic of Ana: Creatures you control get +1/+0.',
    'Wildfire Pact, Mythic, risky, relic of Ana: All red spells cost {2} less.',
  ])
  expect(relics[1]).toHaveAttribute('data-risky', 'true')
})

it.each([
  ['relaxed', 'Timers: Relaxed'],
  ['quick', 'Timers: Quick'],
  ['off', 'Timers: Off'],
] as const)('names the run’s %s timers', async (timers, words) => {
  await renderInRouter(
    <TooltipProvider>
      <RunHeader view={delveView({}, { timers })} content={CONTENT} />
    </TooltipProvider>,
  )
  expect(screen.getByText(words)).toBeInTheDocument()
})

it('folds to one line on a phone, the theme and the hearts, and opens the rest from its button', async () => {
  const view = delveView({}, { status: 'Map', heroes: [hero({ name: 'Ana' }), hero({ name: 'Bo' })] })
  await renderInRouter(
    <TooltipProvider>
      <RunHeader view={view} content={CONTENT} />
    </TooltipProvider>,
  )
  const party = screen.getByRole('list', { name: 'Party' })
  const toggle = screen.getByRole('button', { name: 'Show the run and the party' })
  expect(toggle).toHaveClass('sm:hidden')
  expect(toggle).toHaveAttribute('aria-expanded', 'false')
  for (const folded of [party, screen.getByLabelText(/^Heat/), screen.getByText('Choosing a path')]) {
    expect(folded).toHaveClass('max-sm:hidden')
  }
  expect(screen.getByRole('img', { name: /hearts left/ }).closest('.max-sm\\:hidden')).toBeNull()
  await userEvent.setup().click(toggle)
  expect(screen.getByRole('button', { name: 'Hide the run and the party' })).toHaveAttribute(
    'aria-expanded',
    'true',
  )
  expect(party).not.toHaveClass('max-sm:hidden')
})

it('leaves the party out of the lobby, where the hero cards show it', async () => {
  await renderInRouter(
    <TooltipProvider>
      <RunHeader view={delveView()} content={CONTENT} />
    </TooltipProvider>,
  )
  expect(screen.getByText('Gathering the party')).toBeInTheDocument()
  expect(screen.queryByRole('list', { name: 'Party' })).toBeNull()
})
