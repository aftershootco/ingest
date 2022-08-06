use ingest::Rename;

pub fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1).take(2);
    let input = args.next().ok_or_else(|| anyhow::anyhow!("no input"))?;
    let output = args.next().ok_or_else(|| anyhow::anyhow!("no output"))?;
    let mut builder = ingest::IngestorBuilder::images();
    let rename = Rename {
        name: None,
        position: ingest::Position::Suffix,
        sequence: 5,
        zeroes: 5,
    };
    builder = builder.with_source([input]);
    // builder = builder.with_structure(ingest::Structure::Retain);
    builder = builder.with_structure(ingest::Structure::Rename(rename));
    builder = builder.with_target(output);
    let mut ingest = builder.build()?;
    ingest.ingest()?;
    Ok(())
}
