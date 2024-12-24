use core::fmt;

#[derive(Debug)]
pub enum GraphError {
    Io(std::io::Error),
    GraphConnectionError(String, std::io::Error),
    StorageConnectionError(String, std::io::Error),
    StorageError(String),
    TraversalError(String),
    EdgeNotFound,
    NodeNotFound,
    New(String)
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphError::Io(e) => write!(f, "IO error: {}", e),
            GraphError::StorageConnectionError(msg, e) => {
                write!(f, "Error: {}", format!("{} {}", msg, e))
            },
            GraphError::GraphConnectionError(msg, e) => {
                write!(f, "Error: {}", format!("{} {}", msg, e))
            },
            GraphError::TraversalError(msg) => write!(f, "Traversal error: {}", msg),
            GraphError::StorageError(msg) => write!(f, "Storage error: {}", msg),
            GraphError::EdgeNotFound => write!(f, "Edge not found"),
            GraphError::NodeNotFound => write!(f, "Node not found"),
            GraphError::New(msg) => write!(f, "Graph error: {}", msg),
        }
    }
}

impl From<rocksdb::Error> for GraphError {
    fn from(error: rocksdb::Error) -> Self {
        GraphError::New(error.into_string())
    }
}

impl From<std::io::Error> for GraphError {
    fn from(error: std::io::Error) -> Self {
        GraphError::Io(error)
    }
}

