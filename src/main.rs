use ingest::Rename;

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1).take(2);
    let input = args.next().ok_or_else(|| anyhow::anyhow!("no input"))?;
    let output = args.next().ok_or_else(|| anyhow::anyhow!("no output"))?;
    let rename = Rename {
        name: Some("image"),
        position: ingest::Position::Suffix,
        sequence: 1,
        zeroes: 5,
    };
    let mut ingestor = ingest::IngestorBuilder::default()
        .with_filter(ingest::Filter::default())
        .with_source([&input])
        .with_structure(ingest::Structure::Rename(rename))
        .with_target(output)
        .build()?;
    // let mut ingest = builder.build()?;
    ingestor.ingest().await?;
    Ok(())
}
