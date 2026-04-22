export interface RuntimeSpec {
  type: 'oci' | 'microvm' | 'wasm' | 'auto';
  config?: any;
  module?: string;
}

export interface PolicySpec {
  tier: 'default' | 'isolated' | 'hardened' | 'untrusted';
  noNetwork?: boolean;
  readOnlyRoot?: boolean;
  seccomp?: boolean;
}

export interface WorkloadRef {
  nodeId: string;
  projection: 'endpoint' | 'ip' | 'internalUrl';
  port?: string;
}

export interface WorkloadNode {
  id: string;
  name: string;
  image?: string;
  resources?: { cpu?: string; memory?: string };
  ports?: string[];
  env?: Record<string, string | WorkloadRef>;
  dependsOn?: string[];
  runtime: RuntimeSpec;
  policy: PolicySpec;
}

export interface WorkloadGraph {
  name: string;
  nodes: Record<string, WorkloadNode>;
  edges: Array<{ from: string; to: string }>;
}

export interface NodeInfo {
  nodeId: string;
  name: string;
  containerId?: string;
  state: 'running' | 'stopped' | 'failed' | 'pending' | 'unknown';
  image?: string;
}

export interface ContainerLogs {
  stdout: string;
  stderr: string;
}

export interface GraphStatus {
  nodes: Record<string, string>;
  healthy: boolean;
}

export interface GraphHandle {
  down(opts?: { volumes?: boolean }): Promise<void>;
  status(): Promise<GraphStatus>;
  ps(): Promise<NodeInfo[]>;
  logs(node: string, opts?: { tail?: number }): Promise<ContainerLogs>;
  exec(node: string, cmd: string[]): Promise<ContainerLogs>;
}

export function graph(name: string, builder: (g: any) => Record<string, WorkloadNode>): WorkloadGraph;
export function node(name: string, spec: any): WorkloadNode;
export function runGraph(graph: WorkloadGraph, opts?: any): Promise<GraphHandle>;
export function inspectGraph(graph: WorkloadGraph): Promise<GraphStatus>;

export const runtime: {
  oci(): RuntimeSpec;
  microvm(config?: any): RuntimeSpec;
  wasm(module?: string): RuntimeSpec;
  auto(): RuntimeSpec;
};

export const policy: {
  default(): PolicySpec;
  isolated(): PolicySpec;
  hardened(opts?: any): PolicySpec;
  untrusted(): PolicySpec;
};
