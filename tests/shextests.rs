use std::fs;
use std::path::Path;
//use clap::ValueHint::Url;
use serde_yaml::Value as Yaml;
use serde_json::Value as Json;
use shex2linkml::{convert, self};
use url;

#[test]
fn test_basic_roundtrip() {
    let shex = r#"
        PREFIX ex: <http://example.org/ns/2#>
        ex:Person {
          ex:name xsd:string ;
          ex:age xsd:integer ? ;
        }
    "#;

    // Parse to AST and convert to LinkML
    let base = url::Url::parse("http://schema.example/ns/1").unwrap();
    let base_iri = iri_s::iris::IriS::from_url(&base);
    let schema = shex_compact::ShExParser::parse(shex, None, &base_iri).expect("parse shex");
    let shapes = convert::shapes_from_rudof_ast(&schema);
    let base_string = base_iri.to_string();
    let path = Path::new(base_string.as_str());
    let linkml = convert::build_linkml_doc(path, shapes.unwrap().as_slice()).unwrap();

    // Serialize LinkML
    let linkml_yaml = serde_yaml::to_string(&linkml).unwrap();

    // Convert LinkML back to ShEx
    let linkml_value: Yaml = serde_yaml::from_str(&linkml_yaml).unwrap();
    let shex2 = shex2linkml::linkml_yaml_to_shex(linkml_yaml.as_str()).unwrap();

    // Ensure output contains expected shape label
    assert!(shex2.contains("Person"));
}
/*
#[test]
fn test_json_schema_generation() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        ex:Book {
          ex:title xsd:string ;
        }
    "#;

    let ast = convert::parse_shex(shex).expect("parse shex");
    let json_schema: Json = convert::shex_ast_to_json_schema(&ast, None);

    // basic checks
    assert!(json_schema.get("$schema").is_some());
    assert!(json_schema.get("definitions").is_some());
}

#[test]
fn test_linkml_basic_structure() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        ex:City {
          ex:population xsd:integer ;
        }
    "#;

    let ast = convert::parse_shex(shex).expect("parse shex");
    let linkml = convert::shex_ast_to_linkml(&ast, None);

    let classes = linkml.get("classes").unwrap();
    assert!(classes.is_object());

    let city = classes.get("City").unwrap();
    assert!(city.get("slots").is_some());
}
*/