extern crate futures;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;

use std::io;
use std::str;
use tokio_core::reactor::Core;
use tokio_core::net::TcpListener;
use tokio_core::io::{Codec, EasyBuf, Io, Framed};
use tokio_proto::pipeline::ServerProto;
use tokio_service::{Service, NewService};
use futures::{future, Future, BoxFuture, Stream, Sink};

pub struct LineCodec;
impl Codec for LineCodec {
    type In = String;
    type Out = String;

    fn decode(&mut self, buf: &mut EasyBuf) -> io::Result<Option<Self::In>> {
        if let Some(i) = buf.as_slice().iter().position(|&b| b == b'\n') {
            let line = buf.drain_to(i);

            buf.drain_to(1);

            match str::from_utf8(line.as_slice()) {
                Ok(s) => Ok(Some(s.to_string())),
                Err(_) => Err(io::Error::new(io::ErrorKind::Other, "invalid UTF-8")),
            }
        } else {
            Ok(None)
        }
    }

    fn encode(&mut self, msg: String, buf: &mut Vec<u8>) -> io::Result<()> {
        buf.extend(msg.as_bytes());
        buf.push(b'\n');
        Ok(())
    }
}

pub struct LineProto;
impl<T: Io + 'static> ServerProto<T> for LineProto {
    type Request = String;

    type Response = String;

    type Transport = Framed<T, LineCodec>;
    type BindTransport = Result<Self::Transport, io::Error>;
    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(io.framed(LineCodec))
    }
}

pub struct Echo;
impl Service for Echo {
    type Request = String;
    type Response = String;

    type Error = io::Error;

    type Future = BoxFuture<Self::Response, Self::Error>;

    fn call(&self, req: Self::Request) -> Self::Future {
        future::ok(req).boxed()
    }
}

// struct EchoRev;
// impl Service for EchoRev {
//     type Request = String;
//     type Response = String;
//     type Error = io::Error;
//     type Future = BoxFuture<Self::Response, Self::Error>;

//     fn call(&self, req: Self::Request) -> Self::Future {
//         let rev: String = req.chars()
//             .rev()
//             .collect();
//         future::ok(rev).boxed()
//     }
// }

fn serve<S>(s: S) -> io::Result<()>
    where S: NewService<Request = String, Response = String, Error = io::Error> + 'static
{
    let mut core = Core::new()?;
    let handle = core.handle();

    let address = "127.0.0.1:12345".parse().unwrap();
    let listener = TcpListener::bind(&address, &handle)?;

    let connections = listener.incoming();
    let server = connections.for_each(move |(socket, _peer_addr)| {
        let (writer, reader) = socket.framed(LineCodec).split();
        // let mut service = s.new_service()?;
        let service = s.new_service()?;

        let responses = reader.and_then(move |req| service.call(req));
        let server = writer.send_all(responses)
            .then(|_| Ok(()));
        handle.spawn(server);

        Ok(())
    });

    core.run(server)
}

struct EchoService;
impl Service for EchoService {
    type Request = String;
    type Response = String;
    type Error = io::Error;
    type Future = BoxFuture<String, io::Error>;
    fn call(&self, input: String) -> Self::Future {
        future::ok(input).boxed()
    }
}

fn main() {
    if let Err(e) = serve(|| Ok(EchoService)) {
        println!("Server failed with {}", e);
    }
}

// $ telnet 127.0.0.1 12345
// Trying 127.0.0.1...
// Connected to 127.0.0.1.
// Escape character is '^]'.
// Hello, world!
// Hello, world!
// Connection closed by foreign host.
