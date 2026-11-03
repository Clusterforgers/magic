/** The fix-mark client the card menu of the table marks with: the server's, unless a test or dev scene brings its own. */
import { createContext } from 'react'
import { fixMarksApi } from '@/net/fixMarks'
import type { FixMarksApi } from '@/net/fixMarks'

/** Where the card menu sends `PUT` and `DELETE /api/fix-marks/{oracle_id}`; the shared client by default. */
export const FixMarksApiContext = createContext<FixMarksApi>(fixMarksApi)
