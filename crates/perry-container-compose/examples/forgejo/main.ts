/**
 * perry-container-compose — Production Forgejo Stack Example
 *
 * This example demonstrates a production-ready Forgejo (self-hosted Git service)
 * deployment using Perry's container-compose API.
 *
 * Architecture:
 * - forgejo:  Main Forgejo application
 * - postgres: PostgreSQL database for Forgejo data
 *
 * Features:
 * - Named volumes for persistent data
 * - Custom networks for service isolation
 * - Health checks and restart policies
 * - Environment variable interpolation
 * - Proper port mapping
 *
 * Run: npx tsx crates/perry-container-compose/examples/forgejo/main.ts
 */

import { composeUp, getBackend } from 'perry/container';

async function main() {
  // ──────────────────────────────────────────────────────────────
  // Verify Backend Support
  // ──────────────────────────────────────────────────────────────

  const backend = getBackend();
  console.log(`🔧 Using container backend: ${backend}\n`);

  // ──────────────────────────────────────────────────────────────
  // Forgejo Production Stack Configuration
  // ──────────────────────────────────────────────────────────────

  console.log('🚀 Starting Forgejo production stack...');

  const stack = await composeUp({
    version: '3.8',
    services: {
      postgres: {
        // Use compose-style interpolation for image versions
        image: 'postgres:${POSTGRES_VERSION:-16-alpine}',
        restart: 'always',
        environment: {
          POSTGRES_USER: '${FORGEJO_DB_USER:-forgejo}',
          POSTGRES_PASSWORD: '${FORGEJO_DB_PASSWORD:-changeme}',
          POSTGRES_DB: '${FORGEJO_DB_NAME:-forgejo}',
        },
        volumes: ['forgejo-pgdata:/var/lib/postgresql/data'],
        networks: ['forgejo-network'],
        healthcheck: {
          test: ['CMD-SHELL', 'pg_isready -U ${FORGEJO_DB_USER:-forgejo} -d ${FORGEJO_DB_NAME:-forgejo}'],
          interval: '10s',
          timeout: '5s',
          retries: 5,
        },
      },
      forgejo: {
        image: 'codeberg.org/forgejo/forgejo:${FORGEJO_VERSION:-1.23-stable}',
        restart: 'always',
        depends_on: ['postgres'],
        environment: {
          // Database configuration
          FORGEJO__database__DB_TYPE: 'postgres',
          FORGEJO__database__HOST: 'postgres:5432',
          FORGEJO__database__NAME: '${FORGEJO_DB_NAME:-forgejo}',
          FORGEJO__database__USER: '${FORGEJO_DB_USER:-forgejo}',
          FORGEJO__database__PASSWD: '${FORGEJO_DB_PASSWORD:-changeme}',
          // URL configuration
          FORGEJO__server__PROTOCOL: '${FORGEJO_PROTOCOL:-http}',
          FORGEJO__server__DOMAIN: '${FORGEJO_DOMAIN:-localhost}',
          FORGEJO__server__ROOT_URL: '${FORGEJO_ROOT_URL:-http://localhost:3000}',
          // Security
          FORGEJO__security__INSTALL_LOCK: 'true',
        },
        volumes: [
          'forgejo-data:/data',
          '/etc/timezone:/etc/timezone:ro',
          '/etc/localtime:/etc/localtime:ro',
        ],
        ports: ['3000:3000', '2222:22'],
        networks: ['forgejo-network'],
        healthcheck: {
          test: ['CMD', 'curl', '-f', 'http://localhost:3000/api/healthz'],
          interval: '30s',
          timeout: '10s',
          retries: 3,
          start_period: '20s',
        },
      },
    },
    networks: {
      'forgejo-network': {
        driver: 'bridge',
      },
    },
    volumes: {
      'forgejo-pgdata': {
        driver: 'local',
      },
      'forgejo-data': {
        driver: 'local',
      },
    },
  });

  // ──────────────────────────────────────────────────────────────
  // Verify Stack Status
  // ──────────────────────────────────────────────────────────────

  console.log('\n🔍 Checking service statuses...\n');

  const statuses = await stack.ps();
  console.table(statuses.map(s => ({
    Service: s.name,
    Status: s.status,
    Image: s.image,
    Ports: s.ports.join(', ')
  })));

  // Logical verification: Ensure all services are running
  const allRunning = statuses.every(s => s.status === 'running');
  if (!allRunning) {
    console.error('❌ Error: Not all services are in "running" state.');
    // In a real script, you might want to call stack.down() here if it's a transient failure
    // await stack.down();
    // process.exit(1);
  }

  // ──────────────────────────────────────────────────────────────
  // Health Check Verification
  // ──────────────────────────────────────────────────────────────

  console.log('\n🏥 Performing deep health checks via exec...\n');

  try {
    const pgCheck = await stack.exec('postgres', ['pg_isready', '-U', 'forgejo']);
    if (pgCheck.stdout.includes('accepting connections')) {
      console.log('✅ PostgreSQL is healthy and accepting connections');
    } else {
      console.warn('⚠️ PostgreSQL health check returned unexpected output:', pgCheck.stdout);
    }
  } catch (err) {
    console.error('❌ PostgreSQL health check failed:', err);
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

Default environment:
  FORGEJO_DB_USER=forgejo
  FORGEJO_DB_NAME=forgejo
  FORGEJO_DOMAIN=localhost

Useful commands:
  # View forgejo logs
  await stack.logs({ service: 'forgejo', tail: 50 });

  # Execute a Gitea/Forgejo admin command
  await stack.exec('forgejo', ['forgejo', 'admin', 'user', 'list']);

  # Stop stack
  await stack.down();

─────────────────────────────────────────────────────────────
`);

  // Cleanup on exit
  let cleaningUp = false;
  const cleanup = async () => {
    if (cleaningUp) return;
    cleaningUp = true;
    console.log('\n🧹 Shutting down Forgejo stack...');
    try {
      await stack.down({ volumes: false }); // Keep volumes by default for production safety
      console.log('✅ Stack stopped successfully');
    } catch (err) {
      console.error('❌ Error during shutdown:', err);
    }
    process.exit(0);
  };

  process.on('SIGINT', cleanup);
  process.on('SIGTERM', cleanup);

  console.log('Press Ctrl+C to stop the stack and clean up.');

  // Keep the process alive to handle signals
  setInterval(() => {}, 1000);
}

main().catch((err) => {
  console.error('💥 Fatal error:', err);
  process.exit(1);
});
