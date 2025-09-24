import { SynapseDB } from './src/synapseDb.ts';
import { GraphQLProcessor } from './src/query/graphql/processor.ts';

async function main() {
  const db = await SynapseDB.open(':memory:');
  const proc = new GraphQLProcessor((db as any).store);
  await proc.initialize();
  const q = `query { persons(filter: { age_gt: 1000 }) { name age } }`;
  const fields = (proc as any).parseFields(q);
  console.log(fields);
}

main().catch((e) => { console.error(e); process.exit(1); });

