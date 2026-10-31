/** The card client the previews of the table read the oracle text with: the server's, unless a dev scene brings its own. */
import { createContext } from 'react'
import { cardsApi } from '@/net/cards'
import type { CardsApi } from '@/net/cards'

/** Where a preview of the table reads `GET /api/cards/{oracle_id}`; the shared client by default. */
export const CardsApiContext = createContext<CardsApi>(cardsApi)
