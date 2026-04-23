/**
 * perry/compose — TypeScript bindings for perry-container-compose
 *
 * Docker Compose-like experience for Apple Container, powered by Perry.
 *
 * @module perry/compose
 */

import {
  ComposeSpec,
  ContainerInfo,
  ContainerLogs,
  BackendInfo
} from 'perry/container';

export {
  ComposeSpec,
  ComposeService,
  ComposeNetwork,
  ComposeVolume,
  ListOrDict
} from 'perry/container';

// ============ Operation Result Types ============

/**
 * Result of an exec call inside a container.
 */
export interface ExecResult {
  stdout: string;
  stderr: string;
}

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
  /** Remove orphaned containers */
  removeOrphans?: boolean;
  /** Services to remove (empty = all) */
  services?: string[];
}

export interface LogsOptions {
  /** Follow log output */
  follow?: boolean;
  /** Number of lines to show from the end */
  tail?: number;
  /** Show timestamps */
  timestamps?: boolean;
}

export interface ExecOptions {
  /** User context */
  user?: string;
  /** Working directory */
  workdir?: string;
  /** Additional environment variables */
  env?: Record<string, string>;
}

export interface ConfigOptions {
  /** Output format: "yaml" | "json" */
  format?: "yaml" | "json";
}

// ============ API Functions ============

/**
 * Bring up services defined in a compose file.
 *
 * @param file - Path to compose file (default: "compose.yaml")
 * @param options - Up options
 */
export function up(file?: string, options?: UpOptions): Promise<void>;

/**
 * Stop and remove services.
 *
 * @param file - Path to compose file
 * @param options - Down options
 */
export function down(file?: string, options?: DownOptions): Promise<void>;

/**
 * List service statuses.
 *
 * @param file - Path to compose file
 * @returns Array of ContainerInfo entries
 */
export function ps(file?: string): Promise<ContainerInfo[]>;

/**
 * Get logs from services.
 *
 * @param file - Path to compose file
 * @param services - Services to get logs from (empty = all)
 * @param options - Log options
 * @returns ContainerLogs object
 */
export function logs(
  file?: string,
  services?: string[],
  options?: LogsOptions
): Promise<ContainerLogs>;

/**
 * Execute a command in a running service container.
 *
 * @param file - Path to compose file
 * @param service - Service name
 * @param cmd - Command and arguments to execute
 * @param options - Exec options
 */
export function exec(
  file: string,
  service: string,
  cmd: string[],
  options?: ExecOptions
): Promise<ContainerLogs>;

/**
 * Validate and display the parsed compose configuration.
 *
 * @param file - Path to compose file
 * @param options - Config options
 * @returns Validated configuration as YAML or JSON string
 */
export function config(file?: string, options?: ConfigOptions): Promise<string>;

/**
 * Start existing stopped services (does not create new containers).
 *
 * @param file - Path to compose file
 * @param services - Services to start (empty = all)
 */
export function start(file?: string, services?: string[]): Promise<void>;

/**
 * Stop running services (does not remove containers).
 *
 * @param file - Path to compose file
 * @param services - Services to stop (empty = all)
 */
export function stop(file?: string, services?: string[]): Promise<void>;

/**
 * Restart services.
 *
 * @param file - Path to compose file
 * @param services - Services to restart (empty = all)
 */
export function restart(file?: string, services?: string[]): Promise<void>;
