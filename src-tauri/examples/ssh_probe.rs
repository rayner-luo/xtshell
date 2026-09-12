use std::sync::Arc;
use std::time::Duration;

struct H;
impl russh::client::Handler for H {
    type Error = russh::Error;
    async fn check_server_key(&mut self, _: &russh::keys::PublicKey) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::args().nth(1).expect("用法: ssh_probe <host> <user> <password>");
    let user = std::env::args().nth(2).expect("缺少 user 参数");
    let pass = std::env::args().nth(3).expect("缺少 password 参数");

    let config = Arc::new(russh::client::Config {
        inactivity_timeout: Some(Duration::from_secs(30)),
        ..Default::default()
    });
    println!("connecting...");
    let mut session = match russh::client::connect(config, (host.as_str(), 22u16), H).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("CONNECT FAILED: {e:?}");
            return Ok(());
        }
    };
    println!("handshake ok");

    let res = match session.authenticate_password(&user, &pass).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("AUTH ERROR: {e:?}");
            return Ok(());
        }
    };
    println!("auth success: {}", res.success());
    if !res.success() {
        eprintln!("AUTH REJECTED");
        return Ok(());
    }

    let mut ch = session.channel_open_session().await?;
    ch.request_pty(false, "xterm-256color", 80, 24, 0, 0, &[]).await?;
    ch.request_shell(false).await?;
    println!("shell started");
    ch.data("echo ok\n".as_bytes()).await?;
    let mut got = 0;
    while let Some(msg) = ch.wait().await {
        match msg {
            russh::ChannelMsg::Data { data } => {
                got += 1;
                println!("DATA: {}", String::from_utf8_lossy(&data));
            }
            russh::ChannelMsg::ExitStatus { .. } | russh::ChannelMsg::Eof | russh::ChannelMsg::Close => break,
            _ => {}
        }
        if got > 3 { break; }
    }
    let _ = session.disconnect(russh::Disconnect::ByApplication, "", "English").await;
    Ok(())
}
