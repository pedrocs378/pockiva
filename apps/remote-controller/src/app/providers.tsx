import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { ReactQueryDevtools } from '@tanstack/react-query-devtools'
import { RouterProvider } from '@tanstack/react-router'
import { TanStackRouterDevtools } from '@tanstack/react-router-devtools'
import { router } from './router'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false
    }
  }
})

export const AppProviders = () => (
  <QueryClientProvider client={queryClient}>
    <RouterProvider router={router} />
    {import.meta.env.DEV ? (
      <>
        <TanStackRouterDevtools router={router} position="bottom-right" />
        <ReactQueryDevtools initialIsOpen={false} />
      </>
    ) : null}
  </QueryClientProvider>
)
