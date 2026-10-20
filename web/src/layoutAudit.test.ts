// Goal: the measurements of the layout gate (`e2e/layoutAudit.ts`, used by `e2e/layout.spec.ts` and `scripts/ui-audit.mjs`) find what they are for and nothing else: the rules a page can break and the words for each, the size list and table the script reads and prints, the wait for a layout to settle, and the in-page `measure` over a page whose boxes and styles are given (jsdom has no layout).
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  formatTable,
  measure,
  parseSizes,
  problemsOf,
  SIZES,
  settle,
  sizeName,
  TEXT_FLOOR,
  THEMES,
} from '../e2e/layoutAudit'
import type { Ask, Look } from '../e2e/layoutAudit'

/** A look at a page that keeps every rule, in a 390x844 window. */
function clean(change: Partial<Look> = {}): Look {
  return {
    inner: { width: 390, height: 844 },
    scroll: { width: 390, height: 844, x: 0, y: 0 },
    cutOff: [],
    small: [],
    actions: {},
    ...change,
  }
}

const inside = { box: { left: 10, top: 10, right: 100, bottom: 50 }, free: true }

describe('the rules a page can break', () => {
  it.each<[string, Look, boolean, string[]]>([
    ['a page that keeps them all', clean(), true, []],
    [
      'a page wider than the window',
      clean({ scroll: { width: 420, height: 844, x: 0, y: 0 } }),
      false,
      ['scrolls sideways 420 > 390'],
    ],
    [
      'a page scrolled sideways',
      clean({ scroll: { width: 390, height: 844, x: 12, y: 0 } }),
      false,
      ['was scrolled sideways'],
    ],
    [
      'a page taller than the window, one that must fit',
      clean({ scroll: { width: 390, height: 921, x: 0, y: 0 } }),
      true,
      ['scrolls down 921 > 844'],
    ],
    [
      'a page taller than the window, one that may scroll',
      clean({ scroll: { width: 390, height: 921, x: 0, y: 300 } }),
      false,
      [],
    ],
    [
      'a page that has been scrolled down, one that must fit',
      clean({ scroll: { width: 390, height: 844, x: 0, y: 40 } }),
      true,
      ['was scrolled down'],
    ],
    [
      'a button cut off',
      clean({ cutOff: ['button "Join" 300..420'] }),
      false,
      ['cut off: button "Join" 300..420'],
    ],
    [
      'five boxes cut off',
      clean({ cutOff: ['a 1', 'b 2', 'c 3', 'd 4', 'e 5'] }),
      false,
      ['cut off: a 1', 'cut off: b 2', 'cut off: c 3', 'cut off: 2 more'],
    ],
    [
      'the same small text twice',
      clean({ small: ['9.6px "R"', '9.6px "R"', '10.4px "Enter"'] }),
      false,
      ['small text: 9.6px "R"', 'small text: 10.4px "Enter"'],
    ],
    ['an action that is not there', clean({ actions: { Save: null } }), false, ['Save is missing']],
    [
      'an action beyond the right edge',
      clean({ actions: { Save: { box: { left: 300, top: 10, right: 400, bottom: 50 }, free: true } } }),
      false,
      ['Save is outside the window'],
    ],
    [
      'an action below the window',
      clean({ actions: { Save: { box: { left: 10, top: 820, right: 100, bottom: 860 }, free: true } } }),
      false,
      ['Save is outside the window'],
    ],
    [
      'an action with something over it',
      clean({ actions: { Save: { ...inside, free: false } } }),
      false,
      ['Save is covered'],
    ],
    ['an action in view and free', clean({ actions: { Save: inside } }), true, []],
    [
      'several rules broken at once, in the order of the rules',
      clean({
        scroll: { width: 400, height: 900, x: 0, y: 0 },
        cutOff: ['p 1'],
        small: ['9px "x"'],
        actions: { Save: null },
      }),
      true,
      [
        'scrolls sideways 400 > 390',
        'scrolls down 900 > 844',
        'cut off: p 1',
        'small text: 9px "x"',
        'Save is missing',
      ],
    ],
  ])('judges %s', (_name, look, fits, problems) => {
    expect(problemsOf(look, { fits })).toEqual(problems)
  })
})

describe('the sizes, themes and tables of the audit', () => {
  it('holds the six sizes of the design, largest first, and both themes', () => {
    expect(SIZES.map(sizeName)).toEqual([
      '1920x1080',
      '1440x900',
      '1366x768',
      '1280x720',
      '1024x768',
      '390x844',
    ])
    expect([...THEMES]).toEqual(['light', 'dark'])
    expect(TEXT_FLOOR).toBe(11)
  })

  it.each([
    ['1280x720', [{ width: 1280, height: 720 }]],
    [
      '1280x720, 390x844',
      [
        { width: 1280, height: 720 },
        { width: 390, height: 844 },
      ],
    ],
  ])('reads the sizes "%s"', (text, sizes) => {
    expect(parseSizes(text)).toEqual(sizes)
  })

  it.each(['', 'wide', '1280', '1280x', 'x720', '1280x720,', '12.5x720'])(
    'refuses the sizes "%s"',
    (text) => {
      expect(() => parseSizes(text)).toThrow(/not a size/)
    },
  )

  it('prints a row for every page, size and theme, aligned, with ok for a page that is clean', () => {
    const table = formatTable([
      { page: '/', size: '1280x720', theme: 'light', problems: [] },
      {
        page: '/new',
        size: '390x844',
        theme: 'dark',
        problems: ['scrolls down 921 > 844', 'Create game is covered'],
      },
    ])
    expect(table.split('\n')).toEqual([
      'page  size      theme  result',
      '/     1280x720  light  ok',
      '/new  390x844   dark   scrolls down 921 > 844; Create game is covered',
    ])
  })
})

describe('waiting for a layout to settle', () => {
  it('returns at once when the first look is clean', async () => {
    const look = vi.fn(() => Promise.resolve(clean()))
    await expect(settle(look, { fits: true })).resolves.toEqual([])
    expect(look).toHaveBeenCalledTimes(1)
  })

  it('looks again until the page has kept the rules', async () => {
    const wide = clean({ scroll: { width: 500, height: 844, x: 0, y: 0 } })
    const looks = [wide, wide, clean()]
    const look = vi.fn(() => Promise.resolve(looks.shift() ?? clean()))
    await expect(settle(look, { fits: true }, { stepMs: 1 })).resolves.toEqual([])
    expect(look).toHaveBeenCalledTimes(3)
  })

  it('gives what is still wrong once the time is up', async () => {
    const look = vi.fn(() => Promise.resolve(clean({ cutOff: ['div 400..500'] })))
    await expect(settle(look, { fits: true }, { timeoutMs: 30, stepMs: 5 })).resolves.toEqual([
      'cut off: div 400..500',
    ])
    expect(look.mock.calls.length).toBeGreaterThan(1)
  })
})

interface Rect {
  left: number
  top: number
  right: number
  bottom: number
}

describe('measure, on a page whose boxes and styles are given', () => {
  const rects = new Map<Element, Rect>()
  const styles = new Map<Element, Record<string, string>>()
  const ask = (change: Partial<Ask> = {}): Ask => ({
    actions: {},
    reveal: false,
    exempt: [],
    floor: TEXT_FLOOR,
    ...change,
  })
  const put = (selector: string, left: number, top: number, right: number, bottom: number): Element => {
    const element = document.querySelector(selector)
    if (element === null) throw new Error(`no ${selector}`)
    rects.set(element, { left, top, right, bottom })
    return element
  }
  const style = (selector: string, values: Record<string, string>): void => {
    const element = document.querySelector(selector)
    if (element === null) throw new Error(`no ${selector}`)
    styles.set(element, values)
  }
  const box = ({ left, top, right, bottom }: Rect): DOMRect =>
    ({ left, top, right, bottom, width: right - left, height: bottom - top, x: left, y: top }) as DOMRect

  beforeEach(() => {
    rects.clear()
    styles.clear()
    vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (this: Element) {
      return box(rects.get(this) ?? { left: 0, top: 0, right: 0, bottom: 0 })
    })
    vi.spyOn(window, 'getComputedStyle').mockImplementation(
      (element: Element) =>
        ({
          display: 'block',
          visibility: 'visible',
          position: 'static',
          overflowX: 'visible',
          fontSize: '16px',
          ...styles.get(element),
        }) as unknown as CSSStyleDeclaration,
    )
    for (const [name, value] of [
      ['clientWidth', 390],
      ['clientHeight', 844],
      ['scrollWidth', 390],
      ['scrollHeight', 1000],
    ] as const) {
      Object.defineProperty(document.documentElement, name, { configurable: true, value })
    }
    document.elementFromPoint = vi.fn(() => null)
    Element.prototype.scrollIntoView = vi.fn()
    window.scrollTo = vi.fn()
  })

  afterEach(() => {
    vi.restoreAllMocks()
    for (const name of ['clientWidth', 'clientHeight', 'scrollWidth', 'scrollHeight']) {
      Reflect.deleteProperty(document.documentElement, name)
    }
    document.body.innerHTML = ''
  })

  it('reports the window and the document it is laid out in', () => {
    document.body.innerHTML = '<main></main>'
    expect(measure(ask())).toEqual(
      clean({ scroll: { width: 390, height: 1000, x: 0, y: 0 }, inner: { width: 390, height: 844 } }),
    )
  })

  it('names the outermost box that leaves the window, not the boxes inside it', () => {
    document.body.innerHTML = '<div id="wide"><p id="inner">Words</p></div>'
    put('#wide', 0, 0, 500, 100)
    put('#inner', 10, 10, 450, 40)
    expect(measure(ask()).cutOff).toEqual(['div "Words" 0..500'])
  })

  it('names a control that a box clips, and none that sits inside it', () => {
    document.body.innerHTML =
      '<div id="frame"><button id="in">Save</button><button id="out">Create</button></div>'
    style('#frame', { overflowX: 'hidden' })
    put('#frame', 0, 0, 200, 100)
    put('#in', 10, 10, 100, 50)
    put('#out', 150, 10, 260, 50)
    expect(measure(ask()).cutOff).toEqual(['button "Create" 150..260'])
  })

  it.each([
    ['a box in the flow', {}, ['li "Third" 200..300']],
    ['a fixed box, which no ancestor clips,', { position: 'fixed' }, []],
  ])('judges %s lying beyond the strip that scrolls, inside the window', (_name, values, found) => {
    document.body.innerHTML = '<ul id="strip"><li id="item">Third</li></ul>'
    style('#strip', { overflowX: 'auto' })
    style('#item', values)
    put('#strip', 0, 0, 100, 60)
    put('#item', 200, 0, 300, 60)
    expect(measure(ask()).cutOff).toEqual(found)
  })

  it('leaves alone what a region named in exempt holds', () => {
    document.body.innerHTML = '<ul id="strip" data-slot="strip"><li id="item">Third</li></ul>'
    style('#strip', { overflowX: 'auto' })
    put('#strip', 0, 0, 390, 60)
    put('#item', 380, 0, 556, 60)
    expect(measure(ask()).cutOff).toHaveLength(1)
    expect(measure(ask({ exempt: ['[data-slot="strip"]'] })).cutOff).toEqual([])
  })

  it('checks the region named in exempt itself', () => {
    document.body.innerHTML = '<ul id="strip" data-slot="strip"><li>Third</li></ul>'
    put('#strip', 0, 0, 500, 60)
    expect(measure(ask({ exempt: ['[data-slot="strip"]'] })).cutOff).toEqual(['ul "Third" 0..500'])
  })

  it.each([
    ['a box that is not displayed', { display: 'none' }, { left: 0, top: 0, right: 500, bottom: 40 }],
    ['a box that is hidden', { visibility: 'hidden' }, { left: 0, top: 0, right: 500, bottom: 40 }],
    ['the 1 px box of a hidden label', {}, { left: 400, top: 0, right: 401, bottom: 1 }],
    ['a box with no size', {}, { left: 400, top: 0, right: 400, bottom: 0 }],
  ])('does not count %s', (_name, values, rect) => {
    document.body.innerHTML = '<p id="p">Words</p>'
    style('#p', values)
    put('#p', rect.left, rect.top, rect.right, rect.bottom)
    expect(measure(ask()).cutOff).toEqual([])
  })

  it('does not count the parts of a drawing', () => {
    document.body.innerHTML = '<svg id="drawing"><path id="part"></path></svg>'
    put('#drawing', 0, 0, 100, 100)
    put('#part', 0, 0, 900, 100)
    expect(measure(ask()).cutOff).toEqual([])
  })

  it('lists text under the floor, with its size, and not text at it', () => {
    document.body.innerHTML =
      '<p id="a">Enter</p><p id="b">Eleven</p><p id="c">Hidden</p><span role="img"><i id="d">R</i></span><p id="e"> </p>'
    for (const [id, size] of [
      ['#a', '10.4px'],
      ['#b', '11px'],
      ['#c', '8px'],
      ['#d', '6.8px'],
      ['#e', '9px'],
    ] as const) {
      style(id, { fontSize: size, ...(id === '#c' ? { display: 'none' } : {}) })
      put(id, 0, 0, 100, 20)
    }
    document.querySelector('[role="img"]')?.setAttribute('aria-label', 'Mana cost')
    expect(measure(ask()).small).toEqual(['10.4px "Enter"'])
  })

  it('finds an action by selector or by its words, and says where it is', () => {
    document.body.innerHTML =
      '<header><a id="head">Sign in</a></header><main><button id="go" aria-label="Join game">Join</button><a id="new" href="/new">Create</a></main>'
    put('#head', 0, 0, 10, 10)
    put('#go', 10, 10, 100, 50)
    put('#new', 120, 10, 220, 50)
    const look = measure(
      ask({
        actions: {
          Join: 'text:Join game',
          Create: 'main a[href="/new"]',
          Words: 'text:Create',
          Head: 'text:Sign in',
        },
      }),
    )
    expect(look.actions['Join']).toEqual({ box: { left: 10, top: 10, right: 100, bottom: 50 }, free: true })
    expect(look.actions['Create']?.box).toEqual({ left: 120, top: 10, right: 220, bottom: 50 })
    expect(look.actions['Words']?.box).toEqual({ left: 120, top: 10, right: 220, bottom: 50 })
    // Words are looked for in the page, not in the header of the shell.
    expect(look.actions['Head']).toBeNull()
  })

  it('says an action is missing when no match is drawn', () => {
    document.body.innerHTML = '<main><button id="hidden">Save</button></main>'
    expect(measure(ask({ actions: { Save: 'text:Save', Gone: '#gone' } })).actions).toEqual({
      Save: null,
      Gone: null,
    })
  })

  it('says an action is covered when something else is on top of it', () => {
    document.body.innerHTML = '<main><button id="go">Join</button><div id="sheet"></div></main>'
    put('#go', 10, 10, 110, 50)
    const sheet = document.querySelector('#sheet')
    document.elementFromPoint = vi.fn((x: number) => (x > 80 ? sheet : document.querySelector('#go')))
    expect(measure(ask({ actions: { Join: '#go' } })).actions['Join']?.free).toBe(false)
  })

  it('scrolls an action into view only when asked, and puts the scroll back', () => {
    document.body.innerHTML = '<main><button id="go">Create</button></main>'
    const button = put('#go', 10, 900, 110, 940)
    vi.mocked(Element.prototype.scrollIntoView).mockImplementation(function (this: Element) {
      rects.set(this, { left: 10, top: 400, right: 110, bottom: 440 })
    })
    const left = measure(ask({ actions: { Create: '#go' } })).actions['Create']
    expect(left?.box.top).toBe(900)
    expect(window.scrollTo).not.toHaveBeenCalled()

    const revealed = measure(ask({ actions: { Create: '#go' }, reveal: true })).actions['Create']
    expect(revealed?.box.top).toBe(400)
    expect(button.scrollIntoView).toHaveBeenCalledTimes(1)
    expect(window.scrollTo).toHaveBeenCalledWith(0, 0)
  })
})
