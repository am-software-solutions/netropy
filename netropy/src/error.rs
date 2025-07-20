use thiserror::Error;

#[derive(Error, Debug)]
pub enum SimError {
    #[error("link delivery failed")]
    LinkDeliveryError,
    #[error("node {0} not found")]
    UnknownNode(String),
    #[error("simulation has ended")]
    SimulationEnded,
}
