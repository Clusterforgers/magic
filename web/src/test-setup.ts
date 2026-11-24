import '@testing-library/jest-dom/vitest'
import { cleanup, configure } from '@testing-library/react'
import { afterEach } from 'vitest'
import { toReadClient } from '@/features/reads/useToReadCount'
import { listCache } from '@/net/listCache'
import { lexiconStore } from '@/features/table/lexicon/lexiconStore'
import { tourStore } from '@/features/table/tour/tourStore'
import { FULL_CONTROL_KEY, resetFlowPrefs } from '@/play/flowPrefs'
import { scaled } from '@/test-utils/loadFactor'

// Vitest runs without globals, so Testing Library cannot register its own cleanup.
afterEach(cleanup)

// The nav badge's count is cached per account, and the fake accounts have the same ids in every test.
afterEach(() => toReadClient.clear())

// The deck chooser's lists are cached per client, and some tests share one fake client.
afterEach(() => listCache.clear())

// The table tour and the lexicon are one per page, so every test leaves them shut.
afterEach(() => {
  tourStore.setState({ step: null })
  lexiconStore.setState({ open: false, query: '', kind: 'all' })
})

// A held Full Control saved by one test would be asked for at every later Welcome, so each test starts without it.
afterEach(() => {
  try {
    window.localStorage.removeItem(FULL_CONTROL_KEY)
  } catch {
    // No storage, nothing kept.
  }
  resetFlowPrefs()
})

// `waitFor` and `findBy*` give up after 1 s by default, which a machine running several test suites can exceed.
configure({ asyncUtilTimeout: scaled(4000) })

// jsdom logs "not implemented" for every scroll the router restores on navigation.
window.scrollTo = () => undefined
