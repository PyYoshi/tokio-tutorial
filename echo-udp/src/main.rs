extern crate futures;

#[macro_use]
extern crate tokio_core;

use std::{env, io};
use std::net::SocketAddr;

use futures::{Future, Poll};
use tokio_core::net::UdpSocket;
use tokio_core::reactor::Core;

struct EchoServer {
    socket: UdpSocket,
    buf: Vec<u8>,
    to_send: Option<(usize, SocketAddr)>,
}

impl Future for EchoServer {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {
        loop {
            if let Some((size, peer)) = self.to_send {
                let amt = try_nb!(self.socket.send_to(&self.buf[..size], &peer));
                println!("Echoed {}/{} bytes to {}", amt, size, peer);
                self.to_send = None;
            }

            self.to_send = Some(try_nb!(self.socket.recv_from(&mut self.buf)));
        }
    }
}

fn main() {
    let addr = "127.0.0.1:12345".parse::<SocketAddr>().unwrap();

    let mut l = Core::new().unwrap();
    let handle = l.handle();
    let socket = UdpSocket::bind(&addr, &handle).unwrap();
    println!("Listening on: {}", addr);

    l.run(EchoServer {
            socket: socket,
            buf: vec![0; 1024],
            to_send: None,
        })
        .unwrap();
}

// $ nc -4u 127.0.0.1 12345
// or
// $ portqry -n 127.0.0.1 -e 12345 -p UDP
