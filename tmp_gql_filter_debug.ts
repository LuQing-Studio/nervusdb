import { SynapseDB } from './src/synapseDb.ts';
import { graphql } from './src/query/graphql/index.ts';

async function main() {
  const db = await SynapseDB.open(':memory:');
  for (let i = 1; i <= 3; i++) {
    db.addFact({ subject: `person:${i}`, predicate: 'TYPE', object: 'Person' });
    db.addFact({ subject: `person:${i}`, predicate: 'HAS_NAME', object: `Person ${i}` });
    db.addFact({ subject: `person:${i}`, predicate: 'HAS_AGE', object: String(20 + i) });
  }
  await db.flush();

  const svc = graphql(db.store);
  await svc.regenerateSchema();

  const q1 = `query { persons(filter: { age_gt: 1000 }) { name age } }`;
  const r1 = await svc.executeQuery(q1);
  console.log('r1 persons length', Array.isArray(r1.data?.persons) ? r1.data.persons.length : 'NA');

  const q2 = `query GetAdults($ageFilter: Int){ persons(filter: { age_gt: $ageFilter }) { name age } }`;
  const r2 = await svc.executeQuery(q2, { ageFilter: 18 });
  console.log('r2 persons length', Array.isArray(r2.data?.persons) ? r2.data.persons.length : 'NA');

  await db.close();
  svc.dispose();
}

main().catch((e) => { console.error(e); process.exit(1); });
