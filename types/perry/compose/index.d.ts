/**
 * perry/compose — TypeScript bindings for perry-container-compose
 *
 * Docker Compose-like experience for Apple Container, powered by Perry.
 *
 * @module perry/compose
 */

import { ContainerInfo, ContainerLogs } from "perry/container";

// ============ Configuration Types ============

/**
 * Build configuration for a service image.
 */
export interface Build {
  /** Build context directory (relative to compose file) */
  context?: string;
  /** Path to Dockerfile */
  dockerfile?: string;
  /** Build-time arguments */
  args?: Record<string, string>;
  /** Labels to add to the built image */
  labels?: Record<string, string>;
  /** Build target stage */
  target?: string;
  /** Network to use during build */
  network?: string;
}

/**
 * A single service definition in a Compose file.
 */
export interface Service {
  /** Container image reference */
  image?: string;
  /** Explicit container name */
  container_name?: string;
  /** Port mappings, e.g. "8080:80" */
  ports?: string[];
  /** Environment variables (map or KEY=VALUE list) */
  environment?: Record<string, string> | string[];
  /** Container labels */
  labels?: Record<string, string>;
  /** Volume mounts, e.g. "./data:/data:ro" */
  volumes?: string[];
  /** Build configuration */
  build?: Build;
  /** Service dependencies */
  depends_on?: string[] | Record<string, { condition?: string }>;
  /** Restart policy */
  restart?: "no" | "always" | "on-failure" | "unless-stopped";
  /** Override container entrypoint */
  entrypoint?: string | string[];
  /** Override container command */
  command?: string | string[];
  /** Networks this service is attached to */
  networks?: string[];
  /** Healthcheck configuration */
  healthcheck?: ComposeHealthcheck;
  /** Mount the container root as read-only */
  read_only?: boolean;
  /** Custom seccomp profile path */
  seccomp?: string;
  /** Secrets to expose to the service */
  secrets?: string[];
  /** Configs to expose to the service */
  configs?: string[];
}

/**
 * Healthcheck configuration.
 */
export interface ComposeHealthcheck {
  /** Test command (string or array) */
  test: string | string[];
  /** Check interval (e.g., "30s") */
  interval?: string;
  /** Timeout (e.g., "10s") */
  timeout?: string;
  /** Number of retries before unhealthy */
  retries?: number;
  /** Startup grace period (e.g., "40s") */
  start_period?: string;
}

/**
 * Network definition in a Compose file.
 */
export interface ComposeNetwork {
  driver?: string;
  external?: boolean;
  name?: string;
}

/**
 * Volume definition in a Compose file.
 */
export interface ComposeVolume {
  driver?: string;
  external?: boolean;
  name?: string;
}

/**
 * Root Compose file structure (docker-compose.yaml / compose.yaml).
 */
export interface ComposeSpec {
  name?: string;
  version?: string;
  services: Record<string, Service>;
  networks?: Record<string, ComposeNetwork>;
  volumes?: Record<string, ComposeVolume>;
  secrets?: Record<string, ComposeSecret>;
  configs?: Record<string, ComposeConfig>;
}

/**
 * Secret definition in a Compose file.
 */
export interface ComposeSecret {
  name?: string;
  file?: string;
  environment?: string;
  external?: boolean;
  labels?: Record<string, string>;
  driver?: string;
  driver_opts?: Record<string, string>;
}

/**
 * Config definition in a Compose file.
 */
export interface ComposeConfig {
  name?: string;
  file?: string;
  environment?: string;
  content?: string;
  external?: boolean;
  labels?: Record<string, string>;
}

/**
 * Opaque handle to a running compose stack.
 */
export type ComposeHandle = number;

// ============ Options Types ============

export interface UpOptions {
  /** Start in detached mode (default: true) */
  detach?: boolean;
  /** Build images before starting */
  build?: boolean;
  /** Services to start (empty = all) */
  services?: string[];
  /** Remove orphaned containers */
  removeOrphans?: boolean;
}

export interface DownOptions {
  /** Remove named volumes */
  volumes?: boolean;
}

export interface LogsOptions {
  /** Service name to get logs from (optional) */
  service?: string;
  /** Number of lines to show from the end */
  tail?: number;
}

// ============ API Functions ============

/**
 * Bring up services defined in a compose spec.
 * @param spec Compose specification object
 * @returns Promise resolving to the stack handle
 */
export function up(spec: ComposeSpec): Promise<ComposeHandle>;

/**
 * Stop and remove services in a stack.
 * @param handle Stack handle returned by up()
 * @param options Down options
 */
export function down(handle: ComposeHandle, options?: DownOptions): Promise<void>;

/**
 * List service statuses in a stack.
 * @param handle Stack handle
 * @returns Array of ContainerInfo entries
 */
export function ps(handle: ComposeHandle): Promise<ContainerInfo[]>;

/**
 * Get logs from services in a stack.
 * @param handle Stack handle
 * @param options Log options
 * @returns Promise resolving to ContainerLogs
 */
export function logs(
  handle: ComposeHandle,
  options?: LogsOptions
): Promise<ContainerLogs>;

/**
 * Execute a command in a running service container within a stack.
 * @param handle Stack handle
 * @param service Service name
 * @param cmd Command and arguments to execute
 * @returns Promise resolving to ContainerLogs
 */
export function exec(
  handle: ComposeHandle,
  service: string,
  cmd: string[]
): Promise<ContainerLogs>;

/**
 * Get the resolved compose configuration.
 * @param handle Stack handle
 * @returns Validated configuration as YAML string
 */
export function config(handle: ComposeHandle): Promise<string>;

/**
 * Start existing stopped services in a stack.
 * @param handle Stack handle
 * @param services Services to start (empty = all)
 */
export function start(handle: ComposeHandle, services?: string[]): Promise<void>;

/**
 * Stop running services in a stack.
 * @param handle Stack handle
 * @param services Services to stop (empty = all)
 */
export function stop(handle: ComposeHandle, services?: string[]): Promise<void>;

/**
 * Restart services in a stack.
 * @param handle Stack handle
 * @param services Services to restart (empty = all)
 */
export function restart(handle: ComposeHandle, services?: string[]): Promise<void>;
