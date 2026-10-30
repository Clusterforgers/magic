// Goal: a host carries one mark per attachment saying what it gives, in the tuck order, capped with "+N" past the room of its foot stack and as a count with an icon when compact; its spoken name lists each attachment with what it gives; and its preview lists them while an attachment's own preview says what it hangs on and what it gives.
import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { attachesOf } from './attachMarks'
import { Card } from './Card'
import { buildFixture } from './dev/fixture'
import { EQUIP_STACK, withEquip } from './dev/equip'
import { GrantMarks } from './GrantMarks'
import { Overlay } from './CardOverlay'

const scene = () => withEquip(buildFixture({ seats: 2 }))
type Scene = ReturnType<typeof scene>

/** The attachment of that name hanging on a permanent. */
const hung = (view: Scene['view'], name: string): ObjectView =>
  must(view.objects.find((object) => object.name === name && object.attached_to !== undefined))

/** The host of the five-attachment stack, with its attachments read as the table reads them. */
function stackOf(fixture: Scene) {
  const host = must(fixture.view.objects.find((o) => o.id === hung(fixture.view, EQUIP_STACK[0]).attached_to))
  const attachments = fixture.view.objects.filter((o) => o.attached_to === host.id)
  return { host, attaches: attachesOf(host, attachments) }
}

const texts = (root: HTMLElement, slot = 'card-grant'): (string | null)[] =>
  [...root.querySelectorAll(`[data-slot="${slot}"]`)].map((node) => node.textContent)

const FIVE = ['+3/+3 First strike', '+2/+0 Trample', '+1/+1 Umbra Reach', '+2/+0', 'Haste Shroud']

describe('the grant marks', () => {
  it('has one mark per attachment in the tuck order, toned by what it does', () => {
    const { attaches } = stackOf(scene())
    const { container } = render(
      <GrantMarks attaches={attaches} compact={false} room={9} span={400} text="" />,
    )
    expect(texts(container)).toEqual(FIVE)
    expect(
      [...container.querySelectorAll('[data-slot="card-grant"]')].map((m) => m.getAttribute('data-tone')),
    ).toEqual(['raise', 'raise', 'raise', 'raise', 'plain'])
  })

  it('reads top to bottom as the tucked cards do when the stack grows upward', () => {
    const { attaches } = stackOf(scene())
    const { container } = render(
      <GrantMarks attaches={attaches} compact={false} room={9} span={400} up text="" />,
    )
    expect(texts(container)).toEqual([...FIVE].reverse())
  })

  it('sums the attachments past the room into one mark, which names them in its title', () => {
    const { attaches } = stackOf(scene())
    const { container } = render(
      <GrantMarks attaches={attaches} compact={false} room={3} span={400} text="" />,
    )
    expect(texts(container)).toEqual(FIVE.slice(0, 1))
    expect(texts(container, 'card-grants-more')).toEqual(['+5/+1 Umbra Trample Reach Haste Shroud'])
    const rest = must(container.querySelector<HTMLElement>('[data-slot="card-grants-more"]'))
    expect(rest.title).toBe(
      '4 more attachments: +5/+1, umbra armor, so it is not destroyed, trample, reach, haste and shroud',
    )
  })

  it('writes a mark in whole words over two lines where its width does not hold them in one', () => {
    const { attaches } = stackOf(scene())
    const { container } = render(
      <GrantMarks attaches={attaches} compact={false} room={9} span={75} text="" />,
    )
    const first = must(container.querySelector<HTMLElement>('[data-slot="card-grant"]'))
    expect([...first.querySelectorAll('.truncate')].map((line) => line.textContent)).toEqual([
      '+3/+3 First',
      'strike',
    ])
    expect(first).toHaveTextContent('+3/+3 First strike')
  })

  it.each([
    ['a compact face', true, 9],
    ['a face with no room for two lines', false, 1],
  ])('shows the count and one icon on %s', (_name, compact, room) => {
    const { attaches } = stackOf(scene())
    const { container } = render(
      <GrantMarks attaches={attaches} compact={compact} room={room} span={50} text="" />,
    )
    expect(container.querySelectorAll('[data-slot="card-grant"]')).toHaveLength(0)
    const count = must(container.querySelector<HTMLElement>('[data-slot="card-grants"]'))
    expect(count).toHaveTextContent('5')
    expect(count.querySelector('svg')).not.toBeNull()
    expect(count).toHaveAttribute('data-tone', 'raise')
  })

  it('shows a restriction in the colour of one and the icon of a ban', () => {
    const fixture = scene()
    const host = must(fixture.view.objects.find((o) => o.id === hung(fixture.view, 'Pacifism').attached_to))
    const attaches = attachesOf(host, [hung(fixture.view, 'Pacifism')])
    const { container } = render(<GrantMarks attaches={attaches} compact room={1} span={50} text="" />)
    expect(must(container.querySelector('[data-slot="card-grants"]'))).toHaveAttribute(
      'data-tone',
      'restrict',
    )
  })

  it('names an attachment that gives nothing and dashes one another seat controls', () => {
    const fixture = scene()
    const pacifism = hung(fixture.view, 'Pacifism')
    const host = must(fixture.view.objects.find((o) => o.id === pacifism.attached_to))
    const attaches = attachesOf(host, [pacifism, { ...hung(fixture.view, 'Short Sword'), gives: undefined }])
    const { container } = render(
      <GrantMarks attaches={attaches} compact={false} room={9} span={400} text="" />,
    )
    const [first, second] = [...container.querySelectorAll('[data-slot="card-grant"]')]
    expect(first).toHaveTextContent("Can't attack or block")
    expect(first).toHaveAttribute('data-foreign', 'true')
    expect(first).toHaveAttribute('data-tone', 'restrict')
    expect(second).toHaveTextContent('Short Sword')
    expect(second).not.toHaveAttribute('data-foreign')
  })

  it('draws nothing for a permanent with no attachment', () => {
    const { container } = render(<GrantMarks attaches={[]} compact={false} room={9} span={400} text="" />)
    expect(container).toBeEmptyDOMElement()
  })
})

describe('the foot stack of a picture', () => {
  const foot = (size: 'normal' | 'compact', width: number, turned = false) => {
    const fixture = scene()
    const { host, attaches } = stackOf(fixture)
    const { container } = renderTable({
      fixture,
      ui: (
        <Overlay object={host} role={null} size={size} turned={turned} width={width} links={{ attaches }} />
      ),
    })
    return must(container.querySelector<HTMLElement>('[data-slot="card-marks-foot"]'))
  }

  it('lists the marks that fit and sums the rest in the lines that are left', () => {
    const stack = foot('normal', 96)
    // 96 px wide is 134 px high: three lines, so one mark of one line and the sum of the other four in two.
    expect(texts(stack)).toEqual(['+3/+3 …'])
    expect(texts(stack, 'card-grants-more')).toEqual(['+5/+1 …'])
  })

  it('has room for all five on a large face', () => {
    expect(texts(foot('normal', 240))).toEqual([...FIVE].reverse())
  })

  it('shows the count on a compact face and on a face that lies on its side', () => {
    expect(texts(foot('compact', 60), 'card-grants')).toEqual(['5'])
    expect(texts(foot('normal', 96, true), 'card-grants')).toEqual(['5'])
  })
})

describe('the spoken name of a host', () => {
  const nameOf = (fixture: Scene, attachment: string) => {
    const host = must(hung(fixture.view, attachment).attached_to)
    renderTable({ fixture, ui: <Card id={host} /> })
    return screen.getByRole('button').getAttribute('aria-label') ?? ''
  }

  it('lists each attachment in the tuck order with what it gives', () => {
    const name = nameOf(scene(), 'Rancor')
    const order = [
      'Ethereal Armor attached giving +3/+3 and first strike',
      'Rancor attached giving +2/+0 and trample',
      'Spider Umbra attached giving +1/+1, umbra armor, so it is not destroyed and reach',
      'Bonesplitter attached giving +2/+0',
      'Lightning Greaves attached giving haste and shroud',
    ]
    const at = order.map((line) => name.indexOf(line))
    expect(at.every((index) => index >= 0)).toBe(true)
    expect(at).toEqual([...at].sort((a, b) => a - b))
  })

  it("names another seat's attachment with its controller", () => {
    expect(nameOf(scene(), 'Pacifism')).toMatch(/\S+'s Pacifism attached giving can't attack or block/)
  })

  it('names an attachment that gives nothing and nothing more', () => {
    const fixture = scene()
    const sword = hung(fixture.view, 'Short Sword')
    const bare = {
      ...fixture,
      view: {
        ...fixture.view,
        objects: fixture.view.objects.map((o) => (o.id === sword.id ? { ...o, gives: undefined } : o)),
      },
    }
    expect(nameOf(bare, 'Short Sword')).toMatch(/Short Sword attached(,|$)/)
  })
})

describe('the preview', () => {
  const previewOf = async (fixture: Scene, id: number) => {
    const { container, user } = renderTable({ fixture })
    await user.hover(must(container.querySelector<HTMLElement>(`button[data-object-id="${id}"]`)))
    return screen.getByTestId('card-preview')
  }

  it('lists each attachment of a host with what it gives and who controls it when foreign', async () => {
    const fixture = scene()
    const preview = await previewOf(fixture, must(hung(fixture.view, 'Rancor').attached_to))
    const items = within(
      must(preview.querySelector<HTMLElement>('[data-slot="preview-attachments"]')),
    ).getAllByRole('listitem')
    expect(items.map((item) => item.textContent)).toEqual([
      'Ethereal Armor: +3/+3, first strike',
      'Rancor: +2/+0, trample',
      'Spider Umbra: +1/+1, umbra armor, so it is not destroyed, reach',
      'Bonesplitter: +2/+0',
      'Lightning Greaves: haste, shroud',
    ])
  })

  it("puts the controller before the name of another seat's attachment", async () => {
    const fixture = scene()
    const preview = await previewOf(fixture, must(hung(fixture.view, 'Pacifism').attached_to))
    const [item] = within(
      must(preview.querySelector<HTMLElement>('[data-slot="preview-attachments"]')),
    ).getAllByRole('listitem')
    expect(item?.textContent).toMatch(/^\S+'s Pacifism: can't attack or block$/)
  })

  it('names the host of an attachment and what it gives it', async () => {
    const fixture = scene()
    const rancor = hung(fixture.view, 'Rancor')
    const host = must(fixture.view.objects.find((o) => o.id === rancor.attached_to))
    const preview = await previewOf(fixture, rancor.id)
    expect(must(preview.querySelector('[data-slot="preview-host"]'))).toHaveTextContent(
      `Attached to ${host.name}, giving +2/+0, trample.`,
    )
    expect(preview.querySelector('[data-slot="preview-attachments"]')).toBeNull()
  })

  it('names the player a Curse enchants and says an Equipment on no host waits', async () => {
    const fixture = scene()
    const named = (name: string) => must(fixture.view.objects.find((o) => o.name === name)).id
    expect(await previewOf(fixture, named('Curse of Opulence'))).toHaveTextContent(/Enchants \S+\./)
  })

  it('says an Equipment on no host is not attached', async () => {
    const fixture = scene()
    const sword = must(fixture.view.objects.find((o) => o.name === 'Sword of Fire and Ice'))
    expect(await previewOf(fixture, sword.id)).toHaveTextContent('Not attached to anything.')
  })
})
