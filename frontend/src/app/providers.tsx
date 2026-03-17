import { QueryClientProvider } from '@tanstack/react-query';
import { ReactNode, useEffect } from 'react';
import { ThemeProvider } from '@ui/lib/ThemeContext';
import { wsActor, wsUrl, sendWsMessage } from './actors';
import { queryClient } from './queryClient';
import { useGraphStore } from '@store';

interface ProvidersProps {
  children: ReactNode;
}

export function Providers({ children }: ProvidersProps) {
  useEffect(() => {
    const url = wsUrl ?? `ws://${window.location.host}/ws`;
    wsActor.send({ type: 'CONNECT', url });
    useGraphStore.getState().setWsSend(sendWsMessage);
  }, []);

  return (
    <QueryClientProvider client={queryClient}>
      <ThemeProvider>
        {children}
      </ThemeProvider>
    </QueryClientProvider>
  );
}
