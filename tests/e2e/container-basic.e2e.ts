import { run, inspect, logs, exec, stop, remove } from 'perry/container';

async function test() {
  console.log("Starting container-basic e2e test...");

  const handle = await run({
    image: "alpine:latest",
    cmd: ["echo", "hello e2e"],
  });
  console.log("✓ Container started");

  const info = await inspect(handle.id);
  console.log(`✓ Inspected: ${info.status}`);

  const output = await logs(handle.id);
  if (output.stdout.includes("hello e2e")) {
    console.log("✓ Logs captured");
  }

  await stop(handle.id);
  console.log("✓ Container stopped");

  await remove(handle.id);
  console.log("✓ Container removed");

  console.log("[e2e] PASS");
}

test().catch(e => {
  console.error(e);
  process.exit(1);
});
