import { createRootRoute, createRoute, createRouter } from '@tanstack/react-router'
import { ControllerPage } from '@/pages/ControllerPage'

const rootRoute = createRootRoute()
const controllerRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  component: ControllerPage
})

const routeTree = rootRoute.addChildren([controllerRoute])

export const router = createRouter({ routeTree })

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}
