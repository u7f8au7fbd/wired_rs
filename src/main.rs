
use std::collections::HashMap;
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

type PeerMap = Arc<Mutex<HashMap<String, TcpStream>>>;

//最初のスーパーノードのアドレスを設定
const SUPER_NODE_ADDR: &str = "";
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(20); // 2サイクル分

fn handle_peer(mut stream: TcpStream, peers: PeerMap) {
    let peer_addr = stream.peer_addr().unwrap().to_string();
    let mut buffer = [0; 512];
    loop {
        match stream.read(&mut buffer) {
            Ok(size) if size > 0 => {
                let message = String::from_utf8_lossy(&buffer[..size]);
                println!("接続: {}", message);

                if message == "HEARTBEAT" {
                    continue; // ハートビートメッセージを無視
                }

                let peers = peers.lock().unwrap();
                for (_addr, peer_stream) in peers.iter() {
                    if peer_stream.peer_addr().unwrap() != stream.peer_addr().unwrap() {
                        if let Ok(mut peer_stream) = peer_stream.try_clone() {
                            if let Err(e) = peer_stream.write_all(message.as_bytes()) {
                                eprintln!("メッセージの転送に失敗: {}", e);
                            }
                        }
                    }
                }
            }
            Ok(_) => (),
            Err(e) => {
                eprintln!("接続エラー: {}", e);
                peers.lock().unwrap().remove(&peer_addr);
                break;
            }
        }
    }
}

fn start_supernode(peers: PeerMap) -> std::io::Result<()> {
    let listener = TcpListener::bind(SUPER_NODE_ADDR)?;
    println!("スーパーノードがポート2501で起動");

    // ノード一覧表示スレッド
    let peers_clone = Arc::clone(&peers);
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(2));
        let peers = peers_clone.lock().unwrap();
        println!("接続されたノード: {:?}", peers.keys());
    });

    // ハートビート送信スレッド
    let peers_clone = Arc::clone(&peers);
    thread::spawn(move || loop {
        {
            let peers = peers_clone.lock().unwrap();
            for (_addr, peer_stream) in peers.iter() {
                if let Ok(mut peer_stream) = peer_stream.try_clone() {
                    if let Err(e) = peer_stream.write_all(b"HEARTBEAT") {
                        eprintln!("ハートビートの送信に失敗: {}", e);
                    }
                }
            }
        }
        thread::sleep(HEARTBEAT_INTERVAL);
    });

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let peer_addr = stream.peer_addr().unwrap().to_string();
                peers
                    .lock()
                    .unwrap()
                    .insert(peer_addr.clone(), stream.try_clone().unwrap());
                let peers = Arc::clone(&peers);
                thread::spawn(move || {
                    handle_peer(stream, peers);
                });
            }
            Err(e) => {
                eprintln!("接続エラー: {}", e);
            }
        }
    }
    Ok(())
}

fn connect_to_supernode(
    supernode_addr: &str,
    peer_message: &str,
    last_heartbeat: Arc<Mutex<Instant>>,
) -> std::io::Result<()> {
    loop {
        match TcpStream::connect(supernode_addr) {
            Ok(mut stream) => {
                let addr = stream.local_addr()?.to_string();
                stream.write_all(addr.as_bytes())?;

                // 受信スレッド
                let mut receive_stream = stream.try_clone()?;
                let last_heartbeat_clone = Arc::clone(&last_heartbeat);
                thread::spawn(move || {
                    let mut buffer = [0; 512];
                    loop {
                        match receive_stream.read(&mut buffer) {
                            Ok(size) if size > 0 => {
                                let message = String::from_utf8_lossy(&buffer[..size]);
                                if message == "HEARTBEAT" {
                                    *last_heartbeat_clone.lock().unwrap() = Instant::now();
                                } else {
                                    println!("接続: {}", message);
                                }
                            }
                            Ok(_) => (),
                            Err(e) => {
                                eprintln!("接続エラー: {}", e);
                                break;
                            }
                        }
                    }
                });

                // 送信ループ
                loop {
                    if let Err(e) = stream.write_all(peer_message.as_bytes()) {
                        eprintln!("メッセージの送信に失敗: {}", e);
                        break;
                    }
                    thread::sleep(Duration::from_secs(2));
                }
            }
            Err(e) => {
                eprintln!("スーパーノードへの接続エラー: {}", e);
                thread::sleep(Duration::from_secs(2)); // リトライのための待機
            }
        }
    }
}

fn check_supernode_alive(last_heartbeat: Arc<Mutex<Instant>>) -> bool {
    Instant::now().duration_since(*last_heartbeat.lock().unwrap()) < HEARTBEAT_TIMEOUT
}

fn main() -> std::io::Result<()> {
    let peers: PeerMap = Arc::new(Mutex::new(HashMap::new()));
    let peer_message = "Hello, peer!";
    let last_heartbeat = Arc::new(Mutex::new(Instant::now()));
    let supernode_check_interval = Duration::from_secs(10);

    // スーパーノードとして動作するかどうかを確認
    let running_as_supernode = false;
    match TcpListener::bind(SUPER_NODE_ADDR) {
        Ok(listener) => {
            println!("スーパーノードとして動作します...");
            // スーパーノードとして動作を開始
            let peers_clone = Arc::clone(&peers);
            thread::spawn(move || {
                for stream in listener.incoming() {
                    match stream {
                        Ok(stream) => {
                            let peer_addr = stream.peer_addr().unwrap().to_string();
                            peers_clone
                                .lock()
                                .unwrap()
                                .insert(peer_addr.clone(), stream.try_clone().unwrap());
                            let peers = Arc::clone(&peers_clone);
                            thread::spawn(move || {
                                handle_peer(stream, peers);
                            });
                        }
                        Err(e) => {
                            eprintln!("接続に失敗しました: {}", e);
                        }
                    }
                }
            });

            // 定期的に接続されているノードのIPアドレスの一覧を表示
            loop {
                thread::sleep(Duration::from_secs(2));
                let peers = peers.lock().unwrap();
                println!("接続者のIP: {:?}", peers.keys());
            }
        }
        Err(_) => {
            println!(
                "ピアノードとして動作しているスーパーノードポートにバインドできませんでした..."
            );
        }
    }

    // スーパーノードとして動作しない場合、一般ノードとして動作
    if !running_as_supernode {
        let peers_clone = Arc::clone(&peers);
        let last_heartbeat_clone = Arc::clone(&last_heartbeat);
        thread::spawn(move || loop {
            thread::sleep(supernode_check_interval);
            if !check_supernode_alive(last_heartbeat_clone.clone()) {
                if let Err(e) = start_supernode(peers_clone.clone()) {
                    eprintln!("スーパーノードの起動に失敗しました: {}", e);
                }
                break;
            }
        });

        // スーパーノードに接続し、エラーが発生した場合に再試行する
        loop {
            if let Err(e) =
                connect_to_supernode(SUPER_NODE_ADDR, peer_message, last_heartbeat.clone())
            {
                eprintln!("スーパーノードへの接続エラー: {}", e);
                thread::sleep(Duration::from_secs(2)); // リトライのための待機
            }
        }
    }

    Ok(())
}