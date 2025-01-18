use std::{collections::HashMap, sync::Arc};

use connection::connection::ConnectionHandler;
use helix_engine::graph_core::graph_core::HelixGraphEngine;
use router::router::{HandlerFn, HelixRouter};

pub mod connection;
pub mod router;
pub mod thread_pool;
pub struct GatewayOpts {
}

impl GatewayOpts {
    pub const DEFAULT_POOL_SIZE: usize = 10;
}

pub struct HelixGateway {
    pub connection_handler: ConnectionHandler,
}

impl HelixGateway {
    pub fn new(address: &str, graph: Arc<HelixGraphEngine>, size: usize, routes: Option<HashMap<(String,String), HandlerFn>>) -> HelixGateway {
        let router= HelixRouter::new(routes);
        let connection_handler = ConnectionHandler::new(address, graph, size, router).unwrap();
        HelixGateway {
            connection_handler,
        }
    }
}

#[cfg(test)]
mod tests {
    use connection::connection::ConnectionHandler;
    use helix_engine::types::GraphError;
    use protocol::{request::Request, response::Response};
    use router::router::HelixRouter;
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        sync::{Arc, Mutex},
        time::Duration,
    };
    use tempfile::TempDir;
    use thread_pool::thread_pool::ThreadPool;

    use super::*;

    fn setup_temp_db() -> (HelixGraphEngine, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().to_str().unwrap();
        let storage = HelixGraphEngine::new(db_path).unwrap();
        (storage, temp_dir)
    }

    fn create_test_connection() -> std::io::Result<(TcpStream, TcpStream)> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;

        let client = TcpStream::connect(addr)?;
        let server = listener.accept()?.0;

        for stream in [&client, &server] {
            stream.set_read_timeout(Some(Duration::from_millis(100)))?;
            stream.set_write_timeout(Some(Duration::from_millis(100)))?;
            stream.set_nonblocking(false)?;
        }

        Ok((client, server))
    }

    fn read_with_timeout(stream: &mut TcpStream, timeout: Duration) -> std::io::Result<Vec<u8>> {
        let start = std::time::Instant::now();
        let mut received = Vec::new();
        let mut buffer = [0; 1024];

        while start.elapsed() < timeout {
            match stream.read(&mut buffer) {
                Ok(0) => break, // If EOF reached
                Ok(n) => received.extend_from_slice(&buffer[..n]),
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Ok(received)
    }

    #[test]
    fn test_response_creation_and_sending() -> std::io::Result<()> {
        let (mut client, mut server) = create_test_connection()?;

        let mut response = Response::new();
        response.status = 200;
        response
            .headers
            .insert("Content-Type".to_string(), "text/plain".to_string());
        response.body = b"Hello World".to_vec();

        println!("{:?}", response);
        response.send(&mut server)?;
        server.flush()?;

        let received = read_with_timeout(&mut client, Duration::from_millis(100))?;
        let response_str = String::from_utf8_lossy(&received);

        println!("{:?}", response_str);
        assert!(response_str.contains("HTTP/1.1 200 OK"));
        assert!(response_str.contains("Content-Type: text/plain"));
        assert!(response_str.contains("Content-Length: 11"));
        assert!(response_str.to_string().contains("Hello World"));

        Ok(())
    }

    #[test]
    fn test_thread_pool_creation() {
        let (storage, _) = setup_temp_db();
        let size = 4;
        let router = Arc::new(HelixRouter::new(None));
        let graph = Arc::new(storage);
        let pool = ThreadPool::new(size, graph, router);

        assert_eq!(*pool.num_unused_workers.lock().unwrap(), size);
        assert_eq!(*pool.num_used_workers.lock().unwrap(), 0);
    }

    #[test]
    #[should_panic(expected = "Expected number of threads in thread pool to be more than 0")]
    fn test_thread_pool_zero_size() {
        let (storage, _) = setup_temp_db();
        let router = Arc::new(HelixRouter::new(None));
        let graph = Arc::new(storage);
        ThreadPool::new(0, graph, router);
    }

    #[test]
    fn test_connection_handler() -> Result<(), GraphError> {
        let (storage, _) = setup_temp_db();
        let address = "127.0.0.1:0";

        let router = HelixRouter::new(None);
        let graph = Arc::new(storage);
        let handler = ConnectionHandler::new(address, graph, 4, router)?;

        let addr = handler.listener.local_addr()?;
        let _client = TcpStream::connect(addr)?;

        Ok(())
    }

    #[test]
    fn test_router_integration() -> std::io::Result<()> {
        let (mut client, mut server) = create_test_connection()?;
        let (storage, _) = setup_temp_db();
        let mut router = HelixRouter::new(None);
        let graph_storage = Arc::new(storage);

        // Add route
        router.add_route("GET", "/test", |_, response| {
            response.status = 200;
            response.body = b"Success".to_vec();
            response
                .headers
                .insert("Content-Type".to_string(), "text/plain".to_string());
            Ok(())
        });

        // Send test request
        let request_str = "GET /test HTTP/1.1\r\nHost: localhost\r\n\r\n";
        client.write_all(request_str.as_bytes())?;
        client.flush()?;

        // Handle Request
        let request = Request::from_stream(&mut server)?;
        let mut response = Response::new();
        router
            .handle(graph_storage, request, &mut response)
            .unwrap();
        response.send(&mut server)?;
        server.flush()?;

        let received = read_with_timeout(&mut client, Duration::from_millis(100))?;
        let response_str = String::from_utf8_lossy(&received);

        println!("{:?}", response_str);
        assert!(response_str.contains("HTTP/1.1 200 OK"));
        assert!(response_str.contains("Content-Type: text/plain"));
        assert!(response_str.to_string().contains("Success"));

        Ok(())
    }
}
