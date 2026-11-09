import '@testing-library/jest-dom/vitest'
import { cleanup, configure } from '@testing-library/react'
import { afterEach } from 'vitest'
import { toReadClient } from '@/features/reads/useToReadCount'
import { scaled } from '@/test-utils/loadFactor'

// Vitest runs without globals, so Testing Library cannot register its own cleanup.
afterEach(cleanup)

// The nav badge's count is cached per account, and the fake accounts have the same ids in every test.
afterEach(() => toReadClient.clear())

// `waitFor` and `findBy*` give up after 1 s by default, which a machine running several test suites can exceed.
configure({ asyncUtilTimeout: scaled(4000) })

// jsdom logs "not implemented" for every scroll the router restores on navigation.
window.scrollTo = () => undefined
