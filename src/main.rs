use clap::Parser;
use std::fs;
use std::path::PathBuf;
use anyhow::Context;
use shex2linkml::{convert, convert::*, linkml_to_shex, linkml_to_shex::*};
use iri_s::IriS;

#[derive(Parser, Debug)]
#[command(author, version, about = "Convert between ShEx (compact), LinkML, and JSON Schema using rudof AST")] 
struct Args {
    /// Input ShEx (compact) file to convert to LinkML + JSON Schema
    #[arg(value_name = "INPUT", required = false)]
    input: Option<PathBuf>,

    /// Optional LinkML output path
    #[arg(long)]
    linkml: Option<PathBuf>,

    /// Optional JSON Schema output path
    #[arg(long)]
    jsonschema: Option<PathBuf>,

    /// Optional back-conversion: convert LinkML YAML back to ShEx compact and write here
    #[arg(long)]
    back_to_shex: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if let Some(linkml_in) = args.back_to_shex {
        // The user asked only for LinkML -> ShEx conversion
        let l = fs::read_to_string(&linkml_in).context("reading LinkML")?;
        let shex = linkml_to_shex::linkml_yaml_to_shex(&l)?;
        let out = linkml_in.with_extension("shex");
        fs::write(&out, shex)?;
        println!("Wrote ShEx -> {}", out.display());
        return Ok(());
    }

    let input = match args.input {
        Some(p) => p,
        None => anyhow::bail!("No input ShEx provided. Use the --help for details."),
    };

    let input_str = fs::read_to_string(&input)?;

    // Parse ShEx compact syntax into AST using rudof's compact parser
    // The parser types come from `shex_compact` and `shex_ast` crates.
    let base_iri = iri_s::iris::IriS::from_path(input.as_path()).unwrap(); // _or_else(|e| -> anyhow::bail!(e))
    let schema: shex_ast::Schema = shex_compact::ShExParser::parse(&input_str, None, &base_iri)
        .map_err(|e| anyhow::anyhow!("failed to parse ShEx: {:?}", e))?;

    // Convert AST -> intermediate shape model
    let shapes = convert::shapes_from_rudof_ast(&schema)?;

    // Build LinkML
    let linkml = convert::build_linkml_doc(&input, &shapes)?;

    // Build JSON Schema
    let json_schema = convert::build_json_schema(&input, &shapes);

    // Write outputs
    let linkml_path = args.linkml.unwrap_or_else(|| input.with_extension("-linkml.yaml"));
    let json_path = args.jsonschema.unwrap_or_else(|| input.with_extension("-jsonschema.json"));

    fs::write(&linkml_path, linkml)?;
    fs::write(&json_path, serde_json::to_string_pretty(&json_schema)?)?;

    println!("Wrote LinkML -> {}", linkml_path.display());
    println!("Wrote JSON Schema -> {}", json_path.display());

    Ok(())
}
