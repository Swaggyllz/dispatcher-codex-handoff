import { useMutation, useQueryClient } from "@tanstack/react-query";
import { sendHandoffContinuation } from "@/lib/api/dashboard";
import type { ProviderContinuationResponse } from "@/types";

export function useHandoffContinuation() {
  const queryClient = useQueryClient();

  return useMutation<
    ProviderContinuationResponse,
    Error,
    { prompt: string; packageId: string }
  >({
    mutationFn: ({ prompt, packageId }) =>
      sendHandoffContinuation(prompt, packageId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["telemetry"] });
    },
  });
}
