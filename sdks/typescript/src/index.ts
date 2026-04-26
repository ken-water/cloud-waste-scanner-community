import { createHmac, timingSafeEqual } from "node:crypto";

export type JsonObject = Record<string, unknown>;

export interface PageMeta {
  limit: number;
  next_cursor: string | null;
  has_more: boolean;
}

export interface PageEnvelope<T> {
  items: T[];
  page: PageMeta;
}

export interface CwsClientOptions {
  baseUrl?: string;
  token?: string;
  fetchImpl?: typeof fetch;
}

export class CwsClientError extends Error {
  status: number;
  payload: unknown;

  constructor(status: number, message: string, payload: unknown) {
    super(`CWS API error ${status}: ${message}`);
    this.name = "CwsClientError";
    this.status = status;
    this.payload = payload;
  }
}

export class CwsClient {
  private readonly baseUrl: string;
  private readonly token?: string;
  private readonly fetchImpl: typeof fetch;

  constructor(options: CwsClientOptions = {}) {
    this.baseUrl = (options.baseUrl ?? "http://127.0.0.1:43177").replace(/\/$/, "");
    this.token = options.token;
    this.fetchImpl = options.fetchImpl ?? fetch;
  }

  async request<T = unknown>(
    method: string,
    path: string,
    options: { query?: JsonObject; body?: JsonObject } = {},
  ): Promise<T> {
    const url = new URL(`${this.baseUrl}${path.startsWith("/") ? path : `/${path}`}`);
    for (const [key, value] of Object.entries(options.query ?? {})) {
      if (value !== undefined && value !== null && value !== "") {
        url.searchParams.set(key, String(value));
      }
    }

    const headers: Record<string, string> = { Accept: "application/json" };
    let body: string | undefined;
    if (options.body) {
      headers["Content-Type"] = "application/json";
      body = JSON.stringify(options.body);
    }
    if (this.token) {
      headers.Authorization = `Bearer ${this.token}`;
    }

    const response = await this.fetchImpl(url, {
      method: method.toUpperCase(),
      headers,
      body,
    });
    const text = await response.text();
    const payload = text ? JSON.parse(text) : undefined;
    if (!response.ok) {
      const message =
        payload && typeof payload === "object" && "error" in payload
          ? String((payload as { error: unknown }).error)
          : response.statusText;
      throw new CwsClientError(response.status, message, payload);
    }
    return payload as T;
  }

  status<T = unknown>(): Promise<T> {
    return this.request<T>("GET", "/status");
  }

  openapi<T = unknown>(): Promise<T> {
    return this.request<T>("GET", "/v1/openapi.json");
  }

  compatibility<T = unknown>(): Promise<T> {
    return this.request<T>("GET", "/v1/meta/compatibility");
  }

  listScans<T = unknown>(
    options: { cursor?: string; limit?: number; envelope?: boolean } = {},
  ): Promise<PageEnvelope<T> | T[]> {
    return this.request("GET", "/v1/scans", {
      query: {
        cursor: options.cursor,
        limit: options.limit ?? 50,
        envelope: options.envelope ?? true,
      },
    });
  }

  runScan<T = unknown>(body: JsonObject): Promise<T> {
    return this.request<T>("POST", "/v1/scans", { body });
  }

  listFindings<T = unknown>(
    options: { cursor?: string; limit?: number; envelope?: boolean } = {},
  ): Promise<PageEnvelope<T> | T[]> {
    return this.request("GET", "/v1/findings", {
      query: {
        cursor: options.cursor,
        limit: options.limit ?? 50,
        envelope: options.envelope ?? true,
      },
    });
  }

  listReports<T = unknown>(
    options: { cursor?: string; limit?: number; envelope?: boolean } = {},
  ): Promise<PageEnvelope<T> | T[]> {
    return this.request("GET", "/v1/reports", {
      query: {
        cursor: options.cursor,
        limit: options.limit ?? 50,
        envelope: options.envelope ?? true,
      },
    });
  }

  listK8sContexts<T = unknown>(): Promise<T> {
    return this.request<T>("GET", "/v1/k8s/contexts");
  }

  runK8sScan<T = unknown>(body: {
    kubeconfig_path?: string;
    kube_context?: string;
  }): Promise<T> {
    return this.request<T>("POST", "/v1/k8s/scans", { body });
  }
}

export function verifyWebhookSignature(input: {
  secret: string;
  timestamp: string | number;
  body: string;
  signature: string;
  toleranceSeconds?: number;
  now?: number;
}): boolean {
  if (!input.secret || !input.signature.startsWith("sha256=")) {
    return false;
  }
  const timestamp = Number(input.timestamp);
  if (!Number.isInteger(timestamp)) {
    return false;
  }
  const now = input.now ?? Math.floor(Date.now() / 1000);
  const tolerance = input.toleranceSeconds ?? 300;
  if (tolerance >= 0 && Math.abs(now - timestamp) > tolerance) {
    return false;
  }
  const expected = `sha256=${createHmac("sha256", input.secret)
    .update(`${timestamp}.${input.body}`)
    .digest("hex")}`;
  const expectedBuffer = Buffer.from(expected);
  const actualBuffer = Buffer.from(input.signature);
  return (
    expectedBuffer.length === actualBuffer.length &&
    timingSafeEqual(expectedBuffer, actualBuffer)
  );
}
