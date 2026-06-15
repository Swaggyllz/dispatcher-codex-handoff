import { useMutation, useQuery } from "@tanstack/react-query";
import { fetchPolicy, savePolicy } from "@/lib/api/dashboard";
import type { PolicySaveResponse, PolicyUpdate, RoutingPolicy } from "@/types";

export function usePolicy() {
  return useQuery<RoutingPolicy>({
    queryKey: ["policy"],
    queryFn: fetchPolicy,
    staleTime: 30_000,
  });
}

export function useSavePolicy() {
  return useMutation<PolicySaveResponse, Error, PolicyUpdate>({
    mutationFn: savePolicy,
  });
}
