import { useQuery } from "@tanstack/react-query";
import { fetchTelemetry } from "@/lib/api/dashboard";
import type { TelemetryStats } from "@/types";

export function useTelemetry() {
  return useQuery<TelemetryStats>({
    queryKey: ["telemetry"],
    queryFn: fetchTelemetry,
    refetchInterval: 10_000,
    staleTime: 5_000,
  });
}
