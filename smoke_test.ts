import { run, list } from 'perry/container';
import { up, ps } from 'perry/compose';
import { graph, node, runGraph } from 'perry/workloads';

async function test() {
  const containerId = await run({
    image: 'alpine',
    cmd: ['echo', 'hello']
  });
  console.log(`Started container: ${containerId}`);

  const containers = await list({ all: true });
  console.log(`Found ${containers.length} containers`);

  const stack = await up({
    services: {
      web: { image: 'nginx' }
    }
  });
  console.log(`Started stack: ${stack}`);

  const myGraph = graph("test-app", (g) => {
    const db = g.node("db", { image: "postgres" });
    const api = g.node("api", { image: "my-api", dependsOn: [db] });
    return { db, api };
  });

  const handle = await runGraph(myGraph);
  console.log("Graph running");
}
