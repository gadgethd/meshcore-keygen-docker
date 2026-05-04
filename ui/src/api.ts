const BASE = '/api';
const TIMEOUT = 15000;

async function req<T>(url: string, options?: RequestInit): Promise<T> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), TIMEOUT);
  try {
    const res = await fetch(BASE + url, {
      signal: controller.signal,
      headers: { 'Content-Type': 'application/json', ...(options?.headers as Record<string,string> || {}) },
      ...options,
    });
    if (res.status === 204) return undefined as T;
    if (!res.ok) {
      const body = await res.text().catch(() => '');
      throw new Error(body || `${res.status} ${res.statusText}`);
    }
    return res.json();
  } finally {
    clearTimeout(timer);
  }
}

export interface Job {
  id: string; name: string; prefixes: string[]; backend: string; device: string;
  status: string; priority: number; created_at: string; updated_at: string;
  master_seed?: string; next_counter?: number; attempts_done: number;
  keys_per_second: number; elapsed_seconds: number;
  cpu_reserved_cores: number; cpu_worker_threads: number;
  max_attempts?: number; max_runtime?: number;
  schedule_enabled: boolean; schedule_start?: string; schedule_end?: string; notes?: string;
}

export interface Result {
  id: string; job_id: string; prefix: string; public_key: string; private_key: string;
  candidate_seed?: string; master_seed?: string; counter?: number;
  attempts: number; elapsed_seconds: number; keys_per_second: number;
  backend: string; device: string; created_at: string;
}

export interface Benchmark {
  id: string; created_at: string; backend: string; device: string;
  prefix_length: number; target_prefix: string; attempts: number;
  elapsed_seconds: number; keys_per_second: number; found: boolean;
  timeout_seconds: number; cpu_total_cores: number; cpu_reserved_cores: number;
  cpu_worker_threads: number; is_default: boolean;
}

export interface Estimate {
  prefix_length: number; expected_attempts: number; keys_per_second: number;
  estimated_seconds: number; milestone_50pct_seconds: number;
  milestone_90pct_seconds: number; milestone_95pct_seconds: number;
  milestone_99pct_seconds: number; backend: string; device: string;
  benchmark_id?: string; benchmark_age?: string;
}

export interface SystemStatus {
  cpu_total_cores: number; cpu_reserved_cores: number; cpu_available_workers: number;
  gpu_available: boolean; gpu_name?: string; active_job?: Job;
  queue_length: number; results_count: number; last_benchmark_keys_per_second?: number;
}

export interface SettingsData {
  reserved_cpu_cores: number; max_worker_threads?: number;
  checkpoint_interval_secs: number; default_backend: string; default_benchmark_id?: string;
  timezone: string; hide_secrets: boolean; max_log_lines: number;
}

export const api = {
  status: () => req<SystemStatus>('/status'),
  jobs: () => req<Job[]>('/jobs'),
  createJob: (data: { name?: string; prefixes: string[]; backend?: string; max_attempts?: number; max_runtime?: number; notes?: string }) =>
    req<Job>('/jobs', { method: 'POST', body: JSON.stringify(data) }),
  getJob: (id: string) => req<Job>(`/jobs/${id}`),
  updateJob: (id: string, data: Record<string, unknown>) =>
    req<Job>(`/jobs/${id}`, { method: 'PATCH', body: JSON.stringify(data) }),
  deleteJob: (id: string) => req<void>(`/jobs/${id}`, { method: 'DELETE' }),
  pauseJob: (id: string) => req<Job>(`/jobs/${id}/pause`, { method: 'POST' }),
  resumeJob: (id: string) => req<Job>(`/jobs/${id}/resume`, { method: 'POST' }),
  stopJob: (id: string) => req<Job>(`/jobs/${id}/stop`, { method: 'POST' }),
  restartJob: (id: string) => req<Job>(`/jobs/${id}/restart`, { method: 'POST' }),
  duplicateJob: (id: string) => req<Job>(`/jobs/${id}/duplicate`, { method: 'POST' }),
  results: () => req<Result[]>('/results'),
  deleteResult: (id: string) => req<void>(`/results/${id}`, { method: 'DELETE' }),
  estimate: (prefixes: string[], backend?: string) =>
    req<Estimate>('/estimate', { method: 'POST', body: JSON.stringify({ prefixes, backend }) }),
  benchmarks: () => req<Benchmark[]>('/benchmarks'),
  createBenchmark: (data: Record<string, unknown>) =>
    req<Benchmark>('/benchmarks', { method: 'POST', body: JSON.stringify(data) }),
  deleteBenchmark: (id: string) => req<void>(`/benchmarks/${id}`, { method: 'DELETE' }),
  setDefaultBenchmark: (id: string) => req<void>(`/benchmarks/${id}/set-default`, { method: 'POST' }),
  settings: () => req<SettingsData>('/settings'),
  updateSettings: (data: Record<string, unknown>) =>
    req<SettingsData>('/settings', { method: 'PATCH', body: JSON.stringify(data) }),
  cpuInfo: () => req<Record<string, unknown>>('/system/cpu'),
  devices: () => req<Record<string, unknown>>('/devices'),
};
