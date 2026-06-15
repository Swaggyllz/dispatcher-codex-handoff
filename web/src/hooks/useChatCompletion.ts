import { useMutation, useQueryClient } from "@tanstack/react-query";
import { sendChatCompletion } from "@/lib/api/dashboard";
import type { ChatCompletionResponse, RoutingStrategy } from "@/types";

export function useChatCompletion() {
  const queryClient = useQueryClient();

  return useMutation<
    ChatCompletionResponse,
    Error,
    { prompt: string; strategy: RoutingStrategy }
  >({
    mutationFn: ({ prompt, strategy }) => sendChatCompletion(prompt, strategy),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["telemetry"] });
    },
  });
}
