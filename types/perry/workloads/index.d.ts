/**
 * perry/workloads — Primary API for managing distributed execution DAGs
 *
 * @module perry/workloads
 */

import { ContainerInfo, ContainerLogs } from 'perry/container';

export interface WorkloadNodeConfig {
  /** Container image */
  image: string;
  /** Port mappings, e.g. ["8080:80"] */
  ports?: string[];
  /** Environment variables */
  env?: Record<string, string>;
  /** Dependencies on other nodes */
  dependsOn?: WorkloadNode[];
  /** Explicit runtime selection */
  runtime?: WorkloadRuntime;
  /** Security and isolation policy */
  policy?: WorkloadPolicy;
}

export interface WorkloadNode {
  name: string;
  config: WorkloadNodeConfig;
  /** Returns host:port for the given container port */
  endpoint(port: string | number): string;
  /** Returns container IP address */
  ip(): string;
  /** Returns internal URL, e.g. http://<ip>:<port> */
  internalUrl(): string;
}

export type WorkloadRuntime = "oci" | "microvm" | "wasm" | "auto";

export interface WorkloadPolicy {
  noNetwork?: boolean;
  readOnlyRoot?: boolean;
}

export interface WorkloadGraph {
  name: string;
  nodes: Record<string, WorkloadNode>;
  /** Get logs from a specific node */
  logs(nodeName: string): Promise<ContainerLogs>;
  /** Get current status of all nodes */
  status(): Promise<Record<string, string>>;
}

export interface RunGraphOptions {
  /** Execution strategy */
  strategy?: "sequential" | "max-parallel" | "dependency-aware" | "parallel-safe";
  /** What to do on failure */
  onFailure?: "rollback-all" | "partial-continue" | "halt-graph";
}

/**
 * Define a workload graph.
 */
export function graph(
  name: string,
  builder: (g: GraphBuilder) => Record<string, WorkloadNode>
): WorkloadGraph;

export interface GraphBuilder {
  node(name: string, config: WorkloadNodeConfig): WorkloadNode;
}

/**
 * Execute a workload graph.
 */
export function runGraph(
  app: WorkloadGraph,
  options?: RunGraphOptions
): Promise<WorkloadHandle>;

export interface WorkloadHandle {
  /** Stop and remove all resources in the graph */
  down(options?: { force?: boolean }): Promise<void>;
  /** Get current status of all nodes */
  status(): Promise<Record<string, string>>;
  /** Get logs from a node */
  logs(nodeName: string, options?: { tail?: number }): Promise<ContainerLogs>;
  /** Execute a command in a node */
  exec(nodeName: string, cmd: string[]): Promise<ContainerLogs>;
  /** Get container info for all nodes */
  ps(): Promise<ContainerInfo[]>;
}

export const runtime: {
  oci(): WorkloadRuntime;
  microvm(): WorkloadRuntime;
  wasm(): WorkloadRuntime;
  auto(): WorkloadRuntime;
};

export const policy: {
  default(): WorkloadPolicy;
  isolated(): WorkloadPolicy;
  hardened(config?: WorkloadPolicy): WorkloadPolicy;
  untrusted(): WorkloadPolicy;
};

/**
 * Inspect the state of a running graph.
 */
export function inspectGraph(app: WorkloadGraph): Promise<Record<string, string>>;
