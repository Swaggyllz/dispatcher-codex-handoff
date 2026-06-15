import { useQuery } from "@tanstack/react-query";
import { fetchProviders } from "@/lib/api/dashboard";
import type { ProvidersResponse } from "@/types";

export function useProviders() {
  return useQuery<ProvidersResponse>({
    queryKey: ["providers"],
    queryFn: fetchProviders,
    staleTime: 30_000,
  });
}
