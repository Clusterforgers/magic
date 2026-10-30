// Goal: on the table, an attachment another seat controls is tucked behind its host with a dashed edge and its controller's name (the edge alone, and the name for a screen reader, at the compact size) and one the host's controller holds is not marked; an Aura or Curse on a player puts a chip with a count when several on that player's pod, one-line pod and tile, a button that can be reached with the keyboard in the pods and an image named in words in the tile; and an Equipment or Fortification on no host says "Unattached" in text, or in its name at the compact size.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { PlayState } from '@/play/store'
import { indexById } from '@/play/view'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import { withEquip } from './dev/equip'
import { Card } from './Card'
import { SeatPanel } from './SeatPanel'
import { SeatTile } from './SeatTile'
import { CardRow } from './CardRow'
import { isUnattached } from './hosts'
import type { ObjectView } from '@/protocol/generated'

const fixture = () => withEquip(buildFixture({ seats: 2 }))

/** The scene's object of that name that is on the battlefield, attached or not. */
const named = (view: { objects: ObjectView[] }, name: string): ObjectView =>
  must(view.objects.find((object) => object.name === name && object.zone === 'Battlefield'))

/** The scene's object of that name that hangs on a permanent (the fixture has cards of the same names). */
const hung = (view: { objects: ObjectView[] }, name: string): ObjectView =>
  must(view.objects.find((object) => object.name === name && object.attached_to !== undefined))

describe('a tucked attachment', () => {
  const tucked = (container: HTMLElement, id: number): HTMLElement =>
    must(container.querySelector<HTMLElement>(`[data-tucked="${id}"]`), `tucked ${id}`)

  /** A row of one host with its attachments tucked behind it at a 100 px box. */
  const rowOf = (scene: ReturnType<typeof fixture>, size: 'normal' | 'compact' = 'normal') => {
    const pacifism = hung(scene.view, 'Pacifism')
    const umbra = hung(scene.view, 'Eel Umbra')
    const hosts = [pacifism, umbra, hung(scene.view, 'Bonesplitter')].map((a) => must(a.attached_to))
    const entries = hosts.map((id) => ({
      ids: [id],
      open: false,
      held: false,
      tucked: scene.view.objects.filter((o) => o.attached_to === id).map((o) => o.id),
    }))
    return {
      ui: (
        <CardRow
          ids={hosts}
          stacks={entries}
          label="creatures"
          size={size}
          box={{ width: 100, height: 140 }}
          tuck={{ each: 20, lift: 80 }}
        />
      ),
    }
  }

  it('carries a dashed edge and its controller when another seat controls it, and nothing when the host does', () => {
    const scene = fixture()
    const { container } = renderTable({ fixture: scene, ui: rowOf(scene).ui })
    const rival = must(scene.view.players.find((player) => player.seat !== scene.view.viewer)).seat
    const mine = tucked(container, hung(scene.view, 'Bonesplitter').id)
    expect(mine).not.toHaveAttribute('data-foreign')
    expect(mine.querySelector('[data-slot="tuck-foreign"]')).toBeNull()
    const theirs = tucked(container, hung(scene.view, 'Pacifism').id)
    expect(theirs).toHaveAttribute('data-foreign', String(rival))
    expect(theirs.querySelector('[data-slot="tuck-foreign"]')).toHaveClass(
      'border-dashed',
      'border-border-strong',
    )
    expect(within(theirs).getByText(/Controlled by/)).toBeInTheDocument()
    expect(theirs.querySelector('[data-slot="tuck-owner"]')?.textContent).toMatch(/’s$/)
  })

  it('marks your own Aura on an opponent’s creature the same way', () => {
    const scene = fixture()
    const { container } = renderTable({ fixture: scene, ui: rowOf(scene).ui })
    const umbra = hung(scene.view, 'Eel Umbra')
    expect(tucked(container, umbra.id)).toHaveAttribute('data-foreign', String(scene.view.viewer))
  })

  it('keeps the edge and the spoken controller and drops the written name at the compact size', () => {
    const scene = fixture()
    const { container } = renderTable({ fixture: scene, ui: rowOf(scene, 'compact').ui })
    const theirs = tucked(container, hung(scene.view, 'Pacifism').id)
    expect(theirs.querySelector('[data-slot="tuck-foreign"]')).not.toBeNull()
    expect(theirs.querySelector('[data-slot="tuck-owner"]')).toBeNull()
    expect(within(theirs).getByText(/Controlled by/)).toHaveClass('sr-only')
  })
})

describe('the chips of an enchanted player', () => {
  const rival = (scene: ReturnType<typeof fixture>) =>
    must(scene.view.players.find((player) => player.seat !== scene.view.viewer)).seat

  /** The scene with `extra` more copies of its Curse. */
  const withCurses = (extra: number) => (first: PlayState) => {
    const view = clone(must(first.view))
    const curse = named(view, 'Curse of Opulence')
    const top = Math.max(...view.objects.map((object) => object.id))
    for (let at = 1; at <= extra; at += 1) view.objects.push({ ...curse, id: top + at })
    return { ...first, view, objectsById: indexById(view.objects) }
  }

  it.each(['panel', 'line'] as const)(
    'is a button in the %s that previews the Aura and takes the keyboard',
    (variant) => {
      const scene = fixture()
      const seat = rival(scene)
      renderTable({ fixture: scene, ui: <SeatPanel seat={seat} variant={variant} /> })
      const chip = screen.getByRole('button', { name: /Curse of Opulence .* enchants/ })
      expect(chip.tabIndex).toBe(0)
      expect(chip).toHaveTextContent('Curse of Opulence')
      expect(chip).toHaveAttribute('data-count', '1')
    },
  )

  it('says how many when several of one name are attached', () => {
    const scene = fixture()
    renderTable({
      fixture: scene,
      state: withCurses(2),
      ui: <SeatPanel seat={rival(scene)} variant="panel" />,
    })
    const chip = screen.getByRole('button', { name: /^3 × Curse of Opulence/ })
    expect(chip).toHaveTextContent('Curse of Opulence ×3')
    expect(screen.getAllByRole('button', { name: /enchants/ })).toHaveLength(1)
  })

  it('draws nothing in the pod of a player nothing is attached to', () => {
    const scene = fixture()
    renderTable({ fixture: scene, ui: <SeatPanel seat={scene.view.viewer} variant="panel" /> })
    expect(screen.queryByRole('button', { name: /enchants/ })).toBeNull()
  })

  it('is an image inside the tile, which is a button itself, and the tile’s name has the words', () => {
    const scene = fixture()
    const seat = rival(scene)
    const { container } = renderTable({
      fixture: scene,
      state: withCurses(1),
      ui: <SeatTile seat={seat} focused={false} onFocusSeat={() => undefined} />,
    })
    const tile = must(container.querySelector<HTMLElement>(`[data-tile-seat="${seat}"]`))
    expect(within(tile).queryByRole('button')).toBeNull()
    const chip = within(tile).getByRole('img', { name: /^2 × Curse of Opulence/ })
    expect(chip).toHaveTextContent('×2')
    expect(tile).toHaveAccessibleName(expect.stringContaining('2 × Curse of Opulence'))
  })
})

describe('an Equipment on no host', () => {
  it('says "Unattached" at its foot, and an attached one does not', () => {
    const scene = fixture()
    const sword = must(scene.view.objects.filter((o) => o.name === 'Sword of Fire and Ice').at(-1))
    const bonesplitter = hung(scene.view, 'Bonesplitter')
    renderTable({
      fixture: scene,
      ui: (
        <>
          <Card id={sword.id} size="normal" />
          <Card id={bonesplitter.id} size="normal" />
        </>
      ),
    })
    const face = must(document.querySelector<HTMLElement>(`button[data-object-id="${sword.id}"]`))
    expect(within(face).getByText('Unattached')).toBeVisible()
    expect(face).toHaveAccessibleName(expect.stringContaining('unattached'))
    const other = must(document.querySelector<HTMLElement>(`button[data-object-id="${bonesplitter.id}"]`))
    expect(within(other).queryByText('Unattached')).toBeNull()
    expect(other).not.toHaveAccessibleName(expect.stringContaining('unattached'))
  })

  it('says it in the name alone at the compact size', () => {
    const scene = fixture()
    const sword = must(scene.view.objects.filter((o) => o.name === 'Sword of Fire and Ice').at(-1))
    renderTable({ fixture: scene, ui: <Card id={sword.id} size="compact" /> })
    const chip = must(document.querySelector<HTMLElement>('[data-slot="card-unattached"]'))
    expect(chip.querySelector('.sr-only')).toHaveTextContent('Unattached')
  })

  it.each([
    ['an Equipment on no host', { subtypes: ['Equipment'] }, true],
    ['a Fortification on no host', { subtypes: ['Fortification'] }, true],
    ['an Equipment on a creature', { subtypes: ['Equipment'], attached_to: 3 }, false],
    ['an Equipment in a graveyard', { subtypes: ['Equipment'], zone: { Graveyard: 0 } }, false],
    ['an Aura on no host', { subtypes: ['Aura'] }, false],
    ['a creature', { subtypes: ['Bear'] }, false],
  ] as const)('%s is unattached: %s', (_name, over, unattached) => {
    const base = must(fixture().view.objects[0])
    expect(isUnattached({ ...base, zone: 'Battlefield', ...over } as unknown as ObjectView)).toBe(unattached)
  })
})
