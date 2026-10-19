// Goal: the script in index.html, which sets the theme before the first paint, picks the same theme as the theme store for every stored value and system preference, and survives storage that throws and a browser without matchMedia, so the page never flashes the wrong theme.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { parseChoice, resolveTheme } from '@/hooks/useTheme'

const html = readFileSync(join(import.meta.dirname, '../index.html'), 'utf8')
const SCRIPT = /<script>([\s\S]*?)<\/script>/.exec(html)?.[1] ?? ''

type Media = 'dark' | 'light' | 'absent'
type Storage = string | null | 'throws'

/** Runs the page's script as a browser would: with this storage and preference, on a fresh root element. */
function run(storage: Storage, media: Media): boolean {
  const root = document.createElement('html')
  const fakeDocument = { documentElement: root }
  const fakeStorage = {
    getItem(): string | null {
      if (storage === 'throws') throw new DOMException('denied', 'SecurityError')
      return storage
    },
  }
  const fakeMatchMedia =
    media === 'absent'
      ? undefined
      : (query: string) => ({ matches: media === 'dark' && query.includes('dark') })
  new Function('document', 'localStorage', 'matchMedia', SCRIPT)(fakeDocument, fakeStorage, fakeMatchMedia)
  return root.classList.contains('dark')
}

const STORED: Storage[] = [null, 'light', 'dark', 'system', 'garbage', '', 'throws']
const MEDIA: Media[] = ['dark', 'light', 'absent']

describe('the theme script of index.html', () => {
  it('is an inline script in the head, ahead of the app, so it runs before anything is painted', () => {
    const head = html.slice(0, html.indexOf('</head>'))
    expect(SCRIPT).not.toBe('')
    expect(head).toContain(SCRIPT)
    expect(html.indexOf(SCRIPT)).toBeLessThan(html.indexOf('src="/src/main.tsx"'))
  })

  it('tells the browser both themes are supported, for the canvas before the stylesheet arrives', () => {
    expect(html).toContain('<meta name="color-scheme" content="light dark" />')
  })

  it.each(STORED.flatMap((storage) => MEDIA.map((media) => ({ storage, media }))))(
    'agrees with the theme store for $storage storage and a $media system',
    ({ storage, media }) => {
      const stored = storage === 'throws' ? null : storage
      const expected = resolveTheme(parseChoice(stored), media === 'dark') === 'dark'
      expect(run(storage, media)).toBe(expected)
    },
  )
})
