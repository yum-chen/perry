export interface ContainerInfo {
  id: string;
  name: string;
  image: string;
  status: string;
  ports: string[];
  created: string;
}

export interface ContainerLogs {
  stdout: string;
  stderr: string;
}

export interface ComposeSpec {
  name?: string;
  version?: string;
  services: Record<string, any>;
  networks?: Record<string, any>;
  volumes?: Record<string, any>;
}

export type ComposeHandle = number;

export function up(spec: ComposeSpec | string): Promise<ComposeHandle>;
export function down(handle: ComposeHandle, opts?: { volumes?: boolean }): Promise<void>;
export function ps(handle: ComposeHandle): Promise<ContainerInfo[]>;
export function logs(handle: ComposeHandle, service: string, tail?: number): Promise<ContainerLogs>;
export function exec(handle: ComposeHandle, service: string, cmd: string[]): Promise<ContainerLogs>;
export function config(handle: ComposeHandle): Promise<string>;
export function start(handle: ComposeHandle, services?: string[]): Promise<void>;
export function stop(handle: ComposeHandle, services?: string[]): Promise<void>;
export function restart(handle: ComposeHandle, services?: string[]): Promise<void>;
