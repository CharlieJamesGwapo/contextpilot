use crate::storage_extended::{ModelMetadata, UsageImport};
use contextpilot_core::*;
use serde_json::{Value, json};
/// The schemas are derived from the exact Rust serde boundary/domain types.
pub fn schemas() -> Value {
    json!({"api_version":"v1","database_schema":2,"schemas":{"ToolOutputReport":schemars::schema_for!(ToolOutputReport),"Estimate":schemars::schema_for!(Estimate),"Optimization":schemars::schema_for!(Optimization),"Session":schemars::schema_for!(Session),"Config":schemars::schema_for!(Config),"Checkpoint":schemars::schema_for!(Checkpoint),"CompactionPreview":schemars::schema_for!(CompactionPreview),"Budget":schemars::schema_for!(Budget),"Price":schemars::schema_for!(Price),"ModelMetadata":schemars::schema_for!(ModelMetadata),"UsageImport":schemars::schema_for!(UsageImport),"RepositoryIndex":schemars::schema_for!(RepositoryIndex),"FileContext":schemars::schema_for!(FileContext)}})
}
