//! Partitioning helpers mirroring the v2 architecture document.

use crate::dictionary::StringId;

pub type PartitionId = u32;

#[derive(Debug, Clone)]
pub struct PartitionConfig {
    pub partitions: PartitionId,
}

impl Default for PartitionConfig {
    fn default() -> Self {
        Self { partitions: 1 }
    }
}

impl PartitionConfig {
    pub fn new(partitions: PartitionId) -> Self {
        assert!(partitions > 0, "partitions must be > 0");
        Self { partitions }
    }

    pub fn partition_for(
        &self,
        subject: StringId,
        predicate: StringId,
        object: StringId,
    ) -> PartitionId {
        if self.partitions == 1 {
            return 0;
        }
        let hash =
            subject.wrapping_mul(31) ^ predicate.wrapping_mul(131) ^ object.wrapping_mul(997);
        (hash % self.partitions as u64) as PartitionId
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashing_is_stable() {
        let cfg = PartitionConfig::new(8);
        assert_eq!(cfg.partition_for(1, 2, 3), cfg.partition_for(1, 2, 3));
    }
}
