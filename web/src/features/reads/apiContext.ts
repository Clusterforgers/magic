/** The read client the table's `ReadDialog` uses: the server's, unless a test or dev scene brings its own. */
import { createContext } from 'react'
import { readsApi } from '@/net/reads'
import type { ReadsApi } from '@/net/reads'

/** Where `ReadDialog` sends `GET` and `PUT /api/reads/{oracle_id}`; the shared client by default. */
export const ReadsApiContext = createContext<ReadsApi>(readsApi)
