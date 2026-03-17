import { QueryClient } from '@tanstack/react-query';

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: Infinity,     // only refetch on explicit invalidation
      refetchOnWindowFocus: false,
    },
  },
});
