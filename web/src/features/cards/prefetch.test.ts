// Goal: the pictures loaded ahead are exactly the server's list in the sizes the table draws (each face of a double-faced card, each URL once), a few at a time at idle moments, and nothing at all for text faces, Save-Data or 2G.
import { afterEach, describe, expect, it } from 'vitest'
import type { PictureCard } from '@/protocol/generated'
import {
  FIRST_SIZE,
  PREFETCH_AT_ONCE,
  createPrefetcher,
  drawnSizes,
  mayPrefetch,
  noteDrawnSize,
  onDrawnSize,
  prefetchUrls,
  resetDrawnSizes,
} from './prefetch'
import type { ImageSize } from './images'

const printed: PictureCard = { identity: { oracle_id: 'o1', printing_id: 'p1' }, faces: 1 }
const plain: PictureCard = { identity: { oracle_id: 'o2' }, faces: 1 }
const dfc: PictureCard = { identity: { oracle_id: 'o3', printing_id: 'p3' }, faces: 2 }

afterEach(() => resetDrawnSizes())

describe('prefetchUrls', () => {
  it.each<[string, PictureCard[], ImageSize[], string[]]>([
    ['a chosen printing', [printed], ['normal'], ['/api/images/printing/p1/0/normal']],
    ['a card without a printing', [plain], ['small'], ['/api/images/oracle/o2/0/small']],
    [
      'both faces of a double-faced card',
      [dfc],
      ['normal'],
      ['/api/images/printing/p3/0/normal', '/api/images/printing/p3/1/normal'],
    ],
    [
      'one size after the other',
      [printed, plain],
      ['small', 'normal'],
      [
        '/api/images/printing/p1/0/small',
        '/api/images/oracle/o2/0/small',
        '/api/images/printing/p1/0/normal',
        '/api/images/oracle/o2/0/normal',
      ],
    ],
    ['each URL once', [printed, printed], ['normal', 'normal'], ['/api/images/printing/p1/0/normal']],
    ['nothing for no cards', [], ['normal'], []],
  ])('%s', (_name, cards, sizes, urls) => {
    expect(prefetchUrls(cards, sizes)).toEqual(urls)
  })
})

describe('mayPrefetch', () => {
  it.each<[string, 'on' | 'text', Parameters<typeof mayPrefetch>[1], boolean]>([
    ['pictures on, no hint', 'on', undefined, true],
    ['pictures on, 4G', 'on', { effectiveType: '4g' }, true],
    ['text faces', 'text', undefined, false],
    ['Save-Data', 'on', { saveData: true, effectiveType: '4g' }, false],
    ['2G', 'on', { effectiveType: '2g' }, false],
    ['slow 2G', 'on', { effectiveType: 'slow-2g' }, false],
  ])('%s', (_name, choice, connection, may) => {
    expect(mayPrefetch(choice, connection)).toBe(may)
  })
})

describe('drawn sizes', () => {
  it('start at the first guess and follow the table in the order drawn', async () => {
    expect(drawnSizes()).toEqual([FIRST_SIZE])
    let heard = 0
    const off = onDrawnSize(() => (heard += 1))
    noteDrawnSize('small')
    noteDrawnSize('small')
    noteDrawnSize('art_crop')
    noteDrawnSize('normal')
    expect(drawnSizes()).toEqual(['small', 'normal'])
    await Promise.resolve()
    expect(heard).toBe(2)
    off()
  })
})

describe('createPrefetcher', () => {
  function harness() {
    const idle: (() => void)[] = []
    const started: string[] = []
    const finish = new Map<string, (ok: boolean) => void>()
    const loaded: string[] = []
    const prefetcher = createPrefetcher({
      idle(task) {
        idle.push(task)
        return () => {
          const at = idle.indexOf(task)
          if (at >= 0) idle.splice(at, 1)
        }
      },
      load(url) {
        started.push(url)
        return new Promise((resolve) => finish.set(url, resolve))
      },
      loaded: (url) => loaded.push(url),
    })
    const runIdle = () => idle.splice(0).forEach((task) => task())
    return { prefetcher, started, finish, loaded, runIdle }
  }

  const urls = ['a', 'b', 'c', 'd']

  it('loads nothing before an idle moment, then a few at a time in order', async () => {
    const { prefetcher, started, finish, loaded, runIdle } = harness()
    prefetcher.add(urls)
    expect(started).toEqual([])
    runIdle()
    expect(started).toEqual(urls.slice(0, PREFETCH_AT_ONCE))
    finish.get('a')?.(true)
    await Promise.resolve()
    runIdle()
    expect(started).toEqual(urls.slice(0, PREFETCH_AT_ONCE + 1))
    expect(loaded).toEqual(['a'])
  })

  it('asks for a URL once however often it is added', () => {
    const { prefetcher, started, runIdle } = harness()
    prefetcher.add(['a', 'a'])
    prefetcher.add(['a'])
    runIdle()
    expect(started).toEqual(['a'])
    expect(prefetcher.waiting).toBe(0)
  })

  it('drops what waits once stopped', () => {
    const { prefetcher, started, runIdle } = harness()
    prefetcher.add(urls)
    prefetcher.stop()
    runIdle()
    prefetcher.add(['e'])
    expect(started).toEqual([])
    expect(prefetcher.waiting).toBe(0)
  })
})
