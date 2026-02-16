import type { Db as NervusDb } from "../../../nervusdb-node/index";

type NervusAddon = {
  Db: {
    open(path: string): NervusDb;
  };
};

const addon = require("../native/nervusdb_node.node") as NervusAddon;

const dbPath = "/tmp/nervusdb-ts-local.ndb";
const db = addon.Db.open(dbPath);

const created = db.executeWrite("CREATE (n:Person {name:'TS Local'})");
if (created <= 0) {
  throw new Error(`expected created > 0, got ${created}`);
}

const rows = db.query("MATCH (n:Person) RETURN n LIMIT 1");
if (!Array.isArray(rows) || rows.length === 0) {
  throw new Error("expected non-empty result rows");
}

const txn = db.beginWrite();
txn.query("CREATE (:Person {name:'TS Txn'})");
const affected = txn.commit();
if (affected <= 0) {
  throw new Error(`expected txn commit affected > 0, got ${affected}`);
}

db.close();
console.log("ts-local smoke ok");
