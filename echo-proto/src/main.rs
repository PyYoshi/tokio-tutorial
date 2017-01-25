extern crate futures;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;

use std::io;
use std::str;
use tokio_core::io::{Codec, EasyBuf};
use tokio_proto::pipeline::ServerProto;
use tokio_core::io::{Io, Framed};
use tokio_service::Service;
use futures::{future, Future, BoxFuture};
use tokio_proto::TcpServer;

// Step 1: Implement a codec
// ステップ1: コーデックを実装する

// In general, codecs may need local state, for example to record information about incomplete decoding.
// We can get away without it, though:
// 一般に, コーデックはローカルステート(局所状態?)が必要な場合があります.
// 例えば不完全なデコーディングに関する情報を記録する場合です. しかし我々はそれを回避することができます.
pub struct LineCodec;

// Codecs in Tokio implement the Codec trait, which implements message encoding and decoding.
// To start with, we’ll need to specify the message type.
// In gives the types of incoming messages after decoding, while Out gives the type of outgoing messages prior to encoding:
// コーデックにおいてTokioはメッセージのエンコード・デコードを実装するCodecトレイトを実装します.
// まずはじめに, メッセージタイプを指定する必要があります.
// Inはデコード後の受信メッセージの型を指定し, Outはエンコードする前の送信メッセージの型を指定します.
impl Codec for LineCodec {
    type In = String;
    type Out = String;

    // We’ll use String to represent lines, meaning that we’ll require UTF-8 encoding for this line protocol.
    // For our line-based protocol, decoding is straightforward:
    // 我々は行を表現するためにStringを使います. それはつまり, この行プロトコルにおいてはUTF-8エンコーディングを必要としていることです.
    // ラインベースプロトコルにおいてはデコードは単純(複雑ではない)です. (Rustはデフォルトで文字列はUTF-8が使われるから)

    // The EasyBuf type used here provides simple but efficient buffer management; you can think of it like Arc<[u8]>, a reference-counted immutable slice of bytes, with all the details handled for you.
    // Outgoing messages from the server use Result in order to convey service errors on the Rust side.
    // ここで使用されるEasyBuf型はシンプルで効率的なバッファ管理を提供します. それを参照カウントされたイミュータブルなバイトスライスであるArc<[u8]>のようにすべてが処理されるとあなたは考えることができるでしょう.
    // サーバからの送信メッセージはRust側からのサービスエラーを運ぶためにResultを利用します.

    // When decoding, we are given an input EasyBuf that contains a chunk of unprocessed data, and we must try to extract the first complete message, if there is one.
    // If the buffer doesn’t contain a complete message, we return None, and the server will automatically fetch more data before trying to decode again.
    // The drain_to method on EasyBuf splits the buffer in two at the given index, returning a new EasyBuf instance corresponding to the prefix ending at the index, and updating the existing EasyBuf to contain only the suffix.
    // It’s the typical way to remove one complete message from the input buffer.
    // デコーディングするとき我々はEasyBufに未処理データのチャンクが与えられ, そして初めの完全なメッセージがあれば展開(抽出?)する必要があります.
    // もしバッファに完全なメッセージが含まれていない場合はNoneを返し, サーバは自動的にデコード前のデータのフェッチを試みます.
    // EasyBufにあるdrain_toメソッドは指定されたインデックスでバッファを二分割し, インデックスに終わるプレフィックス(なぞ)に合致する新しいEasyBufインスタンスを返します. そして既存のEasyBufをサフィックスだけを含むように更新します.
    // それは典型的な方法で1つの完全なメッセージを入力バッファから削除します.
    fn decode(&mut self, buf: &mut EasyBuf) -> io::Result<Option<Self::In>> {
        if let Some(i) = buf.as_slice().iter().position(|&b| b == b'\n') {
            // remove the serialized frame from the buffer.
            // バッファからシリアライズされたフレームを削除する
            let line = buf.drain_to(i);

            // Also remove the '\n'
            // '\n'も同様に削除する
            buf.drain_to(1);

            // Turn this data into a UTF string and return it in a Frame.
            // このデータをUTFへ変換し, Frameへ返します.
            match str::from_utf8(line.as_slice()) {
                Ok(s) => Ok(Some(s.to_string())),
                Err(_) => Err(io::Error::new(io::ErrorKind::Other, "invalid UTF-8")),
            }
        } else {
            Ok(None)
        }
    }

    // Encoding is even easier: you’re given mutable access to a Vec<u8>, into which you serialize your output data.
    // To keep things simple, we won’t provide support for error responses:
    // エンコードはさらに簡単です. 出力されたデータをシリアライズするVec<u8>へのミュータブルなアクセス権を与えられます.
    // 物事を単純にするために, エラーレスポンスをサポートしません.
    fn encode(&mut self, msg: String, buf: &mut Vec<u8>) -> io::Result<()> {
        buf.extend(msg.as_bytes());
        buf.push(b'\n');
        Ok(())
    }
}

// Step 2: Specify the protocol
// ステップ2: プロトコルを指定する

// Next, we turn the codec into a full-blown protocol.
// The tokio-proto crate is equipped to deal with a variety of protocol styles, including multiplexed and streaming protocols.
// For our line-based protocol, though, we’ll use the simplest style: a pipelined, non-streaming protocol:
// 次は, 本格的なプロトコルへ変換します.
// tokio-protoクレートは多重化やストリーミングを含んだ様々なプロトコルスタイルを使えます.
// ラインベースプロトコルに対しては最高にシンプルなパイプラインや非ストリーミングプロトコルスタイルを使えます.

// As with codecs, protocols can carry state, typically used for configuration.
// We don’t need any configuration, so we’ll make another unit struct:
// コーデックの場合と同様にプロトコルは, コンフィグレーションに利用される状態を保持しています.
// 他のコンフィグレーションを必要としないので, 別のユニットを構造体にします:
pub struct LineProto;

// Setting up a protocol requires just a bit of boilerplate, tying together our chosen protocol style with the codec that we’ve built:
// プロトコルを設定するには選んだプロトコルスタイルに組み込んだコーデック結びつけるだけの小さなボイラプレートが必要です:
impl<T: Io + 'static> ServerProto<T> for LineProto {
    /// For this protocol style, `Request` matches the codec `In` type
    /// このプロトコルスタイルにおいて`はRequest`はIn型のコーデックとマッチします.
    type Request = String;

    /// For this protocol style, `Response` matches the coded `Out` type
    /// このプロトコルスタイルにおいては`Response`はOut型のコーデックとマッチします.
    type Response = String;

    /// A bit of boilerplate to hook in the codec:
    // コーデックをフックするための小さなボイラプレート:
    type Transport = Framed<T, LineCodec>;
    type BindTransport = Result<Self::Transport, io::Error>;
    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(io.framed(LineCodec))
    }
}

// Step 3: Implement a service
// ステップ3: サービスを実装する

// At this point, we’ve built a generic line-based protocol.
// To actually use this protocol, we need to pair it with a service that says how to respond to requests.
// The tokio-service crate provides a Service trait for just this purpose:

// As with the other components we’ve built, in general a service may have data associated with it.
// The service we want for this example just echos its input, so no additional data is needed:
pub struct Echo;

// At its core, a service is an asynchronous (non-blocking) function from requests to responses.
// We’ll have more to say about asynchronous programming in the next guide; the only thing to know right now is that Tokio uses futures for asynchronous code, through the Future trait.
// You can think of a future as an asynchronous version of Result.
// Let’s bring the basics into scope:

// For our echo service, we don’t need to do any I/O to produce a response for a request.
// So we use future::ok to make a future that immediately returns a value—in this case, returning the request immediately back as a successful response.
// To keep things simple, we’ll also box the future into a trait object, which allows us to use the BoxFuture trait to define our service, no matter what future we actually use inside—more on those tradeoffs later!
impl Service for Echo {
    // These types must match the corresponding protocol types:
    type Request = String;
    type Response = String;

    // For non-streaming protocols, service errors are always io::Error
    type Error = io::Error;

    // The future for computing the response; box it for simplicity.
    type Future = BoxFuture<Self::Response, Self::Error>;

    // Produce a future for computing a response from a request.
    fn call(&self, req: Self::Request) -> Self::Future {
        // In this case, the response is immediate.
        future::ok(req).boxed()
    }
}

// We’re done—now configure and run!

// With that, we have the ingredients necessary for a full-blown server: a general protocol, and a particular service to provide on it.
// All that remains is to actually configure and launch the server, which we’ll do using the TcpServer builder:
fn main() {
    // Specify the localhost address
    let addr = "0.0.0.0:12345".parse().unwrap();

    // The builder requires a protocol and an address
    let server = TcpServer::new(LineProto, addr);

    // We provide a way to *instantiate* the service for each new
    // connection; here, we just immediately return a new instance.
    server.serve(|| Ok(Echo));
}

// UTF-8な端末から叩かないとtelnetが落ちます.
// Windows環境ではmsys2からtelnetを叩くと良い.

// You can run this code and connect locally to try it out:
// ~ $ telnet localhost 12345
// Trying 127.0.0.1...
// Connected to localhost.
// Escape character is '^]'.
// hello, world!
// hello, world!
// echo
// echo

// Pairing with another service
// That was a fair amount of ceremony for a simple echo server.
// But most of what we did—the protocol specification—is reusable.
// To prove it, let’s build a service that echos its input in reverse:
struct EchoRev;

impl Service for EchoRev {
    type Request = String;
    type Response = String;
    type Error = io::Error;
    type Future = BoxFuture<Self::Response, Self::Error>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let rev: String = req.chars()
            .rev()
            .collect();
        future::ok(rev).boxed()
    }
}

// Not too shabby. And now, if we serve EchoRev instead of Echo, we’ll see:

// ~ $ telnet localhost 12345
// Trying 127.0.0.1...
// Connected to localhost.
// Escape character is '^]'.
// hello, world!
// !dlrow ,olleh
// echo
// ohce
