use super::{
    EdgeKey, Error, GraphSnapshot, InternalNodeId, LabelId, RelTypeId, Result, Row, Value,
};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

pub trait Procedure: Send + Sync {
    fn execute(&self, snapshot: &dyn ErasedSnapshot, args: Vec<Value>) -> Result<Vec<Row>>;
}

pub trait ErasedSnapshot {
    fn neighbors_erased(
        &self,
        src: InternalNodeId,
        rel: Option<RelTypeId>,
    ) -> Box<dyn Iterator<Item = EdgeKey> + '_>;
    fn incoming_neighbors_erased(
        &self,
        dst: InternalNodeId,
        rel: Option<RelTypeId>,
    ) -> Box<dyn Iterator<Item = EdgeKey> + '_>;
    fn node_property_erased(
        &self,
        iid: InternalNodeId,
        key: &str,
    ) -> Option<nervusdb_api::PropertyValue>;
    fn resolve_label_name_erased(&self, id: LabelId) -> Option<String>;
    fn resolve_rel_type_name_erased(&self, id: RelTypeId) -> Option<String>;
    fn resolve_node_labels_erased(&self, iid: InternalNodeId) -> Option<Vec<LabelId>>;
    fn node_properties_erased(
        &self,
        iid: InternalNodeId,
    ) -> Option<std::collections::BTreeMap<String, nervusdb_api::PropertyValue>>;
    fn edge_properties_erased(
        &self,
        key: EdgeKey,
    ) -> Option<std::collections::BTreeMap<String, nervusdb_api::PropertyValue>>;
}

impl<S: GraphSnapshot> ErasedSnapshot for S {
    fn neighbors_erased(
        &self,
        src: InternalNodeId,
        rel: Option<RelTypeId>,
    ) -> Box<dyn Iterator<Item = EdgeKey> + '_> {
        Box::new(self.neighbors(src, rel))
    }

    fn incoming_neighbors_erased(
        &self,
        dst: InternalNodeId,
        rel: Option<RelTypeId>,
    ) -> Box<dyn Iterator<Item = EdgeKey> + '_> {
        Box::new(self.incoming_neighbors(dst, rel))
    }

    fn node_property_erased(
        &self,
        iid: InternalNodeId,
        key: &str,
    ) -> Option<nervusdb_api::PropertyValue> {
        self.node_property(iid, key)
    }

    fn resolve_label_name_erased(&self, id: LabelId) -> Option<String> {
        self.resolve_label_name(id)
    }

    fn resolve_rel_type_name_erased(&self, id: RelTypeId) -> Option<String> {
        self.resolve_rel_type_name(id)
    }

    fn resolve_node_labels_erased(&self, iid: InternalNodeId) -> Option<Vec<LabelId>> {
        self.resolve_node_labels(iid)
    }

    fn node_properties_erased(
        &self,
        iid: InternalNodeId,
    ) -> Option<std::collections::BTreeMap<String, nervusdb_api::PropertyValue>> {
        self.node_properties(iid)
    }

    fn edge_properties_erased(
        &self,
        key: EdgeKey,
    ) -> Option<std::collections::BTreeMap<String, nervusdb_api::PropertyValue>> {
        self.edge_properties(key)
    }
}

pub struct ProcedureRegistry {
    handlers: HashMap<String, Arc<dyn Procedure>>,
}

impl Default for ProcedureRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcedureRegistry {
    pub fn new() -> Self {
        let mut handlers: HashMap<String, Arc<dyn Procedure>> = HashMap::new();
        handlers.insert("db.info".to_string(), Arc::new(DbInfoProcedure));
        handlers.insert("math.add".to_string(), Arc::new(MathAddProcedure));
        Self { handlers }
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Procedure>> {
        self.handlers.get(name).cloned()
    }
}

pub static GLOBAL_PROCEDURE_REGISTRY: OnceLock<ProcedureRegistry> = OnceLock::new();

pub fn get_procedure_registry() -> &'static ProcedureRegistry {
    GLOBAL_PROCEDURE_REGISTRY.get_or_init(ProcedureRegistry::new)
}

struct DbInfoProcedure;

impl Procedure for DbInfoProcedure {
    fn execute(&self, _snapshot: &dyn ErasedSnapshot, _args: Vec<Value>) -> Result<Vec<Row>> {
        Ok(vec![Row::new(vec![(
            "version".to_string(),
            Value::String("2.0.0".to_string()),
        )])])
    }
}

struct MathAddProcedure;

impl Procedure for MathAddProcedure {
    fn execute(&self, _snapshot: &dyn ErasedSnapshot, args: Vec<Value>) -> Result<Vec<Row>> {
        if args.len() != 2 {
            return Err(Error::Other("math.add requires 2 arguments".to_string()));
        }
        let a = match &args[0] {
            Value::Int(i) => *i as f64,
            Value::Float(f) => *f,
            _ => {
                return Err(Error::Other(
                    "math.add requires numeric arguments".to_string(),
                ));
            }
        };
        let b = match &args[1] {
            Value::Int(i) => *i as f64,
            Value::Float(f) => *f,
            _ => {
                return Err(Error::Other(
                    "math.add requires numeric arguments".to_string(),
                ));
            }
        };
        Ok(vec![Row::new(vec![(
            "result".to_string(),
            Value::Float(a + b),
        )])])
    }
}
