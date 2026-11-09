// Goal: on a compact face narrower than NARROW_PX every mark is an icon or a glyph of at most GLYPH_MAX characters that never shrinks, its full words kept for a screen reader and the tooltip; at NARROW_PX the words come back.
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectView } from '@/protocol/generated'
import { permanent } from '@/test-utils/cards'
import { TextFace } from './CardFace'
import type { CombatRole, Links } from './cardModel'
import { NARROW_PX } from './cardSize'
import { CardWidthContext } from './ImageFace'
import { GLYPH_MAX } from './narrowMarks'

interface Case {
  mark: string
  slot: string
  object: ObjectView
  role?: CombatRole
  links?: Links
  /** What the glyph writes beside its icon, if anything. */
  glyph: string
  /** The words a screen reader and the tooltip get. */
  words: string
}

const CASES: Case[] = [
  {
    mark: 'sick',
    slot: 'card-status',
    object: permanent({ summoning_sick: true }),
    glyph: '',
    words: 'Summoning sick',
  },
  { mark: 'tapped', slot: 'card-status', object: permanent({ tapped: true }), glyph: '', words: 'Tapped' },
  { mark: 'damage', slot: 'card-status', object: permanent({ damage: 3 }), glyph: '3', words: '3 damage' },
  {
    mark: 'attacking',
    slot: 'card-status',
    object: permanent({}),
    role: 'attacking',
    glyph: '',
    words: 'Attacking',
  },
  {
    mark: 'blocked',
    slot: 'card-status',
    object: permanent({}),
    role: 'blocked',
    glyph: '',
    words: 'Blocked',
  },
  {
    mark: 'blocking',
    slot: 'card-status',
    object: permanent({}),
    role: 'blocking',
    glyph: '',
    words: 'Blocking',
  },
  {
    mark: 'phased out',
    slot: 'card-status',
    object: permanent({ phased_out: true }),
    glyph: '',
    words: 'Phased out',
  },
  { mark: 'level', slot: 'card-status', object: permanent({ level: 12 }), glyph: 'L12', words: 'Level 12' },
  {
    mark: '+1/+1 counters',
    slot: 'card-counter',
    object: permanent({}, { counters: [['+1/+1', 2]] }),
    glyph: '+2',
    words: '2 +1/+1 counters',
  },
  {
    mark: '-1/-1 counter',
    slot: 'card-counter',
    object: permanent({}, { counters: [['-1/-1', 1]] }),
    glyph: '−1',
    words: '1 -1/-1 counter',
  },
  {
    mark: 'many +1/+1 counters',
    slot: 'card-counter',
    object: permanent({}, { counters: [['+1/+1', 120]] }),
    glyph: '+99+',
    words: '120 +1/+1 counters',
  },
  {
    mark: 'poison counters',
    slot: 'card-counter',
    object: permanent({}, { counters: [['poison', 3]] }),
    glyph: '☠3',
    words: '3 poison counters',
  },
  {
    mark: 'another counter',
    slot: 'card-counter',
    object: permanent({}, { counters: [[{ Other: 'oil' }, 4]] }),
    glyph: '4',
    words: '4 oil counters',
  },
  {
    mark: 'host',
    slot: 'card-attached-to',
    object: permanent({}),
    links: { host: 'Serra Angel of the Long Name' },
    glyph: '',
    words: 'On Serra Angel of the Long Name',
  },
  {
    mark: 'protector',
    slot: 'card-protector',
    object: permanent({}),
    links: { protector: 'Alice' },
    glyph: '',
    words: 'Protected by Alice',
  },
]

function draw(width: number, { object, role, links }: Case) {
  return render(
    <CardWidthContext.Provider value={width}>
      <TextFace object={object} role={role ?? null} size="compact" links={links} fit />
    </CardWidthContext.Provider>,
  ).container
}

/** What a mark shows, its screen-reader words left out. */
function shown(mark: Element): string {
  return [...mark.children]
    .filter((child) => !child.classList.contains('sr-only') && child.tagName !== 'svg')
    .map((child) => child.textContent ?? '')
    .join('')
}

describe('narrow marks', () => {
  const widths = [44, 60, NARROW_PX - 1]
  it.each(widths.flatMap((width) => CASES.map((item) => [width, item.mark, item] as const)))(
    'at %i px draws %s as a glyph that reads whole',
    (width, _mark, item) => {
      const box = draw(width, item)
      const mark = box.querySelector(`[data-slot="${item.slot}"]`)
      expect(mark).not.toBeNull()
      expect(mark?.hasAttribute('data-glyph')).toBe(true)
      expect(shown(mark as Element)).toBe(item.glyph)
      expect(item.glyph.length).toBeLessThanOrEqual(GLYPH_MAX)
      expect(mark?.className).toContain('shrink-0')
      expect(mark?.className).not.toContain('truncate')
      expect(mark?.getAttribute('title')).toBe(item.words)
      expect(mark?.querySelector('.sr-only')?.textContent).toBe(item.words)
      if (item.glyph === '') expect(mark?.querySelector('svg')).not.toBeNull()
    },
  )

  it.each(CASES.map((item) => [item.mark, item] as const))(
    'writes %s in words at NARROW_PX',
    (_mark, item) => {
      const mark = draw(NARROW_PX, item).querySelector(`[data-slot="${item.slot}"]`)
      expect(mark?.hasAttribute('data-glyph')).toBe(false)
    },
  )
})
