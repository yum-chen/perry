import { graph, node, runGraph } from 'perry/workloads';

async function test() {
  console.log("Starting workloads-graph e2e test...");

  const app = graph("test-app", (g) => {
    const web = g.node("web", {
      image: "alpine:latest",
      cmd: ["sleep", "3600"]
    });
    return { web };
  });

  const handle = await runGraph(app);
  console.log("✓ Graph handle acquired");

  const status = await handle.status();
  if (status.healthy) {
    console.log("✓ Graph is healthy");
  }

  await handle.down();
  console.log("✓ Graph stopped");

  console.log("[e2e] PASS");
}

test().catch(e => {
  console.error(e);
  process.exit(1);
});
