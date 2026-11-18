// Goal: a cleared floor offers Bank the win or Descend as a party vote that sends the choice, marks the player's own vote, lists every ballot and says what a descent changes; the floor shows in the run header, the ladder and the intro, and the run summary counts the floors cleared.
import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { DelveAction, DelveFightView, DelveView, Run } from '@/protocol/generated'
import { BossIntro } from './BossIntro'
import { BossLadder } from './BossLadder'
import { DescendPanel } from './DescendPanel'
import { CONTENT, delveView, hero } from './fakeDelves'
import { floorBallots, nextStep, runStage } from './model'
import { RunHeader } from './RunHeader'

const ana = hero()
const bo = hero({ name: 'Bo', user: 'u1' })
const bot = hero({ name: 'Bot', user: null, bot: 'Normal' })
const DESCENT = [
  'Floor 2: four new bosses',
  'The party keeps its life, relics, Satchels and upgrades, and has 3 hearts',
]

function cleared(floorVotes: Run['floor_votes'] = {}, over: Partial<Run> = {}): DelveView {
  return delveView(
    { descent: DESCENT },
    {
      status: 'FloorCleared',
      at: 3,
      cleared: [0, 1, 3],
      floors_cleared: 1,
      floor_votes: floorVotes,
      heroes: [ana, bo, bot],
      ...over,
    },
  )
}

function panel(view: DelveView) {
  const act = vi.fn<(action: DelveAction) => void>()
  render(<DescendPanel view={view} act={act} busy={false} />)
  return act
}

describe('the floor choice', () => {
  it('offers Bank or Descend and sends the vote pressed', () => {
    const act = panel(cleared())
    expect(screen.getByRole('heading', { name: 'Floor 1 cleared' })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /Descend to floor 2/ }))
    expect(act).toHaveBeenCalledWith({ kind: 'floor_vote', descend: true })
    fireEvent.click(screen.getByRole('button', { name: /Bank the win/ }))
    expect(act).toHaveBeenLastCalledWith({ kind: 'floor_vote', descend: false })
    expect(screen.getByRole('button', { name: /Bank the win/ })).toHaveTextContent('after 1 floor.')
  })

  it('says what descending changes in plain words', () => {
    panel(cleared())
    const changes = screen.getByRole('heading', { name: 'What descending changes' }).parentElement!
    for (const line of DESCENT) expect(changes).toHaveTextContent(line)
  })

  it('marks the player’s vote and lists every ballot', () => {
    panel(cleared({ 0: true, 1: false }))
    expect(screen.getByRole('button', { name: /Descend to floor 2/ })).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('button', { name: /Bank the win/ })).toHaveAttribute('aria-pressed', 'false')
    const items = within(screen.getByRole('list', { name: 'Floor votes' })).getAllByRole('listitem')
    expect(items.map((li) => li.getAttribute('data-ballot'))).toEqual(['descend', 'bank', 'follows'])
    expect(items[0]).toHaveTextContent('Ana (you)')
    expect(items[1]).toHaveTextContent('bank the win')
  })

  it('shows nothing while the floor is not cleared', () => {
    const { container } = render(
      <DescendPanel view={delveView({}, { status: 'Map' })} act={vi.fn()} busy={false} />,
    )
    expect(container).toBeEmptyDOMElement()
  })

  it('words the stage and the next step', () => {
    const view = cleared()
    expect(runStage(view)).toBe('Floor 1 cleared')
    expect(nextStep(view)).toContain('Bank the win')
    expect(floorBallots(view.run).map((b) => b.state)).toEqual(['pending', 'pending', 'follows'])
  })
})

describe('the floor on the page', () => {
  it('shows the floor in the run header and on the ladder', () => {
    const view = delveView({}, { status: 'Map', at: 0, cleared: [0], floor: 2, floors_cleared: 1 })
    render(
      <>
        <RunHeader view={view} content={CONTENT} />
        <BossLadder view={view} content={CONTENT} />
      </>,
    )
    expect(within(screen.getByRole('banner')).getByText('Floor 2')).toBeInTheDocument()
    expect(
      within(screen.getByRole('region', { name: 'The descent' })).getByText('Floor 2'),
    ).toBeInTheDocument()
  })

  it('lists Intensify in the boss intro on a deeper floor', () => {
    const fight: DelveFightView = {
      boss: 'Skrix',
      life: 60,
      rules: [],
      phases: [],
      minions: [],
      intensify: ['Intensify: the Enrage begins on the heroes’ turn 6 and the passive acts once more'],
    }
    const view = delveView(
      { boss: 'skrix_nest', fight },
      { status: { Fight: { game: null } }, at: 0, floor: 2 },
    )
    render(<BossIntro view={view} content={CONTENT} />)
    const intensify = screen.getByRole('heading', { name: 'Floor 2: Intensified' }).parentElement!
    expect(intensify).toHaveTextContent('the passive acts once more')
  })
})
