mod claude;
mod model;
pub mod registry;

pub use model::{Agent, AgentStatus, AgentType};
pub use registry::{create_default_registry, AgentDetector, AgentRegistry};
