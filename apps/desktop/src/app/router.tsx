import { createRootRoute, createRoute, createRouter } from '@tanstack/react-router'
import { EmulatorPage } from '@/pages/EmulatorPage'

const rootRoute = createRootRoute()
const emulatorRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  component: EmulatorPage
})

const routeTree = rootRoute.addChildren([emulatorRoute])

export const router = createRouter({ routeTree })

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}
