pub mod agent_message;
pub mod compute;
pub mod error;

pub use agent_message::*;
pub use compute::*;
pub use error::*;

pub const AGENTMESSAGE_V1: &str = "agentmessage.io/v1";
pub const AGENTGRID_V1: &str = "agentgrid.io/v1";
