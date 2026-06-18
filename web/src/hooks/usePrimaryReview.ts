import { useMutation, useQueryClient } from "@tanstack/react-query";
import { sendPrimaryReview } from "@/lib/api/dashboard";
import type { ProviderContinuationResponse } from "@/types";

export function usePrimaryReview() {
  const queryClient = useQueryClient();

  return useMutation<ProviderContinuationResponse, Error, { prompt: string }>({
    mutationFn: ({ prompt }) => sendPrimaryReview(prompt),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["telemetry"] });
    },
  });
}
