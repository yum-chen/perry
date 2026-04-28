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
  /** Port mappings (e.g., "8080:80") */
  ports?: string[];
  /** Volume mounts (e.g., "/host/path:/container/path:ro") */
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
 *
 * @param all If true, include stopped containers
 * @returns Promise resolving to a **JSON-encoded** `ContainerInfo[]`
 *   string — call `JSON.parse(await list(all))` to recover the array.
 *   The string-shape return reflects Perry's current FFI contract;
 *   server-side array-materialization is a planned ergonomics task.
 */
export function list(all?: boolean): Promise<string>;

/**
 * Inspect a container.
 *
 * @param id Container ID or name
 * @returns Promise resolving to a **JSON-encoded** `ContainerInfo`
 *   string. Call `JSON.parse(await inspect(id))` to recover the object.
 */
export function inspect(id: string): Promise<string>;

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
 *
 * @param id Container ID or name
 * @param options Options for logs (`tail`: number of trailing lines)
 * @returns Promise resolving to a **JSON-encoded** `ContainerLogs`
 *   string. Call `JSON.parse(await logs(id))` to recover
 *   `{ stdout, stderr }`.
 */
export function logs(
  id: string,
  options?: {
    /** Number of lines to return from the end (negative = no limit) */
    tail?: number;
  }
): Promise<string>;

/**
 * Execute a command in a running container.
 *
 * @param id Container ID or name
 * @param cmd Command to execute
 * @param options Options for exec
 * @returns Promise resolving to a **JSON-encoded** `ContainerLogs`
 *   string. Call `JSON.parse(await exec(id, cmd))` to recover
 *   `{ stdout, stderr }`.
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
): Promise<string>;

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
 * @param reference Image reference (e.g., "alpine:latest", "cgr.dev/chainguard/alpine-base@sha256:...")
 * @returns Promise resolving when image is pulled
 */
export function pullImage(reference: string): Promise<void>;

/**
 * List images in the local cache.
 *
 * @returns Promise resolving to a **JSON-encoded** `ImageInfo[]` string.
 *   Call `JSON.parse(await listImages())` to recover the array.
 */
export function listImages(): Promise<string>;

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

/**
 * Multi-container application specification.
 */
export interface ComposeSpec {
  /** Compose file version */
  version?: string;
  /** Service definitions */
  services: Record<string, ComposeService>;
  /** Network definitions */
  networks?: Record<string, ComposeNetwork>;
  /** Volume definitions */
  volumes?: Record<string, ComposeVolume>;
}

/**
 * Service definition in Compose.
 *
 * Mirrors `perry/compose`'s `Service` interface — kept in sync via the
 * `types_compose_service_keys_in_sync` invariant test in
 * `crates/perry-stdlib/tests/container_workspace_invariants.rs`.
 */
export interface ComposeService {
  /** Container image */
  image?: string;
  /** Explicit container name (required for cross-service DNS today — see
   *  `docs/src/container/networking.md#cross-service-dns`) */
  container_name?: string;
  /** Build configuration */
  build?: {
    /** Build context directory */
    context?: string;
    /** Dockerfile path (relative to context) */
    dockerfile?: string;
    /** Build-time arguments */
    args?: Record<string, string>;
    /** Labels to add to the built image */
    labels?: Record<string, string>;
    /** Build target stage */
    target?: string;
    /** Network to use during build */
    network?: string;
  };
  /** Command to run */
  command?: string | string[];
  /** Override container entrypoint */
  entrypoint?: string | string[];
  /** Environment variables */
  environment?: Record<string, string> | string[];
  /** Container labels */
  labels?: Record<string, string>;
  /** Port mappings, e.g. `"8080:80"` */
  ports?: string[];
  /** Volume mounts: named (`"forgejo-data:/data"`) or bind
   *  (`"./config:/app/config:ro"`) */
  volumes?: string[];
  /** Networks to attach to */
  networks?: string[];
  /** Service dependencies — array form OR map form with conditions
   *  (`{ db: { condition: "service_healthy" } }`) */
  depends_on?: string[] | Record<string, { condition?: string }>;
  /** Restart policy */
  restart?: "no" | "always" | "on-failure" | "unless-stopped";
  /** Healthcheck configuration */
  healthcheck?: ComposeHealthcheck;
  /** UID / username the container's processes run as (`"1000"` / `"git"`) */
  user?: string;
  /** Working directory inside the container */
  working_dir?: string;
  /** Read-only root filesystem */
  read_only?: boolean;
  /** Privileged mode — use sparingly */
  privileged?: boolean;
  /** Linux capabilities to add (e.g. `["NET_ADMIN"]`) */
  cap_add?: string[];
  /** Linux capabilities to drop (e.g. `["ALL"]`) */
  cap_drop?: string[];
}

/**
 * Healthcheck configuration (compose-spec § service.healthcheck).
 *
 * `interval`, `timeout`, `start_period` accept Go-duration strings
 * (`"30s"`, `"2m"`, `"1h30m"`); the OCI runtime parses them.
 *
 * `test` is either a `["NONE"]` sentinel that disables the image's own
 * healthcheck, or `["CMD", "<cmd>", "<arg>", ...]` /
 * `["CMD-SHELL", "<shell-line>"]`.
 */
export interface ComposeHealthcheck {
  /** Test command (string or array) */
  test: string | string[];
  /** Check interval (e.g., `"30s"`) */
  interval?: string;
  /** Timeout (e.g., `"10s"`) */
  timeout?: string;
  /** Number of retries before unhealthy */
  retries?: number;
  /** Startup grace period (e.g., `"40s"`) */
  start_period?: string;
  /** Disable the image's built-in healthcheck */
  disable?: boolean;
}

/**
 * Network configuration.
 */
export interface ComposeNetwork {
  /** Network driver (`"bridge"` is the default; `"overlay"` for swarm) */
  driver?: string;
  /** External: don't create — assume the network already exists */
  external?: boolean;
  /** Override the network's runtime name */
  name?: string;
  /**
   * Internal-only network: containers attached can only reach other
   * containers on the same network — no external bridge / routing,
   * no host-network egress. Use this for the database side of a
   * web/db split so postgres etc. can't be reached from the host.
   */
  internal?: boolean;
  /** Driver-specific options */
  driver_opts?: Record<string, string>;
  /** Labels */
  labels?: Record<string, string>;
}

/**
 * Volume configuration.
 */
export interface ComposeVolume {
  /** Volume driver */
  driver?: string;
  /** External: don't create — assume the volume already exists */
  external?: boolean;
  /** Override the volume's runtime name */
  name?: string;
  /** Driver-specific options */
  driver_opts?: Record<string, string>;
  /** Labels */
  labels?: Record<string, string>;
}

/**
 * Bring up a Compose stack.
 * @param spec Compose specification
 * @returns Promise resolving to the stack ID (number)
 */
export function composeUp(spec: ComposeSpec): Promise<number>;

// ---------------------------------------------------------------------------
// Cleanup / teardown helpers (no ComposeHandle required)
// ---------------------------------------------------------------------------

/**
 * Summary returned by `downByProject` / `downAll`. JSON-encoded across
 * the FFI boundary — call `JSON.parse(await downByProject(...))` to
 * get this typed shape.
 */
export interface CleanupReport {
  containers_removed: number;
  networks_removed: number;
  volumes_removed: number;
  /** Per-resource error messages; cleanup is best-effort */
  errors: string[];
}

/**
 * Options for `downByProject` / `downAll`.
 */
export interface CleanupOptions {
  /** Drop named volumes (default false — preserves data). */
  volumes?: boolean;
  /** Best-effort prune unused networks (default true). */
  networks?: boolean;
}

/**
 * Tear down every container labelled with `perry.compose.project =
 * <project>`, regardless of whether you still hold the original
 * `ComposeHandle`. Useful when:
 *
 *   - The original process crashed without calling `down()`.
 *   - You're in a different process / session and don't have the
 *     in-memory handle anymore.
 *   - You're cleaning up between dev iterations.
 *
 * @returns Promise resolving to a JSON-encoded `CleanupReport` string.
 *   Call `JSON.parse(await downByProject('myapp'))` to parse it.
 */
export function downByProject(
  project: string,
  options?: CleanupOptions,
): Promise<string>;

/**
 * Tear down EVERY Perry-managed container on this host. **Use
 * sparingly** — this stops every stack the user has ever brought up
 * via `perry/compose`, regardless of which terminal session it's
 * running in. Returns the same JSON-encoded `CleanupReport` shape as
 * `downByProject`.
 */
export function downAll(options?: CleanupOptions): Promise<string>;

/**
 * Idempotent single-container removal. Stop + force-remove if the
 * container exists; treat NotFound as success. Returns `"true"` if
 * the container was found and removed, `"false"` if it didn't exist.
 *
 * Useful in test cleanup paths and recovery scripts where you're not
 * sure whether a container was ever started.
 */
export function removeIfExists(
  idOrName: string,
  force?: boolean,
): Promise<string>;

// ---------------------------------------------------------------------------
// Platform Information
// ---------------------------------------------------------------------------

/**
 * Get the name of the container backend being used.
 * @returns "apple/container" on macOS/iOS, "podman" on all other platforms
 */
export function getBackend(): string;

/**
 * Detected container runtime metadata. Returned by `detectBackend()` after
 * `JSON.parse`'ing the result.
 */
export interface BackendInfo {
  /** Canonical backend name (e.g. `"docker"`, `"podman"`, `"apple/container"`) */
  name: string;
  /** Whether the backend was successfully probed and is ready to use */
  available: boolean;
  /** Failure reason if `available === false` (empty string when available) */
  reason: string;
  /** Optional CLI version string when the backend is available */
  version?: string;
}

/**
 * Probe for available container runtimes and return details about each.
 *
 * @returns Promise resolving to a **JSON-encoded** `BackendInfo[]`
 *   string. Call `JSON.parse(await detectBackend())` to recover the
 *   typed array. Each entry includes `name`, `available`, `reason`
 *   (failure reason if any), and an optional `version` field.
 *   Example:
 *
 *   ```ts
 *   const probed = JSON.parse(await detectBackend()) as BackendInfo[];
 *   const live   = probed.filter(b => b.available);
 *   ```
 */
export function detectBackend(): Promise<string>;
