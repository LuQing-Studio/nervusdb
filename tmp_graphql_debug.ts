import { rm } from 'node:fs/promises';
import { SynapseDB } from './src/synapseDb.ts';
import { graphql } from './src/query/graphql/index.ts';

const DB_PATH = './tmp_graphql_db.synapsedb';

async function resetDb(): Promise<void> {
  for (const suffix of ['', '.pages', '.wal']) {
    try {
      await rm(`${DB_PATH}${suffix}`, { recursive: true });
    } catch {}
  }
}

async function setupLargeTestData(db: SynapseDB): Promise<void> {
  for (let i = 1; i <= 50; i++) {
    db.addFact({ subject: `person:${i}`, predicate: 'TYPE', object: 'Person' });
    db.addFact({ subject: `person:${i}`, predicate: 'HAS_NAME', object: `Person ${i}` });
    db.addFact({ subject: `person:${i}`, predicate: 'HAS_AGE', object: String(20 + (i % 40)) });
    db.addFact({ subject: `person:${i}`, predicate: 'HAS_EMAIL', object: `person${i}@example.com` });
  }

  for (let i = 1; i <= 10; i++) {
    db.addFact({ subject: `org:${i}`, predicate: 'TYPE', object: 'Organization' });
    db.addFact({ subject: `org:${i}`, predicate: 'HAS_NAME', object: `Organization ${i}` });
  }

  for (let i = 1; i <= 20; i++) {
    db.addFact({ subject: `project:${i}`, predicate: 'TYPE', object: 'Project' });
    db.addFact({ subject: `project:${i}`, predicate: 'HAS_NAME', object: `Project ${i}` });
  }

  await db.flush();
}

async function main() {
  await resetDb();
  const db = await SynapseDB.open(DB_PATH);
  await setupLargeTestData(db);
  const gql = graphql(db.store);
  const initialSchema = await gql.getSchema();
  console.log('contains Product?', initialSchema.includes('Product'));
  console.log('schema snippet', initialSchema.slice(0, 200));
  await db.close();
  gql.dispose();
  await resetDb();
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
