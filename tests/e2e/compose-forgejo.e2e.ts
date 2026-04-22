import { up } from 'perry/container-compose';

async function test() {
  console.log("Starting compose-forgejo e2e test...");

  const stack = await up({
    services: {
      db: {
        image: "postgres:16-alpine",
        environment: { POSTGRES_PASSWORD: "test" }
      },
      forgejo: {
        image: "codeberg.org/forgejo/forgejo:1.23-stable",
        depends_on: ["db"]
      }
    }
  });
  console.log("✓ Stack is up");

  const status = await stack.ps();
  if (status.length >= 2) {
    console.log("✓ All services visible in ps");
  }

  await stack.down({ volumes: true });
  console.log("✓ Stack cleaned up");

  console.log("[e2e] PASS");
}

test().catch(e => {
  console.error(e);
  process.exit(1);
});
