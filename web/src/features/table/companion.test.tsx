// Goal: a seat's companion shows in its pod by stage (hidden, outside the game, in hand) with the card's name only when the server sent it, nothing for a seat without one, and the priority prompt offers "Bring ... to hand" exactly when the server offers the special action and answers BringCompanion.
import { screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { PlayState } from '@/play/store'
import type { Answer, CompanionView, PriorityOptions } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { bringLabel, companionFacts } from './companion'

const base: PriorityOptions = {
  playable_lands: [],
  castable: [],
  mana_abilities: [],
  can_act: true,
  can_pass: true,
}

function render(companion: CompanionView | undefined, options?: PriorityOptions) {
  const answer = vi.fn((_answer: Answer) => true)
  const table = renderTable({
    session: { answer },
    state: (state: PlayState) => {
      const view = clone(must(state.view))
      const me = must(view.players[state.seat ?? 0])
      me.companion = companion
      return {
        ...state,
        view,
        pending:
          options === undefined
            ? state.pending
            : { id: 4, decision: { player: state.seat ?? 0, kind: { Priority: options } } },
      }
    },
  })
  return { ...table, answer }
}

describe('the words of the slot', () => {
  it.each<[CompanionView, string, string]>([
    [{ stage: 'Hidden' }, 'Companion', "Ann's companion: not revealed yet"],
    [{ stage: 'Revealed', name: 'Lurrus' }, 'Lurrus', "Ann's companion Lurrus: outside the game"],
    [{ stage: 'Brought', name: 'Lurrus', object: 4 }, 'Lurrus', "Ann's companion Lurrus: in hand"],
  ])('%j reads %s', (companion, label, words) => {
    const facts = companionFacts(companion, 'Ann')
    expect([facts.label, facts.words]).toEqual([label, words])
  })

  it.each([
    [undefined, 'Bring companion ({3})'],
    ['Lurrus', 'Bring Lurrus to hand ({3})'],
  ])('names the button for %s', (name, text) => {
    expect(bringLabel(name)).toBe(text)
  })
})

describe('the companion slot', () => {
  it.each<[string, CompanionView]>([
    ['hidden', { stage: 'Hidden' }],
    ['revealed', { stage: 'Revealed', name: 'Lurrus' }],
    ['brought', { stage: 'Brought', name: 'Lurrus', object: 3 }],
  ])('is drawn once for a %s companion', (_label, companion) => {
    render(companion)
    const slot = document.querySelectorAll('[data-slot="companion"]')
    expect(slot).toHaveLength(1)
    expect(slot[0]?.getAttribute('data-stage')).toBe(companion.stage)
    expect(slot[0]?.getAttribute('aria-label')).toContain(companion.name ?? 'companion')
  })

  it('is absent for a seat without a companion', () => {
    render(undefined)
    expect(document.querySelector('[data-slot="companion"]')).toBeNull()
  })
})

describe('the bring action', () => {
  const revealed: CompanionView = { stage: 'Revealed', name: 'Lurrus' }

  it('is not offered when the server does not offer it', () => {
    render(revealed, base)
    expect(screen.queryByRole('button', { name: /Bring/ })).toBeNull()
  })

  it('answers BringCompanion when offered', async () => {
    const { answer, user } = render(revealed, { ...base, companion: true })
    await user.click(screen.getByRole('button', { name: 'Bring Lurrus to hand ({3})' }))
    expect(answer.mock.calls).toEqual([[{ Priority: 'BringCompanion' }]])
  })
})
