import { run, create, start, stop, remove, list, inspect, pullImage, inspectImage, getBackend } from 'perry/container';
import { up, down, ps, logs, exec, config } from 'perry/compose';

/**
 * Integration Test Suite for perry/container and perry/compose
 *
 * Note: These tests require a running container backend (podman or docker).
 */

async function testContainerLifecycle() {
  console.log('--- Testing Container Lifecycle ---');

  const backend = getBackend();
  console.log(`Backend: ${backend}`);

  const image = 'alpine:latest';
  console.log(`Pulling ${image}...`);
  await pullImage(image);

  const info = await inspectImage(image);
  console.log(`Image ID: ${info.id}`);

  console.log('Running ephemeral container...');
  const handle = await run({
    image,
    cmd: ['echo', 'hello perry'],
    rm: true
  });
  console.log(`Container started: ${handle.id}`);

  console.log('Creating persistent container...');
  const persistent = await create({
    image,
    name: 'perry-test-container',
    cmd: ['sleep', '100']
  });

  await start(persistent.id);
  const containerInfo = await inspect(persistent.id);
  console.log(`Status: ${containerInfo.status}`);

  const containers = await list(true);
  console.log(`Total containers: ${containers.length}`);

  await stop(persistent.id);
  await remove(persistent.id);
  console.log('Container removed.');
}

async function testComposeOrchestration() {
  console.log('\n--- Testing Compose Orchestration ---');

  const spec = {
    version: '3.8',
    services: {
      web: {
        image: 'nginx:alpine',
        ports: ['8081:80']
      },
      redis: {
        image: 'redis:alpine'
      }
    }
  };

  console.log('Bringing up stack...');
  const stackId = await composeUp(spec);
  console.log(`Stack ID: ${stackId}`);

  const services = await ps(stackId);
  console.table(services);

  console.log('Executing command in redis...');
  const result = await exec(stackId, 'redis', ['redis-cli', 'ping']);
  console.log(`Redis ping: ${result.stdout.trim()}`);

  const stackConfig = await config(stackId);
  console.log('Resolved config size:', stackConfig.length);

  console.log('Tearing down stack...');
  await down(stackId, { volumes: true });
  console.log('Stack destroyed.');
}

async function runTests() {
  try {
    await testContainerLifecycle();
    await testComposeOrchestration();
    console.log('\n✅ All integration tests passed!');
  } catch (e) {
    console.error('\n❌ Integration test failed:');
    console.error(e);
    process.exit(1);
  }
}

runTests();
