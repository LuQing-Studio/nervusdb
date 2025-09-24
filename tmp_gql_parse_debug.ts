import { SynapseDB } from './src/synapseDb.ts';
import { graphql } from './src/query/graphql/index.ts';
import { GraphQLProcessor } from './src/query/graphql/processor.ts';

async function main() {
  const db = await SynapseDB.open(':memory:');
  const svc = graphql(db.store);
  const proc = new GraphQLProcessor((svc as any).store);
  await proc.initialize();
  const q = `query { persons(filter: { age_gt: 1000 }) { name age } }`;
  const parsed = (proc as any).parseQuery(q);
  console.log(JSON.stringify(parsed, null, 2));
  await db.close();
  svc.dispose();
}

main().catch((e) => { console.error(e); process.exit(1); });
