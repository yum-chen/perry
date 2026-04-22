/**
 * perry-container-compose — Production Forgejo Stack Example
 *
 * This example demonstrates a production-ready Forgejo (self-hosted Git service)
 * deployment using Perry's container-compose API.
 */

import { up, ps, exec, down, logs } from 'perry/container-compose';
import { getBackend } from 'perry/container';

async function main() {
    const backend = getBackend();
    console.log(`🔧 Using container backend: ${backend}\n`);

    const FORGEJO_VERSION = '1.23-stable';
    const postgresVersion = '16-alpine';

    console.log('🚀 Bringing up Forgejo stack...');

    const stack = await up({
      name: 'forgejo-prod',
      version: '3.8',
      services: {
        postgres: {
          image: `postgres:${postgresVersion}`,
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
          image: `codeberg.org/forgejo/forgejo:${FORGEJO_VERSION}`,
          restart: 'always',
          depends_on: ['postgres'],
          environment: {
            FORGEJO__database__HOST: '${FORGEJO_DB_HOST:-postgres:5432}',
            FORGEJO__database__name: '${FORGEJO_DB_NAME:-forgejo}',
            FORGEJO__database__user: '${FORGEJO_DB_USER:-forgejo}',
            FORGEJO__database__passwd: '${FORGEJO_DB_PASSWORD:-changeme}',
            FORGEJO__server__PROTOCOL: '${FORGEJO_PROTOCOL:-http}',
            FORGEJO__server__DOMAIN: '${FORGEJO_DOMAIN:-localhost}',
            FORGEJO__server__ROOT_URL: '${FORGEJO_ROOT_URL:-http://localhost:3000}',
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
        'forgejo-pgdata': {
          driver: 'local',
        },
        'forgejo-data': {
          driver: 'local',
        },
        'forgejo-config': {
          driver: 'local',
        },
      },
    });

    console.log('\n🔍 Checking Forgejo stack status...\n');

    const statuses = await ps(stack);
    for (const s of statuses) {
        console.log(`Service: ${s.name}, Status: ${s.status}`);
    }

    console.log('\n🏥 Performing health checks...\n');

    const postgresHealth = await exec(stack, 'postgres', [
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
    }

    console.log('\n🎉 Forgejo Stack is Ready!');

    // In a real app we might wait for SIGINT, here we just cleanup for verification
    console.log('\n🧹 Cleaning up stack...');
    await down(stack, { volumes: true });
    console.log('✅ Cleanup complete');
}

main().catch(err => {
    console.error('Fatal error:', err);
    process.exit(1);
});
