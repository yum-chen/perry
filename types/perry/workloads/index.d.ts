/**
 * perry/workloads — Workload-graph-centric orchestration for Perry.
 *
 * This module provides a high-level API for declaring and running multi-node
 * workloads as typed directed acyclic graphs (DAGs).
 *
 * @module perry/workloads
 */

import { ContainerLogs } from "perry/container";

// ============ Runtime & Policy Specs ============

/**
 * Explicit runtime selection for a workload node.
 */
export type RuntimeSpec =
  | { type: "oci" }
  | { type: "microvm"; config?: any }
  | { type: "wasm"; module?: string }
  | { type: "auto" };

/**
 * Helper for selecting runtime.
 */
export const runtime: {
  oci(): RuntimeSpec;
  microvm(config?: any): RuntimeSpec;
  wasm(module?: string): RuntimeSpec;
  auto(): RuntimeSpec;
};

/**
 * Isolation policy for a workload node.
 */
export interface PolicySpec {
  tier: "default" | "isolated" | "hardened" | "untrusted";
  noNetwork?: boolean;
  readOnlyRoot?: boolean;
  seccomp?: boolean;
}

/**
 * Helper for selecting policy.
 */
export const policy: {
  default(): PolicySpec;
  isolated(): PolicySpec;
  hardened(opts?: { noNetwork?: boolean; readOnlyRoot?: boolean; seccomp?: boolean }): PolicySpec;
  untrusted(): PolicySpec;
};

// ============ Graph Definition ============

/**
 * A reference to another node's address, resolved after the graph starts.
 */
export interface WorkloadRef {
  nodeId: string;
  projection: "endpoint" | "ip" | "internalUrl";
  port?: string;
}

/**
 * A single node in a workload graph.
 */
export interface WorkloadNode {
  id: string;
  name: string;
  image?: string;
  resources?: { cpu?: string; memory?: string };
  ports?: string[];
  env?: Record<string, string | WorkloadRef>;
  dependsOn?: string[]; // resolved node IDs
  runtime: RuntimeSpec;
  policy: PolicySpec;

  /** Resolve to this node's endpoint address at the given port */
  endpoint(port: string): WorkloadRef;
  /** Resolve to this node's IP address */
  ip(): WorkloadRef;
  /** Resolve to a default HTTP URL for this node */
  internalUrl(): WorkloadRef;
}

/**
 * Options for a graph node.
 */
export interface NodeOptions {
  image?: string;
  resources?: { cpu?: string; memory?: string };
  ports?: string[];
  env?: Record<string, string | WorkloadRef>;
  dependsOn?: WorkloadNode[];
  runtime?: RuntimeSpec;
  policy?: PolicySpec;
}

/**
 * Builder for a workload graph.
 */
export interface GraphBuilder {
  /** Add a new node to the graph */
  node(name: string, options: NodeOptions): WorkloadNode;
}

/**
 * A declared workload graph.
 */
export interface WorkloadGraph {
  name: string;
  nodes: Record<string, WorkloadNode>;
}

/**
 * Declare a named workload graph.
 */
export function graph<T>(
  name: string,
  builder: (g: GraphBuilder) => T
): WorkloadGraph & T;

// ============ Execution & Handles ============

/**
 * Options for running a graph.
 */
export interface RunGraphOptions {
  strategy?: "sequential" | "max-parallel" | "dependency-aware" | "parallel-safe";
  onFailure?: "rollback-all" | "partial-continue" | "halt-graph";
}

/**
 * Current state of a node in the graph.
 */
export type NodeState = "running" | "stopped" | "failed" | "pending" | "unknown";

/**
 * Status snapshot of the entire graph.
 */
export interface GraphStatus {
  nodes: Record<string, NodeState>;
  healthy: boolean;
  errors?: Record<string, string>;
}

/**
 * Metadata for a running node.
 */
export interface NodeInfo {
  nodeId: string;
  name: string;
  containerId?: string;
  state: NodeState;
  image?: string;
}

/**
 * Handle to a running workload graph.
 */
export interface GraphHandle {
  /** Stop and remove the graph */
  down(options?: { volumes?: boolean }): Promise<void>;
  /** Get current status of all nodes */
  status(): Promise<GraphStatus>;
  /** Get the original graph definition */
  graph(): WorkloadGraph;
  /** Get logs from a node */
  logs(node?: string, options?: { tail?: number }): Promise<ContainerLogs>;
  /** Execute a command in a node's container */
  exec(node: string, cmd: string[]): Promise<ContainerLogs>;
  /** List node information */
  ps(): Promise<NodeInfo[]>;
}

/**
 * Start a workload graph.
 */
export function runGraph(
  graph: WorkloadGraph,
  options?: RunGraphOptions
): Promise<GraphHandle>;

/**
 * Inspect a graph's status without starting it.
 */
export function inspectGraph(graph: WorkloadGraph): Promise<GraphStatus>;
