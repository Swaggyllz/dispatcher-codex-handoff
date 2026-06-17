import type {
  HealthStatus,
  ProvidersResponse,
  TelemetryStats,
  ChatCompletionResponse,
  RoutingPolicy,
  PolicySaveResponse,
  PolicyUpdate,
  ProviderContinuationResponse,
  RoutingStrategy,
} from "@/types";

const BASE = "/v1";

async function request<T>(url: string, init?: RequestInit): Promise<T> {
  const resp = await fetch(url, init);
  const body = await resp.json().catch(() => null);
  if (!resp.ok) {
    throw new DashboardApiError(
      body?.error?.message ?? `HTTP ${resp.status}: ${resp.statusText}`,
      resp.status,
      body?.error?.fields ?? [],
    );
  }
  return body as T;
}

export class DashboardApiError extends Error {
  constructor(
    message: string,
    public readonly status: number,
    public readonly fields: Array<{ field: string; message: string }>,
  ) {
    super(message);
    this.name = "DashboardApiError";
  }
}

export function fetchHealth(): Promise<HealthStatus> {
  return request<HealthStatus>(`${BASE}/health`);
}

export function fetchProviders(): Promise<ProvidersResponse> {
  return request<ProvidersResponse>(`${BASE}/providers`);
}

export function fetchTelemetry(): Promise<TelemetryStats> {
  return request<TelemetryStats>(`${BASE}/telemetry`);
}

export function fetchPolicy(): Promise<RoutingPolicy> {
  return request<RoutingPolicy>(`${BASE}/policy`);
}

export function savePolicy(policy: PolicyUpdate): Promise<PolicySaveResponse> {
  return request<PolicySaveResponse>(`${BASE}/policy`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(policy),
  });
}

export function sendChatCompletion(
  prompt: string,
  strategy: RoutingStrategy,
): Promise<ChatCompletionResponse> {
  return request<ChatCompletionResponse>(`${BASE}/chat/completions`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      model: "auto",
      messages: [{ role: "user", content: prompt }],
      stream: false,
      strategy,
    }),
  });
}

export function sendHandoffContinuation(
  prompt: string,
): Promise<ProviderContinuationResponse> {
  return fetch(`${BASE}/responses`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-Dispatcher-Mode": "provider-auto",
    },
    body: JSON.stringify({
      model: "auto",
      input: [
        {
          type: "message",
          role: "user",
          content: [{ type: "input_text", text: prompt }],
        },
      ],
      stream: false,
      strategy: "auto",
    }),
  }).then(async (resp) => {
    const body = await resp.json().catch(() => null);
    if (!resp.ok) {
      throw new DashboardApiError(
        body?.error?.message ?? `HTTP ${resp.status}: ${resp.statusText}`,
        resp.status,
        body?.error?.fields ?? [],
      );
    }
    return {
      ...(body as ProviderContinuationResponse),
      dispatcher_provider: resp.headers.get("x-dispatcher-provider"),
      dispatcher_model: resp.headers.get("x-dispatcher-model"),
    };
  });
}
