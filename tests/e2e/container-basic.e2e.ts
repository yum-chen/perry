import { run, inspect, stop, remove } from 'perry/container';

console.log('--- Start E2E Basic Container Test ---');

try {
  const handle = await run({
    image: 'alpine',
    cmd: ['echo', 'hello-perry'],
  });
  console.log('✓ Container started');

  const info = await inspect(handle.id);
  console.log(`✓ Container status: ${info.status}`);

  await stop(handle.id);
  console.log('✓ Container stopped');

  await remove(handle.id);
  console.log('✓ Container removed');

  console.log('[e2e] PASS');
} catch (e) {
  console.error('E2E Test Failed:', e);
  process.exit(1);
}
