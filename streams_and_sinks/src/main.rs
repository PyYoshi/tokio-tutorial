extern crate futures;
extern crate tokio_core;

use futures::stream::Stream;
use tokio_core::reactor::Core;
use tokio_core::net::TcpListener;

fn main() {
    let mut core = Core::new().unwrap();
    let address = "127.0.0.1:12345".parse().unwrap();
    let listener = TcpListener::bind(&address, &core.handle()).unwrap();

    let connections = listener.incoming();

    // オリジナルコード
    // let welcomes = connections.and_then(|(socket, _peer_addr)| {
    //     tokio_core::io::write_all(socket, b"Hello, world!\n")
    // });

    // welcomes利用すればwrite_allを再度呼び出せる
    // let welcomes = connections.and_then(|(socket, _peer_addr)| {
    //     println!("client address: {}", _peer_addr);
    //     tokio_core::io::write_all(socket, b"Hello, world!\n")
    // });
    // let welcomes = welcomes.and_then(|(socket, _peer_addr)| {
    //     tokio_core::io::write_all(socket, b"It's a beautiful world!\n")
    // });

    // and_thenをつなげればwrite_allを再度呼び出せる
    let welcomes = connections.and_then(|(socket, _peer_addr)| {
            println!("client address: {}", _peer_addr);
            tokio_core::io::write_all(socket, b"Hello, world!\n")
        })
        .and_then(|(socket, _peer_addr)| {
            tokio_core::io::write_all(socket, b"It's a beautiful world!\n")
        });

    let server = welcomes.for_each(|(_socket, _welcome)| Ok(()));

    core.run(server).unwrap();
}

// ~ $ telnet 127.0.0.1 12345
// Trying 127.0.0.1...
// Connected to 127.0.0.1.
// Escape character is '^]'.
// Hello, world!
// It's a beautiful world!
// Connection closed by foreign host.
