// Goal: the run header shows the stage, Heat, the hearts left, the seed code to copy and each hero's life bar, gold and relics, a risky relic marked as such.
import { screen, within } from '@testing-library/react'
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
  expect(screen.getByLabelText('Heat 2')).toBeInTheDocument()
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
    'Ember Idol, relic of Ana: Creatures you control get +1/+0.',
    'Wildfire Pact, risky, relic of Ana: All red spells cost {2} less.',
  ])
  expect(relics[1]).toHaveAttribute('data-risky', 'true')
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
