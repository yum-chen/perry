/**
 * perry-container-compose — Production Forgejo Stack Example
 *
 * This example demonstrates a production-ready Forgejo (self-hosted Git service)
 * deployment using Perry's container-compose API.
 *
 * architecture:
 * - forgejo:  Main Forgejo application
 * - postgres: PostgreSQL database for Forgejo data
 *
 * Run: npx tsx main.ts
 */

import { pullImage } from 'perry/container';
import { composeUp, getBackend } from 'perry/compose';

// ──────────────────────────────────────────────────────────────
// Configuration & DRY Constants
// ──────────────────────────────────────────────────────────────

const POSTGRES_IMAGE = 'postgres:16-alpine';
const FORGEJO_IMAGE = 'codeberg.org/forgejo/forgejo:1.23-stable';

// ──────────────────────────────────────────────────────────────
// Verify Backend Support
// ──────────────────────────────────────────────────────────────

const backend = getBackend();
console.log(`🔧 Using container backend: ${backend}\n`);

// ──────────────────────────────────────────────────────────────
// Explicitly Pull Images
// ──────────────────────────────────────────────────────────────

console.log('🚚 Pulling images...');
await pullImage(POSTGRES_IMAGE);
await pullImage(FORGEJO_IMAGE);

// ──────────────────────────────────────────────────────────────
// Forgejo Production Stack Configuration
// ──────────────────────────────────────────────────────────────

// Stack name for tracking
const stack = await composeUp({
  version: '3.8',
  services: {
    postgres: {
      image: POSTGRES_IMAGE,
      restart: 'always',
      environment: {
        POSTGRES_USER: '${FORGEJO_DB_USER:-forgejo}',
        POSTGRES_PASSWORD: '${FORGEJO_DB_PASSWORD:-changeme}',
        POSTGRES_DB: '${FORGEJO_DB_NAME:-forgejo}',
      },
      volumes: ['forgejo-pgdata:/var/lib/postgresql/data'],
      ports: ['5432:5432'],
      networks: ['forgejo-network'],
    },
    forgejo: {
      image: FORGEJO_IMAGE,
      restart: 'always',
      dependsOn: ['postgres'],
      environment: {
        // Database configuration
        FORGEJO__database__HOST: '${FORGEJO_DB_HOST:-postgres:5432}',
        FORGEJO__database__name: '${FORGEJO_DB_NAME:-forgejo}',
        FORGEJO__database__user: '${FORGEJO_DB_USER:-forgejo}',
        FORGEJO__database__passwd: '${FORGEJO_DB_PASSWORD:-changeme}',
        // URL configuration (adjust for your setup)
        FORGEJO__server__PROTOCOL: '${FORGEJO_PROTOCOL:-http}',
        FORGEJO__server__DOMAIN: '${FORGEJO_DOMAIN:-localhost}',
        FORGEJO__server__ROOT_URL: '${FORGEJO_ROOT_URL:-http://localhost:3000}',
        // Admin configuration
        FORGEJO__security__INSTALL_LOCK: 'true',
        FORGEJO__service__DISABLE_REGISTRATION: 'false',
        FORGEJO__service__REQUIRE_SIGNIN: 'true',
      },
      volumes: [
        'forgejo-data:/data',
        'forgejo-config:/config',
      ],
      ports: ['3000:3000', '2222:22'],
      networks: ['forgejo-network'],
    },
  },
  networks: {
    'forgejo-network': {
      driver: 'bridge',
    },
  },
  volumes: {
    'forgejo-pgdata': {},
    'forgejo-data': {},
    'forgejo-config': {},
  },
});

// ──────────────────────────────────────────────────────────────
// Verify Stack Status
// ──────────────────────────────────────────────────────────────

console.log('\n🔍 Checking Forgejo stack status...\n');

const statuses = await stack.ps();
console.table(statuses);

// Verify both services are running
const allRunning = statuses.every((s) => s.status === 'running' || s.status.includes('Up'));
if (!allRunning) {
  console.error('❌ Not all services are running!');
  console.log('Logs from forgejo service:');
  const logs = await stack.logs({ service: 'forgejo', tail: 50 });
  console.log(logs.stdout);
  await stack.down({ volumes: true });
  process.exit(1);
}

console.log('✅ Stack is up and running!');

// ──────────────────────────────────────────────────────────────
// Health Check: Verify PostgreSQL is ready
// ──────────────────────────────────────────────────────────────

console.log('\n🏥 Performing health checks...\n');

const postgresHealth = await stack.exec('postgres', [
  'pg_isready',
  '-U',
  'forgejo',
  '-d',
  'forgejo',
]);

if (postgresHealth.stdout.includes('accepting connections')) {
  console.log('✅ PostgreSQL: ready');
} else {
  console.error('❌ PostgreSQL: not ready');
  console.error('stderr:', postgresHealth.stderr);
  await stack.down({ volumes: true });
  process.exit(1);
}

// ──────────────────────────────────────────────────────────────
// Usage Instructions
// ──────────────────────────────────────────────────────────────

console.log(`
─────────────────────────────────────────────────────────────
🎉 Forgejo Stack is Ready!
─────────────────────────────────────────────────────────────

Access URLs:
  - Web UI:  http://localhost:3000
  - SSH:     ssh://localhost:2222

Useful commands:
  # View logs
  await stack.logs({ service: 'forgejo', tail: 100 });

  # Execute command in forgejo container
  await stack.exec('forgejo', ['ls', '/data']);

  # Stop stack (preserves data)
  await stack.down();

  # Stop stack and remove volumes (destroys all data)
  await stack.down({ volumes: true });

─────────────────────────────────────────────────────────────
`);

// ──────────────────────────────────────────────────────────────
// Cleanup on SIGINT/SIGTERM
// ──────────────────────────────────────────────────────────────

const cleanup = async () => {
  console.log('\n🧹 Cleaning up stack...');
  await stack.down({ volumes: true });
  console.log('✅ Cleanup complete');
  process.exit(0);
};

process.on('SIGINT', cleanup);
process.on('SIGTERM', cleanup);
