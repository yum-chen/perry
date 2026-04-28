# Containers — Overview

Perry ships a first-class container subsystem that lets a TypeScript program
manage OCI containers and multi-container stacks directly, without shelling
out to `docker compose` or hand-rolling subprocess wrappers. The user-facing
API is split across two TypeScript modules:

| Module | Use case |
|---|---|
| [`perry/container`](./containers.md) | Single-container lifecycle: `run`, `create`, `start`, `stop`, `remove`, `inspect`, `logs`, `exec`, plus image management. |
| [`perry/compose`](./compose.md) | Multi-service orchestration: `up`, `down`, `ps`, `logs`, `exec`, `start`, `stop`, `restart`, `config` — driven by a TS object literal that mirrors the Compose spec. |

Both modules compile to **direct calls into a Rust backend** that talks to
whatever OCI-compatible runtime is on the host. There is no JavaScript
runtime in the loop, no YAML file emitter, no `docker-compose` shell-out:
the spec is a TS object, the engine is in-process, and orchestration logic
(dependency ordering, rollback, healthcheck waits) runs natively.

## Backend auto-detection

You do **not** configure a runtime up-front. On first use, Perry probes a
platform-specific priority list of OCI runtimes (with a 2-second timeout
per candidate) and caches the first one that responds:

| Platform | Probe order |
|---|---|
| **macOS / iOS** | `apple/container` → `orbstack` → `colima` → `rancher-desktop` → `lima` → `podman` → `nerdctl` → `docker` |
| **Linux** | `podman` → `nerdctl` → `docker` |
| **Windows** | `podman` → `nerdctl` → `docker` |

The choices reflect three priorities: platform-native runtimes win
(`apple/container` on macOS, the others on Linux), daemonless / rootless
runtimes (`podman`, `nerdctl`) beat daemon-based ones, and `docker` is
always the last fallback.

The same `ComposeSpec` produces deterministic behavior across every
backend in this list — same project-namespaced names, same DNS
aliases, same `ContainerInfo` shape from `inspect`, with explicit
warnings (or hard failures, opt-in) when a feature like
`privileged: true` can't be honored on the chosen runtime. See
[Cross-Backend Determinism](./determinism.md) for the architecture.

```typescript
{{#include ../../examples/stdlib/container/snippets.ts:backend-detect}}
```

### Overrides

| Variable | Effect |
|---|---|
| `PERRY_CONTAINER_BACKEND=<name>` | Skip auto-detection; force a specific backend. Errors if the named backend isn't probeable. |
| `PERRY_NO_INSTALL_PROMPT=1` | Disable the interactive installer when no backend is found. Defaults to allowed when `stderr` is a TTY. |
| `PERRY_CONTAINER_VERIFY_IMAGES=1` | Run `cosign verify` against every pulled image before use. See [Security](./security.md#image-verification). |
| `PERRY_ALLOW_UNTRUSTED_SHARED_KERNEL=1` | Opt out of the workload-graph requirement that `policy.tier = "untrusted"` runs in a microVM. **Not recommended for actual untrusted code.** |

## Module layout

```text
TypeScript code
    ↓  import { run } from 'perry/container'
    ↓  import { up }  from 'perry/compose'
HIR (perry-hir)        — recognises the import paths as native modules
codegen (perry-codegen)— emits direct calls to FFI symbols (NativeModSig dispatch table)
FFI bridge (perry-stdlib::container)
    ↓
ComposeEngine (perry-container-compose)
    ↓
ContainerBackend trait → CliBackend<P: CliProtocol>  (DockerProtocol / AppleContainerProtocol / LimaProtocol)
    ↓
docker / podman / apple/container / colima / orbstack / lima / nerdctl
```

The split exists so the compiler can stay agnostic about which runtime
will actually execute the spec: HIR + codegen reference symbol *strings*
only, and the runtime backend is swappable without recompilation of user
code.

## Canonical lifecycle

The pattern most production deployments follow is the same as
`docker compose up -d` / `down`:

1. **`up()`** — bring the stack up, return an opaque integer handle, and
   exit when every service is started (`up()` does not block on
   healthchecks; for that, see [Healthchecks &
   readiness](./compose.md#waiting-for-readiness)).
2. **Run a separate readiness probe** (or rely on the in-spec
   `healthcheck` block) to verify the stack is actually serving.
3. **Exit 0**: the containers keep running thanks to docker's daemon
   (`restart: unless-stopped` survives host reboots).
4. **`down(handle)`** later (typically from a separate invocation) to
   tear the stack down. Volumes are preserved by default; pass
   `{ volumes: true }` to also drop them.

Perry's runtime currently does not deliver `process.on('SIGINT', ...)`
handlers to your TS code, so a `Ctrl-C`-tears-down pattern can't be
written today. The example deployments under
[`example-code/forgejo-deployment`](https://github.com/PerryTS/perry/tree/main/example-code/forgejo-deployment)
use the two-invocation pattern (`./forgejo_app` and
`./forgejo_app --down`) instead.

## When to use which module

Reach for **`perry/container`** when:

- You need to run a single utility container (CI helper, build tool,
  database migration runner, capability sandbox) and clean up after it.
- You're building a higher-level abstraction on top of OCI primitives.
- You need fine-grained per-container security knobs (`cap_add`,
  `seccomp`, `read_only`, `user`).

Reach for **`perry/compose`** when:

- You're deploying a multi-service application (web + db, app + cache +
  worker, etc.).
- You need dependency-ordered startup with healthcheck conditions.
- You want named volumes, custom networks, and rollback-on-failure
  semantics.
- You'd otherwise reach for a `docker-compose.yaml` file.

The two modules share a runtime; you can mix them in the same program if
you e.g. use `perry/compose` for the long-running stack and `perry/
container` for one-off tasks against the same containers.

## Where to read next

- [Single-container lifecycle](./containers.md) — every `perry/container`
  call documented with examples.
- [Compose orchestration](./compose.md) — `perry/compose` and the
  `ComposeSpec` shape, including the canonical TS-object pattern.
- [Networking](./networking.md) — networks, the `internal` flag, and
  the cross-service-DNS gotcha (and how to work around it today).
- [Volumes](./volumes.md) — named-vs-bind, preservation across `down()`,
  and the `forgejo-pgdata`-style stable-name pattern.
- [Security](./security.md) — capabilities, image verification with
  cosign, and the workload-graph policy tiers.
- [Production patterns](./production-patterns.md) — case study using
  the [`example-code/forgejo-deployment`](https://github.com/PerryTS/perry/tree/main/example-code/forgejo-deployment)
  example and the gotchas it surfaced.
