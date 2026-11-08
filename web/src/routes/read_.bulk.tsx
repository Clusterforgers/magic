import { createFileRoute } from '@tanstack/react-router'
import { BulkReadPage } from '@/features/bulk-reads/BulkReadPage'

export const Route = createFileRoute('/read_/bulk')({ component: () => <BulkReadPage /> })
