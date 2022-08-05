pub fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1).take(2);
    let input = args.next().ok_or_else(|| anyhow::anyhow!("no input"))?;
    let output = args.next().ok_or_else(|| anyhow::anyhow!("no output"))?;
    ingest::copy_files_with_structure(input, output)?;
    Ok(())
}
