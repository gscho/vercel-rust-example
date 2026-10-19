"use client";

import { useCallback, useEffect, useState } from "react";

const POLL_INTERVAL_MS = 5000;

interface PollingState<T> {
  data?: T;
  error?: Error;
}

/** Loads data on mount and every few seconds. `load` must be stable between renders. */
export function usePolling<T>(load: () => Promise<T>) {
  const [state, setState] = useState<PollingState<T>>({});

  const refresh = useCallback(async () => {
    try {
      const data = await load();
      setState({ data });
    } catch (error) {
      setState((previous) => ({ ...previous, error: error instanceof Error ? error : new Error(String(error)) }));
    }
  }, [load]);

  useEffect(() => {
    void refresh();
    const timer = setInterval(() => void refresh(), POLL_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [refresh]);

  return { ...state, refresh };
}
