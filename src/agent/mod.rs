mod claude;
mod model;
mod opencode;
pub mod registry;

pub use model::{Agent, AgentStatus, AgentType};
pub use registry::{AgentDetector, AgentRegistry, create_default_registry};
