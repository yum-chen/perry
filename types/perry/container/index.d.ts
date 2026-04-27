// Type declarations for perry/container — Perry's OCI container management module
// These types are auto-written by `perry init` / `perry types` so IDEs
// and tsc can resolve `import { ... } from "perry/container"`.

// ---------------------------------------------------------------------------
// Container Lifecycle
// ---------------------------------------------------------------------------

/**
 * Configuration for a single container.
 */
export interface ContainerSpec {
  /** Container image (required) */
  image: string;
  /** Container name (optional) */
  name?: string;
  /** Port mappings (e.g., ["8080:80"]) */
  ports?: string[];
  /** Volume mounts (e.g., ["/host/path:/container/path:ro"]) */
  volumes?: string[];
  /** Environment variables */
  env?: Record<string, string>;
  /** Command to run (overrides image CMD) */
  cmd?: string[];
  /** Entrypoint (overrides image ENTRYPOINT) */
  entrypoint?: string[];
  /** Network to attach to */
  network?: string;
  /** Remove container on exit */
  rm?: boolean;
  /** Read-only root filesystem */
  readOnly?: boolean;
}

/**
 * Handle to a container instance.
 */
export interface ContainerHandle {
  /** Container ID */
  id: string;
  /** Container name (if specified) */
  name?: string;
}

/**
 * Run a container from the given spec.
 * @param spec Container configuration
 * @returns Promise resolving to ContainerHandle
 */
export function run(spec: ContainerSpec): Promise<ContainerHandle>;

/**
 * Build a container image from a spec.
 * @param spec Build configuration
 * @param imageName Name to tag the built image with
 */
export function build(
  spec: {
    context?: string;
    dockerfile?: string;
    args?: Record<string, string>;
  },
  imageName: string
): Promise<void>;

/**
 * Create a container from the given spec without starting it.
 * @param spec Container configuration
 * @returns Promise resolving to ContainerHandle
 */
export function create(spec: ContainerSpec): Promise<ContainerHandle>;

/**
 * Start a previously created container.
 * @param id Container ID or name
 * @returns Promise resolving when container is started
 */
export function start(id: string): Promise<void>;

/**
 * Stop a running container.
 * @param id Container ID or name
 * @param timeout Timeout in seconds before force-terminating (default: 10)
 * @returns Promise resolving when container is stopped
 */
export function stop(id: string, timeout?: number): Promise<void>;

/**
 * Remove a container.
 * @param id Container ID or name
 * @param force If true, stop and remove a running container
 * @returns Promise resolving when container is removed
 */
export function remove(id: string, force?: boolean): Promise<void>;

// ---------------------------------------------------------------------------
// Container Inspection and Listing
// ---------------------------------------------------------------------------

/**
 * Information about a container.
 */
export interface ContainerInfo {
  /** Container ID */
  id: string;
  /** Container name */
  name: string;
  /** Image reference */
  image: string;
  /** Container status (e.g., "running", "exited") */
  status: string;
  /** Port mappings */
  ports: string[];
  /** Creation timestamp (ISO 8601) */
  created: string;
}

/**
 * List containers.
 * @param all If true, include stopped containers
 * @returns Promise resolving to array of ContainerInfo
 */
export function list(all?: boolean): Promise<ContainerInfo[]>;

/**
 * Inspect a container.
 * @param id Container ID or name
 * @returns Promise resolving to ContainerInfo
 */
export function inspect(id: string): Promise<ContainerInfo>;

// ---------------------------------------------------------------------------
// Container Logs and Exec
// ---------------------------------------------------------------------------

/**
 * Logs captured from a container.
 */
export interface ContainerLogs {
  /** Standard output */
  stdout: string;
  /** Standard error */
  stderr: string;
}

/**
 * Get logs from a container.
 * @param id Container ID or name
 * @param tail Number of lines to return from the end
 * @returns Promise resolving to ContainerLogs
 */
export function logs(id: string, tail?: number): Promise<ContainerLogs>;

/**
 * Execute a command in a running container.
 * @param id Container ID or name
 * @param cmd Command to execute
 * @param options Options for exec
 * @returns Promise resolving to ContainerLogs
 */
export function exec(
  id: string,
  cmd: string[],
  options?: {
    /** Environment variables */
    env?: Record<string, string>;
    /** Working directory */
    workdir?: string;
  }
): Promise<ContainerLogs>;

// ---------------------------------------------------------------------------
// Image Management
// ---------------------------------------------------------------------------

/**
 * Information about a container image.
 */
export interface ImageInfo {
  /** Image ID */
  id: string;
  /** Repository name */
  repository: string;
  /** Image tag */
  tag: string;
  /** Image size in bytes */
  size: number;
  /** Creation timestamp (ISO 8601) */
  created: string;
}

/**
 * Pull a container image from a registry.
 * @param reference Image reference (e.g., "alpine:latest")
 * @returns Promise resolving when image is pulled
 */
export function pullImage(reference: string): Promise<void>;

/**
 * List images in the local cache.
 * @returns Promise resolving to array of ImageInfo
 */
export function listImages(): Promise<ImageInfo[]>;

/**
 * Remove an image from the local cache.
 * @param reference Image reference
 * @param force If true, remove even if image is in use
 * @returns Promise resolving when image is removed
 */
export function removeImage(reference: string, force?: boolean): Promise<void>;

// ---------------------------------------------------------------------------
// Compose (Multi-Container Orchestration)
// ---------------------------------------------------------------------------

export type ListOrDict = Record<string, string | number | boolean | null> | string[];

/**
 * Multi-container application specification.
 */
export interface ComposeSpec {
  /** Project name (optional) */
  name?: string;
  /** Compose file version (deprecated) */
  version?: string;
  /** Service definitions */
  services: Record<string, ComposeService>;
  /** Network definitions */
  networks?: Record<string, ComposeNetwork | null>;
  /** Volume definitions */
  volumes?: Record<string, ComposeVolume | null>;
  /** Secret definitions */
  secrets?: Record<string, any>;
  /** Config definitions */
  configs?: Record<string, any>;
}

/**
 * Service definition in Compose.
 */
export interface ComposeService {
  /** Container image */
  image?: string;
  /** Build configuration */
  build?: string | {
    context?: string;
    dockerfile?: string;
    args?: ListOrDict;
    labels?: ListOrDict;
    target?: string;
  };
  /** Command to run */
  command?: string | string[];
  /** Entrypoint */
  entrypoint?: string | string[];
  /** Environment variables */
  environment?: ListOrDict;
  /** Environment files */
  env_file?: string | string[];
  /** Port mappings */
  ports?: Array<string | number | {
    target: number | string;
    published?: number | string;
    protocol?: string;
    mode?: string;
  }>;
  /** Volume mounts */
  volumes?: Array<string | {
    type: "bind" | "volume" | "tmpfs" | "cluster" | "npipe" | "image";
    source?: string;
    target?: string;
    read_only?: boolean;
  }>;
  /** Networks to attach to */
  networks?: string[] | Record<string, any>;
  /** Service dependencies */
  depends_on?: string[] | Record<string, {
    condition: "service_started" | "service_healthy" | "service_completed_successfully";
    required?: boolean;
  }>;
  /** Restart policy */
  restart?: string;
  /** Healthcheck configuration */
  healthcheck?: {
    test: string | string[];
    interval?: string;
    timeout?: string;
    retries?: number;
    start_period?: string;
  };
  /** Explicit container name */
  container_name?: string;
  /** Labels */
  labels?: ListOrDict;
  /** Hostname */
  hostname?: string;
  /** User */
  user?: string;
  /** Working directory */
  working_dir?: string;
  /** Privileged mode */
  privileged?: boolean;
  /** Read-only root filesystem */
  read_only?: boolean;
}

/**
 * Network configuration.
 */
export interface ComposeNetwork {
  driver?: string;
  driver_opts?: Record<string, string>;
  external?: boolean;
  internal?: boolean;
  name?: string;
  labels?: ListOrDict;
}

/**
 * Volume configuration.
 */
export interface ComposeVolume {
  driver?: string;
  driver_opts?: Record<string, string>;
  external?: boolean;
  name?: string;
  labels?: ListOrDict;
}

/**
 * Handle to a Compose stack.
 */
export interface ComposeHandle {
  /** Stop and remove all resources in the stack */
  down(volumes?: boolean): Promise<void>;

  /** Get container info for all services in the stack */
  ps(): Promise<ContainerInfo[]>;

  /** Get logs from the stack */
  logs(service?: string, tail?: number): Promise<ContainerLogs>;

  /** Execute a command in a service container */
  exec(service: string, cmd: string[]): Promise<ContainerLogs>;
}

/**
 * Bring up a Compose stack.
 * @param spec Compose specification
 * @returns Promise resolving to ComposeHandle
 */
export function composeUp(spec: ComposeSpec): Promise<ComposeHandle>;

// ---------------------------------------------------------------------------
// Platform Information
// ---------------------------------------------------------------------------

/**
 * Information about a detected container backend.
 */
export interface BackendInfo {
  name: string;
  available: boolean;
  reason?: string;
  version?: string;
}

/**
 * Get the name of the container backend being used.
 * @returns e.g. "apple/container", "orbstack", "podman", "docker"
 */
export function getBackend(): string;

/**
 * Probe for all available container backends.
 * @returns Array of information about probed backends
 */
export function detectBackend(): Promise<BackendInfo[]>;
