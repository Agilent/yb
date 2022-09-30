use concurrent_git_pool::client::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::connect("127.0.0.1:12345").await?;

    let p1 = client.lookup_or_clone("https://github.com/console-rs/indicatif.git");
    let p2 = client.lookup_or_clone("https://github.com/yoctoproject/poky.git");

    let ret = tokio::join!(p1, p2);
    dbg!(ret);

    eprintln!("DONE");

    Ok(())
}
