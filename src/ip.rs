use reqwest::Error;
use std::net::{IpAddr, Ipv4Addr};

pub async fn get_ip() -> Result<(IpAddr, String), Error> {
    let local_ip = local_ip_address::local_ip().unwrap_or(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)));
    // グローバルIPアドレスを取得
    let global_ip = reqwest::get("https://api.ipify.org").await?.text().await?;
    Ok((local_ip, global_ip))
}
