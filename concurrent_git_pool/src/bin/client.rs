use git_reference_cache::client::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::new("127.0.0.1:1234").await?;

    let p1 = client.lookup_or_clone("https://github.com/console-rs/indicatif.git".into());
    let p2 = client.lookup_or_clone("https://github.com/yoctoproject/poky.git".into());

    let ret = tokio::join!(p1, p2);
    dbg!(ret);

    eprintln!("DONE");

    Ok(())
}
