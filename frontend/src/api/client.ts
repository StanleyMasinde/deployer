import type { BuildConfig } from "../build";

export class ApiError extends Error {
  constructor(
    message: string,
    public status: number,
  ) {
    super(message);
  }
}

export type Resource = {
  type: string;
  id?: string;
  attributes?: Record<string, unknown>;
};

type Document = {
  data?: Resource | Resource[] | null;
  errors?: { detail?: string; title?: string }[];
};

export function unpack<T>(data: Resource): T {
  return { ...data.attributes, ...(data.id ? { id: Number(data.id) || data.id } : {}) } as T;
}

export async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const response = await fetch(`/api${path}`, {
    credentials: "same-origin",
    ...options,
    headers: {
      Accept: "application/vnd.api+json",
      ...(options.body ? { "Content-Type": "application/vnd.api+json" } : {}),
      ...options.headers,
    },
  });

  if (response.status === 204 || response.status === 202) return undefined as T;

  const document: Document = await response.json();

  if (!response.ok) {
    throw new ApiError(document.errors?.[0]?.detail ?? "The request failed.", response.status);
  }

  if (Array.isArray(document.data)) return document.data.map(unpack) as T;
  if (document.data) return unpack<T>(document.data);

  return undefined as T;
}

export function body(type: string, attributes: object): string {
  return JSON.stringify({ data: { type, attributes } });
}

export type App = {
  buildConfig: BuildConfig;
  installCommand: string;
  buildCommand: string;
  outputDirectory: string;
  deletionRequested: boolean;
  id: number;
  repository: string;
  branch: string;
  domain: string;
  spa: boolean;
  workflowPr: string | null;
};

export type Deployment = {
  id: number;
  appId: number;
  runId: number;
  attempt: number;
  commitSha: string;
  status: string;
  error: string | null;
  createdAt: number;
  finishedAt: number | null;
};

export type Repository = { fullName: string; defaultBranch: string };
