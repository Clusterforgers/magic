// Goal: the coach shows the lesson's step with its words and lit part, moves on with Next and with the game, keeps the step across a reload, shows a held input's reason, says when it lost its place, sets the session's gate to the step's input, and ends with Next lesson and Back to lessons.
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPlayStore } from '@/play/store'
import { buildFixture } from '@/features/table/dev/fixture'
import { fixtureSession, fixtureState } from '@/features/table/dev/devGame'
import { Coach } from './Coach'
import { lessonSession } from './lessonSession'
import type { Lesson } from './step'

const LESSON: Lesson = {
  id: 'seat',
  title: 'Your seat',
  text: '',
  minutes: 1,
  steps: [
    { id: 'one', kind: 'read', title: 'First', text: 'Read this first.' },
    {
      id: 'two',
      kind: 'do',
      title: 'Second',
      text: 'Press {@pass.primary} now.',
      presses: ['pass.primary'],
      allow: { kind: 'pass' },
      done: (facts) => facts.turn >= 99,
    },
    { id: 'three', kind: 'read', title: 'Third', text: 'The end.' },
  ],
}

function setup(game = 'g1') {
  const store = createPlayStore()
  store.setState(fixtureState(buildFixture({ seats: 2 })))
  const session = lessonSession(fixtureSession(store), () => Date.now())
  const props = {
    lesson: LESSON,
    game,
    session,
    onRestart: vi.fn(),
    onLeave: vi.fn(),
    onFinish: vi.fn(),
    onNextLesson: vi.fn(),
    nextTitle: 'Mana and spells',
  }
  const view = render(<Coach {...props} />)
  return { store, session, props, ...view }
}

const root = (): HTMLElement => document.querySelector<HTMLElement>('[data-slot="coach"]') ?? document.body

describe('Coach', () => {
  beforeEach(() => window.sessionStorage.clear())
  afterEach(() => vi.useRealTimers())

  it('shows a read step and moves on with Next, setting the gate to the step on show', async () => {
    const { session } = setup()
    expect(root()).toHaveAttribute('data-coach-step', 'one')
    expect(screen.getByText('Read this first.')).toBeInTheDocument()
    expect(session.coach.getState().gate?.allow).toBeUndefined()
    await userEvent.click(screen.getByRole('button', { name: 'Next' }))
    expect(root()).toHaveAttribute('data-coach-step', 'two')
    expect(root()).toHaveAttribute('data-coach-kind', 'do')
    expect(session.coach.getState().gate?.allow).toEqual({ kind: 'pass' })
    expect(screen.getByText('Your move')).toBeInTheDocument()
  })

  it('keeps its step across a reload of the game', async () => {
    const first = setup('g2')
    await userEvent.click(screen.getByRole('button', { name: 'Next' }))
    first.unmount()
    setup('g2')
    expect(root()).toHaveAttribute('data-coach-step', 'two')
  })

  it('shows the reason of a held input', async () => {
    const { session } = setup()
    await userEvent.click(screen.getByRole('button', { name: 'Next' }))
    act(() => {
      session.concede()
    })
    expect(await screen.findByText('Not yet: pass first.')).toBeInTheDocument()
    expect(document.querySelector('[data-slot="coach-card"]')).toHaveAttribute('data-shake')
  })

  it('says it lost its place when a do step cannot find its part for 3 s', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    setup()
    await act(async () => {
      screen.getByRole('button', { name: 'Next' }).click()
    })
    // The fixture table is not drawn here, so the primary button is nowhere.
    await act(async () => {
      vi.advanceTimersByTime(3500)
    })
    expect(root()).toHaveAttribute('data-coach-step', 'lost')
    expect(screen.getByRole('button', { name: 'Restart lesson' })).toBeInTheDocument()
  })

  it('ends with Next lesson and Back to lessons and marks the lesson finished', async () => {
    window.sessionStorage.setItem('lesson-step:g3', '2')
    const { props } = setup('g3')
    await userEvent.click(screen.getByRole('button', { name: 'Finish' }))
    expect(root()).toHaveAttribute('data-coach-step', 'end')
    expect(props.onFinish).toHaveBeenCalled()
    expect(screen.getByText(/Next up: Mana and spells/)).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Next lesson' }))
    expect(props.onNextLesson).toHaveBeenCalled()
    await userEvent.click(screen.getByRole('button', { name: 'Back to lessons' }))
    expect(props.onLeave).toHaveBeenCalled()
  })
})
