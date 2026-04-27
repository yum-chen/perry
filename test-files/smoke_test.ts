import { run, list, composeUp } from 'perry/container';
import { graph, node, runGraph } from 'perry/workloads';

async function main() {
    const c = await run({ image: 'alpine' });
    console.log(c.id);

    const app = graph("test", (g) => {
        const db = g.node("db", { image: "postgres" });
        return { db };
    });
    await runGraph(app);
}
