// Goal: an event says plainly what each choice does before it is made: its cost and its reward side by side as chips (gold, life, life cap, a relic with its rarity, a card, a loss, a commander bane), a free choice reads as free, who it affects, a gamble's odds, why a choice the hero cannot take is shut (gold, life, life cap, a relic, a commander), and a choice with no effect says so; pressing a choice sends its index.
import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { CommanderFacts } from '@/protocol/generated'
import type { HeroFacts } from './eventEffects'
import { CONTENT } from './fakeDelves'
import { EventScene } from './EventScene'

const [toll, bargain] = CONTENT.events

function scene(gold: number, life = 40, busy = false) {
  return sceneOf(toll, { gold, life }, busy)
}

function sceneOf(event: (typeof CONTENT.events)[number] | undefined, hero: HeroFacts, busy = false) {
  const pick = vi.fn<(option: number) => void>()
  render(<EventScene event={event} hero={hero} busy={busy} onPick={pick} />)
  return pick
}

const choice = (name: string) => screen.getByRole('button', { name })
const side = (name: string, which: 'cost' | 'reward') => {
  const el = choice(name).querySelector(`[data-side="${which}"]`)
  if (!(el instanceof HTMLElement)) throw new Error(`no ${which} side on ${name}`)
  return el
}
const chipsOf = (el: HTMLElement) => [...el.querySelectorAll('[data-chip]')].map((c) => c.textContent)

const ruric: CommanderFacts = {
  name: 'Ruric',
  creature: true,
  planeswalker: false,
  power: 4,
  generic: 2,
  keywords: [],
}

describe('an event before the choice', () => {
  it('keeps the story and says who a choice affects', () => {
    scene(100)
    expect(screen.getByText('A goblin blocks a bridge.')).toBeInTheDocument()
    expect(
      screen.getByText(/Each hero chooses for themselves: the effects are yours alone/),
    ).toBeInTheDocument()
  })

  it('shows a payment on the cost side and its reward on the reward side', () => {
    scene(100)
    expect(chipsOf(side('Pay the toll', 'cost'))).toEqual(['−25 gold'])
    expect(chipsOf(side('Pay the toll', 'reward'))).toEqual(['a card for your Satchel'])
    expect(side('Pay the toll', 'cost').querySelector('[data-chip]')).toHaveAttribute('data-tone', 'cost')
    expect(side('Pay the toll', 'reward').querySelector('[data-chip]')).toHaveAttribute('data-tone', 'gain')
  })

  it('shows a life cost with the gold it buys', () => {
    scene(0)
    expect(chipsOf(side('Push past', 'cost'))).toEqual(['−8 life'])
    expect(chipsOf(side('Push past', 'reward'))).toEqual(['+40 gold'])
  })

  it('reads a choice without a cost as free and keeps a gamble on the reward side', () => {
    scene(0)
    expect(within(side('Reach into the sack', 'cost')).getByText('Free')).toBeInTheDocument()
    const chance = side('Reach into the sack', 'reward').querySelector('[data-chance]')
    expect(chance?.textContent).toBe('50%a relic50%−10 life')
  })

  it('says a choice with no effect does nothing', () => {
    scene(0)
    expect(within(choice('Turn back')).getByText('Nothing happens.')).toBeInTheDocument()
    expect(choice('Turn back').querySelector('[data-side]')).toBeNull()
  })

  it('describes each choice to a screen reader with the server’s cost and reward', () => {
    scene(100)
    expect(choice('Pay the toll')).toHaveAccessibleDescription(
      'Cost: 25 gold. Reward: a card for your Satchel',
    )
    expect(choice('Reach into the sack')).toHaveAccessibleDescription(
      'Cost: nothing. Reward: 50%: a relic · 50%: −10 life',
    )
    expect(choice('Turn back')).toHaveAccessibleDescription('Nothing happens')
  })

  it('sends the index of the choice pressed', () => {
    const pick = scene(100)
    fireEvent.click(choice('Push past'))
    expect(pick).toHaveBeenCalledWith(1)
  })
})

describe('a bargain', () => {
  const able: HeroFacts = { gold: 0, life: 40, maxLife: 40, relics: 2, commander: ruric }

  it.each([
    ['Bleed for it', ['−5 max life for the run'], ['a Mythic relic']],
    ['Burn a card', ['give up a card of your deck or Satchel'], ['a Rare relic']],
    ['Trade a relic', ['give up a relic'], ['+80 gold']],
    ['Weaken your champion', ['Ruric gets −1/−1'], ['a Common relic']],
    ['Tax your champion', ['Ruric costs {1} more to cast'], ['+30 gold']],
  ])('%s costs %j for %j', (name, cost, reward) => {
    sceneOf(bargain, able)
    expect(chipsOf(side(name, 'cost'))).toEqual(cost)
    expect(chipsOf(side(name, 'reward'))).toEqual(reward)
    expect(choice(name)).toBeEnabled()
  })

  it.each([
    ['Common', 'text-muted-foreground'],
    ['Rare', 'text-relic'],
    ['Mythic', 'text-delve'],
  ])('writes a %s relic’s rarity in its colour', (rarity, colour) => {
    sceneOf(bargain, able)
    const word = screen.getByText(rarity, { selector: `[data-chip] [data-rarity="${rarity}"]` })
    expect(word).toHaveClass(colour)
  })

  it('draws the cut, the losses and the bane as costs with their own pictures', () => {
    sceneOf(bargain, able)
    const kinds = ['Bleed for it', 'Burn a card', 'Trade a relic', 'Weaken your champion'].map((name) =>
      side(name, 'cost').querySelector('[data-chip]')?.getAttribute('data-chip'),
    )
    expect(kinds).toEqual(['cap', 'card', 'relic', 'bane'])
  })

  it.each<[string, HeroFacts, string]>([
    ['Bleed for it', { ...able, maxLife: 24 }, 'Needs a max life of 25: yours is 24'],
    ['Trade a relic', { ...able, relics: 0 }, 'Needs a relic to give up: you have none'],
    ['Tax your champion', { ...able, commander: null }, 'Needs a commander: your deck has none'],
    [
      'Weaken your champion',
      { ...able, commander: { ...ruric, creature: false } },
      'Needs a creature commander: Ruric is not a creature',
    ],
  ])('shuts %s for %j: %s', (name, hero, reason) => {
    const pick = sceneOf(bargain, hero)
    const shut = choice(name)
    expect(shut).toBeDisabled()
    expect(within(shut).getByText(reason)).toBeInTheDocument()
    expect(shut).toHaveAccessibleDescription(new RegExp(reason.replace(/[{}]/g, '\\$&')))
    fireEvent.click(shut)
    expect(pick).not.toHaveBeenCalled()
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
