extern crate futures;
extern crate native_tls;
extern crate tokio_core;
extern crate tokio_tls;

use std::io;
use std::net::ToSocketAddrs;

use futures::Future;
use native_tls::TlsConnector;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;
use tokio_tls::TlsConnectorExt;

fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    // std::net::ToSocketAddrsトレイトで文字列がto_socket_addrsを呼べるように
    // to_socket_addrsがDNS名前解決を行う
    let addr = "www.rust-lang.org:443".to_socket_addrs().unwrap().next().unwrap();

    // native_tls::TlsConnectorがTLSコネクションを張るためのインスタンスを生成
    let cx = TlsConnector::builder().unwrap().build().unwrap();

    // futures::Futureトレイトによりsocketにand_thenメソッドが生える
    let socket = TcpStream::connect(&addr, &handle);

    let tls_handshake = socket.and_then(|socket| {
        let tls = cx.connect_async("www.rust-lang.org", socket);
        tls.map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    });

    // HTTPヘッダをソケットに書き込む準備
    let request = tls_handshake.and_then(|socket| {
        tokio_core::io::write_all(socket,
                                  "\
GET / HTTP/1.0\r\n
Host: www.rust-lang.org\r\n
\r\n
"
                                      .as_bytes())
    });

    // HTTPレスポンスを受け取る準備
    let response =
        request.and_then(|(socket, _request)| tokio_core::io::read_to_end(socket, Vec::new()));

    // レスポンスが来るまでイベントループする
    let (_socket, data) = core.run(response).unwrap();
    println!("{}", String::from_utf8_lossy(&data));
}
