// Goal: an event says plainly what each choice does before it is made: effects as chips (gold, life, relic, card), who it affects, a gamble's odds, why a choice the hero cannot pay is shut, and a choice with no effect says so; pressing a choice sends its index.
import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { CONTENT } from './fakeDelves'
import { EventScene } from './EventScene'

const toll = CONTENT.events[0]

function scene(gold: number, life = 40, busy = false) {
  const pick = vi.fn<(option: number) => void>()
  render(<EventScene event={toll} gold={gold} life={life} busy={busy} onPick={pick} />)
  return pick
}

const choice = (name: string) => screen.getByRole('button', { name })

describe('an event before the choice', () => {
  it('keeps the story and says who a choice affects', () => {
    scene(100)
    expect(screen.getByText('A goblin blocks a bridge.')).toBeInTheDocument()
    expect(
      screen.getByText(/Each hero chooses for themselves: the effects are yours alone/),
    ).toBeInTheDocument()
  })

  it('shows a payment and its reward as a cost and a gain', () => {
    scene(100)
    const chips = [...choice('Pay the toll').querySelectorAll('[data-chip]')]
    expect(chips.map((c) => c.textContent)).toEqual(['−25 gold', 'a card for your deck'])
    expect(chips.map((c) => c.getAttribute('data-tone'))).toEqual(['cost', 'gain'])
  })

  it('shows a life cost with the gold it buys', () => {
    scene(0)
    const text = choice('Push past').textContent
    expect(text).toContain('−8 life')
    expect(text).toContain('+40 gold')
  })

  it('shows a gamble with the odds of each way it can go', () => {
    scene(0)
    const chance = choice('Reach into the sack').querySelector('[data-chance]')
    expect(chance).not.toBeNull()
    expect(chance?.textContent).toBe('50%a relic50%−10 life')
  })

  it('says a choice with no effect does nothing', () => {
    scene(0)
    expect(within(choice('Turn back')).getByText('Nothing happens.')).toBeInTheDocument()
  })

  it('describes each choice to a screen reader with the server’s words', () => {
    scene(100)
    expect(choice('Pay the toll')).toHaveAccessibleDescription('−25 gold, a card for your deck')
    expect(choice('Reach into the sack')).toHaveAccessibleDescription('50%: a relic · 50%: −10 life')
    expect(choice('Turn back')).toHaveAccessibleDescription('Nothing happens')
  })

  it('sends the index of the choice pressed', () => {
    const pick = scene(100)
    fireEvent.click(choice('Push past'))
    expect(pick).toHaveBeenCalledWith(1)
  })
})

describe('a choice the hero cannot take', () => {
  it('is shut with the gold it needs and the gold the hero has', () => {
    const pick = scene(18)
    const pay = choice('Pay the toll')
    expect(pay).toBeDisabled()
    expect(within(pay).getByText('Needs 25 gold: you have 18')).toBeInTheDocument()
    expect(pay).toHaveAccessibleDescription(/Needs 25 gold: you have 18/)
    fireEvent.click(pay)
    expect(pick).not.toHaveBeenCalled()
    expect(choice('Turn back')).toBeEnabled()
  })

  it('is shut with the life it needs to stay standing', () => {
    scene(0, 8)
    const push = choice('Push past')
    expect(push).toBeDisabled()
    expect(within(push).getByText('Needs more than 8 life: you have 8')).toBeInTheDocument()
    expect(choice('Reach into the sack')).toBeEnabled()
  })

  it('opens when the hero has exactly the gold', () => {
    scene(25)
    expect(choice('Pay the toll')).toBeEnabled()
  })

  it('shuts every choice while an answer is on its way', () => {
    scene(100, 40, true)
    for (const name of ['Pay the toll', 'Push past', 'Reach into the sack', 'Turn back'])
      expect(choice(name)).toBeDisabled()
  })
})
