// Goal: a card draws every permanent state in words as well as shape (tapped, summoning sick, damage, power and toughness, attacking, blocked, blocking), keeps its seams for the decision layer, and shows a larger preview on hover and keyboard focus.
import { act, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { CombatView, ObjectView } from '@/protocol/generated'
import { card, forest, permanent } from '@/test-utils/cards'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { Card } from './Card'
import type { CardProps } from './Card'
import { indexById } from '@/play/view'

const ID = 900

interface Shown {
  object: ObjectView
  combat?: Partial<CombatView>
  props?: Partial<CardProps>
}

/** One card on its own, in the store as object 900, with an optional combat. */
function renderCard({ object, combat, props }: Shown) {
  return renderTable({
    ui: <Card id={ID} {...props} />,
    state: (state) => {
      const view = clone(must(state.view))
      view.objects = [...view.objects, { ...object, id: ID }]
      view.combat = { ...view.combat, attackers: [], ...combat }
      return { ...state, view, objectsById: indexById(view.objects) }
    },
  })
}

const face = (container: HTMLElement): HTMLElement =>
  must(container.querySelector<HTMLElement>('[data-slot="card-face"]'))
const button = (): HTMLElement => screen.getByRole('button')

describe('card face', () => {
  it('shows the name, mana cost, type line and power and toughness', () => {
    const { container } = renderCard({ object: card({ name: 'Hill Giant', colors: ['Red'] }) })
    const text = face(container)
    expect(within(text).getByText('Hill Giant')).toBeInTheDocument()
    expect(within(text).getByRole('img', { name: 'Mana cost: 1 generic, green' })).toBeInTheDocument()
    expect(within(text).getByText('Creature — Bear')).toBeInTheDocument()
    expect(within(text).getByText('2/2')).toBeInTheDocument()
    expect(text).toHaveAttribute('data-frame', 'red')
  })

  it('lists the keywords as plain text under the type line, and nothing for a card without any', () => {
    const { container } = renderCard({
      object: card({ name: 'Serra Angel', keywords: ['Flying', 'Vigilance'] }),
    })
    const keywords = must(container.querySelector('[data-slot="card-keywords"]'))
    expect(keywords).toHaveTextContent('Flying, Vigilance')
    expect(button()).toHaveAccessibleName(expect.stringContaining('flying, vigilance'))
  })

  it('has no keyword line for a card with no keywords', () => {
    const { container } = renderCard({ object: card() })
    expect(container.querySelector('[data-slot="card-keywords"]')).toBeNull()
  })

  it('has one obvious slot for card art', () => {
    const { container } = renderCard({ object: card() })
    expect(container.querySelectorAll('[data-slot="card-art"]')).toHaveLength(1)
  })

  it.each([
    [[], 'colorless'],
    [['White'], 'white'],
    [['Blue', 'Black'], 'gold'],
  ] as const)('draws colours %j in a %s frame', (colors, frame) => {
    const { container } = renderCard({ object: card({ colors: [...colors] }) })
    expect(face(container)).toHaveAttribute('data-frame', frame)
  })

  it('leaves out the cost and power of a land and names what it taps for', () => {
    const { container } = renderCard({ object: forest() })
    expect(within(face(container)).queryByRole('img')).toBeNull()
    expect(button()).toHaveAccessibleName('Forest, Basic Land — Forest, Tap: add green mana')
  })

  it('draws a card that is not a permanent too, without any state', () => {
    const { container } = renderCard({ object: card({ zone: { Hand: 0 }, permanent: null }) })
    expect(face(container)).not.toHaveClass('rotate-90')
    expect(within(face(container)).queryByText('Tapped')).toBeNull()
  })

  it('draws nothing for an object that is not in the view', () => {
    const { container } = renderTable({ ui: <Card id={31337} /> })
    expect(container.querySelector('[data-object-id]')).toBeNull()
  })
})

describe('permanent state is drawn as words, not colour alone', () => {
  it('lays a tapped card on its side inside the box of an upright one and says "Tapped"', () => {
    const { container } = renderCard({ object: permanent({ tapped: true }) })
    expect(face(container)).toHaveAttribute('data-turned', 'true')
    expect(face(container)).toHaveClass('h-[calc(var(--card-w)*5/7)]')
    expect(button()).toHaveClass('w-(--card-w)', 'h-(--card-h)')
    expect(button().className).not.toContain('w-(--card-h)')
    expect(within(face(container)).getByText('Tapped')).toBeInTheDocument()
    expect(button()).toHaveAttribute('data-tapped', 'true')
    expect(button()).toHaveAccessibleName(expect.stringContaining('tapped'))
  })

  it('leaves an untapped card upright with no tapped label', () => {
    const { container } = renderCard({ object: permanent({}) })
    expect(face(container)).not.toHaveAttribute('data-turned')
    expect(face(container)).toHaveClass('h-(--card-h)')
    expect(within(face(container)).queryByText('Tapped')).toBeNull()
    expect(button()).not.toHaveAttribute('data-tapped')
  })

  it('says "Sick" for a summoning-sick creature', () => {
    const { container } = renderCard({ object: permanent({ summoning_sick: true }) })
    expect(within(face(container)).getByText('Sick')).toBeInTheDocument()
    expect(button()).toHaveAccessibleName(expect.stringContaining('summoning sick'))
  })

  it('counts the damage on a creature', () => {
    const { container } = renderCard({ object: permanent({ damage: 2 }) })
    expect(within(face(container)).getByText('2 dmg')).toBeInTheDocument()
    expect(button()).toHaveAccessibleName(expect.stringContaining('2 damage'))
  })

  it('shows the current power and toughness the layers give', () => {
    const { container } = renderCard({ object: card({ power: 5, toughness: 1 }) })
    expect(within(face(container)).getByText('5/1')).toBeInTheDocument()
  })

  it('says every state at once when there are several', () => {
    const { container } = renderCard({ object: permanent({ tapped: true, summoning_sick: true, damage: 1 }) })
    for (const label of ['Tapped', 'Sick', '1 dmg'])
      expect(within(face(container)).getByText(label)).toBeInTheDocument()
  })

  it.each([
    [
      'attacking',
      { attackers: [{ object: ID, target: { Player: 1 }, blocked: false, blockers: [] }] },
      'Attacking',
    ],
    [
      'blocked',
      { attackers: [{ object: ID, target: { Player: 1 }, blocked: true, blockers: [5] }] },
      'Blocked',
    ],
    [
      'blocking',
      { attackers: [{ object: 5, target: { Player: 1 }, blocked: true, blockers: [ID] }] },
      'Blocking',
    ],
  ])('says "%s" for a card the combat names', (_role, combat, label) => {
    const { container } = renderCard({ object: permanent({}), combat })
    expect(within(face(container)).getByText(label)).toBeInTheDocument()
  })

  it('says nothing about combat for a card that takes no part', () => {
    const { container } = renderCard({ object: permanent({}) })
    for (const label of ['Attacking', 'Blocked', 'Blocking']) {
      expect(within(face(container)).queryByText(label)).toBeNull()
    }
  })

  it('updates when a batch changes the card, and only then', () => {
    const { container, update } = renderCard({ object: permanent({}) })
    update((state) => {
      const view = clone(must(state.view))
      const changed = must(view.objects.find((object) => object.id === ID))
      must(changed.permanent).tapped = true
      return { view, objectsById: indexById(view.objects) }
    })
    expect(within(face(container)).getByText('Tapped')).toBeInTheDocument()
  })
})

describe('compact cards', () => {
  it('keep the name, power and toughness and state but drop the cost and type line', () => {
    const { container } = renderCard({ object: permanent({ tapped: true }), props: { size: 'compact' } })
    const text = face(container)
    expect(within(text).getByText('Grizzly Bears')).toBeInTheDocument()
    expect(within(text).getByText('2/2')).toBeInTheDocument()
    expect(within(text).getByText('Tapped')).toBeInTheDocument()
    expect(within(text).queryByRole('img')).toBeNull()
    expect(within(text).queryByText('Creature — Bear')).toBeNull()
  })

  it('still say everything to a screen reader', () => {
    renderCard({ object: card(), props: { size: 'compact' } })
    expect(button()).toHaveAccessibleName(
      'Grizzly Bears, cost 1 generic, green, Creature — Bear, power 2, toughness 2',
    )
  })
})

describe('seams for the decision layer', () => {
  it('carries its object id', () => {
    renderCard({ object: card() })
    expect(button()).toHaveAttribute('data-object-id', String(ID))
  })

  it.each([
    ['legal', 'false'],
    ['dimmed', 'false'],
    ['selected', 'true'],
  ] as const)('marks a %s card for the decision layer', (highlight, pressed) => {
    renderCard({ object: card(), props: { highlight } })
    expect(button()).toHaveAttribute('data-highlight', highlight)
    expect(button()).toHaveAttribute('aria-pressed', pressed)
  })

  it('is not a toggle until something highlights it', () => {
    renderCard({ object: card() })
    expect(button()).not.toHaveAttribute('data-highlight')
    expect(button()).not.toHaveAttribute('aria-pressed')
  })

  it('calls onSelect with its id on a click, Enter and Space', async () => {
    const onSelect = vi.fn()
    const { user } = renderCard({ object: card(), props: { onSelect } })
    await user.click(button())
    await user.keyboard('{Enter}')
    await user.keyboard(' ')
    expect(onSelect).toHaveBeenCalledTimes(3)
    expect(onSelect).toHaveBeenCalledWith(ID)
  })

  it('does not need an onSelect', async () => {
    const { user } = renderCard({ object: card() })
    await user.click(button())
    expect(button()).toBeInTheDocument()
  })

  it('leaves the tab order when told it is not the tab stop', () => {
    renderCard({ object: card(), props: { tabbable: false } })
    expect(button()).toHaveAttribute('tabindex', '-1')
  })
})

describe('the preview', () => {
  const firstCard = (container: HTMLElement): HTMLElement =>
    must(container.querySelector<HTMLElement>('[data-battlefield-seat="1"] button[data-object-id]'))

  it('shows the large card with its abilities on hover and hides it when the pointer leaves', async () => {
    const { container, user } = renderTable({ seats: 2 })
    const land = firstCard(container)
    expect(screen.queryByTestId('card-preview')).toBeNull()
    await user.hover(land)
    const preview = screen.getByTestId('card-preview')
    expect(preview).toHaveTextContent('{T}: Add {')
    expect(preview).toHaveTextContent('Basic Land')
    expect(preview).toHaveTextContent('On the battlefield. Controlled by Bo.')
    await user.unhover(land)
    expect(screen.queryByTestId('card-preview')).toBeNull()
  })

  it('shows the preview of a card that takes keyboard focus, and hides it on blur', async () => {
    const { container, user } = renderTable({ seats: 2 })
    const land = firstCard(container)
    act(() => land.focus())
    expect(await screen.findByTestId('card-preview')).toHaveTextContent('Basic Land')
    act(() => land.blur())
    expect(screen.queryByTestId('card-preview')).toBeNull()
    expect(user).toBeDefined()
  })

  it('says what a creature is doing in combat and what state it is in', async () => {
    const { container, user, fixture } = renderTable({ seats: 2 })
    const attacker = must(fixture.view.combat.attackers[0]).object
    await user.hover(must(container.querySelector<HTMLElement>(`button[data-object-id="${attacker}"]`)))
    const preview = screen.getByTestId('card-preview')
    expect(preview).toHaveTextContent('Attacking Alice, blocked')
    expect(preview).toHaveTextContent('Status: tapped, attacking, blocked')
    expect(preview).toHaveTextContent('No abilities.')
    expect(preview.querySelector('[data-slot="preview-keywords"]')).toBeNull()
  })

  it('lists the keywords of a creature in words', async () => {
    const { container, user, fixture } = renderTable({
      seats: 2,
      state: (state) => {
        const view = clone(must(state.view))
        const attacker = must(view.combat.attackers[0]).object
        view.objects = view.objects.map((object) =>
          object.id === attacker ? { ...object, keywords: ['Flying', 'First strike'] } : object,
        )
        return { ...state, view, objectsById: indexById(view.objects) }
      },
    })
    const attacker = must(fixture.view.combat.attackers[0]).object
    await user.hover(must(container.querySelector<HTMLElement>(`button[data-object-id="${attacker}"]`)))
    const preview = screen.getByTestId('card-preview')
    expect(preview.querySelector('[data-slot="preview-keywords"]')).toHaveTextContent(
      'Keywords: Flying, First strike',
    )
  })

  it('docks on the side away from the card', async () => {
    const { container, user } = renderTable({ seats: 2 })
    const land = firstCard(container)
    land.getBoundingClientRect = () => new DOMRect(10, 100, 60, 84)
    await user.hover(land)
    expect(screen.getByTestId('card-preview')).toHaveAttribute('data-dock', 'right')
    await user.unhover(land)
    land.getBoundingClientRect = () => new DOMRect(1300, 100, 60, 84)
    await user.hover(land)
    expect(screen.getByTestId('card-preview')).toHaveAttribute('data-dock', 'left')
  })

  it('pins on a click, stays when the pointer leaves, and closes with its button or Escape', async () => {
    const { container, user } = renderTable({ seats: 2 })
    const land = firstCard(container)
    await user.click(land)
    expect(screen.getByTestId('card-preview')).toHaveAttribute('data-pinned', 'true')
    await user.unhover(land)
    expect(screen.getByTestId('card-preview')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Close preview' }))
    expect(screen.queryByTestId('card-preview')).toBeNull()
    await user.click(land)
    expect(screen.getByTestId('card-preview')).toBeInTheDocument()
    await user.keyboard('{Escape}')
    expect(screen.queryByTestId('card-preview')).toBeNull()
  })

  it('pins on Enter of a card with no act, as it does on a click', async () => {
    const { container, user } = renderTable({ seats: 2 })
    act(() => firstCard(container).focus())
    await user.keyboard('{Enter}')
    expect(screen.getByTestId('card-preview')).toHaveAttribute('data-pinned', 'true')
    await user.keyboard('{Escape}')
    expect(screen.queryByTestId('card-preview')).toBeNull()
  })

  it('closes when the card it shows leaves the game', async () => {
    const { container, user, update } = renderTable({ seats: 2 })
    const land = firstCard(container)
    const id = Number(land.getAttribute('data-object-id'))
    await user.click(land)
    expect(screen.getByTestId('card-preview')).toBeInTheDocument()
    update((state) => {
      const view = clone(must(state.view))
      view.objects = view.objects.filter((object) => object.id !== id)
      view.battlefield = {
        count: view.battlefield.count - 1,
        cards: must(view.battlefield.cards).filter((other) => other !== id),
      }
      return { view, objectsById: indexById(view.objects) }
    })
    expect(screen.queryByTestId('card-preview')).toBeNull()
  })
})
