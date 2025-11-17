use anyhow::Context;
use serde_yaml::Value as YamlValue;

/// Convert a LinkML YAML content string to a ShEx compact string.
/// This is a best-effort conversion assuming LinkML `classes` and `slots` sections
/// exist. Predicates will be generated using the `prefixes` mapping when available
/// (e.g., `ex:propertyName`), otherwise as `http://example.org/propertyName`.
pub fn linkml_yaml_to_shex(yaml_str: &str) -> anyhow::Result<String> {
    let doc: YamlValue = serde_yaml::from_str(yaml_str).context("parsing linkml yaml")?;

    // Extract prefixes map
    let prefixes = match doc.get("prefixes") {
        Some(YamlValue::Mapping(m)) => m.iter().filter_map(|(k,v)| {
            if let (YamlValue::String(k1), YamlValue::String(v1)) = (k.clone(), v.clone()) { Some((k1, v1)) } else { None }
        }).collect::<Vec<(String,String)>>(),
        _ => Vec::new(),
    };

    // get classes and slots
    let classes = match doc.get("classes") {
        Some(YamlValue::Mapping(m)) => m.clone(),
        _ => anyhow::bail!("LinkML YAML missing `classes` mapping"),
    };
    let slots = match doc.get("slots") {
        Some(YamlValue::Mapping(m)) => m.clone(),
        _ => serde_yaml::Mapping::new(),
    };

    // Helper to expand a slot name into a predicate IRI/curie
    let pred_for = |slot_name: &str| -> String {
        // If a prefix `ex` exists, use it
        if let Some((pfx, iri)) = prefixes.get(0) {
            format!("{}:{}", pfx, slot_name)
        } else {
            format!("http://example.org/{}", slot_name)
        }
    };

    // Build ShEx compact: one shape per class
    let mut out = String::new();

    for (class_name_val, class_entry) in classes.iter() {
        if let YamlValue::String(class_name) = class_name_val {
            out.push_str(&format!("<{}> IRI
", class_name));
            // slots: sequence of slot names
            if let YamlValue::Mapping(map) = class_entry {
                if let Some(slots_val) = map.get(&YamlValue::String("slots".to_string())) {
                    if let YamlValue::Sequence(sarr) = slots_val {
                        out.push_str("{
");
                        for s in sarr.iter() {
                            if let YamlValue::String(slot_name) = s {
                                // lookup slot definition for range/cardinality
                                let slot_def = slots.get(&YamlValue::String(slot_name.clone()));
                                let (range_str, minc, maxc) = match slot_def {
                                    Some(YamlValue::Mapping(m)) => {
                                        let range = m.get(&YamlValue::String("range".to_string())).and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or("string".to_string());
                                        let minc = m.get(&YamlValue::String("min_count".to_string())).and_then(|v| v.as_i64()).unwrap_or(0);
                                        let maxc = m.get(&YamlValue::String("max_count".to_string())).and_then(|v| v.as_i64()).unwrap_or(1);
                                        (range, minc, maxc)
                                    }
                                    _ => ("string".to_string(), 0, 1),
                                };

                                let pred = pred_for(slot_name);
                                let qc = if minc == 0 && maxc > 1 { "*" } else if minc == 1 && maxc > 1 { "+" } else if minc == 1 && maxc == 1 { "" } else { "?" };
                                // Map range back to a ShEx nodeConstraint: datatype -> xsd, otherwise assume @<shape> or IRI
                                let constraint = if range_str == "string" { "" } else if range_str == "integer" { " xsd:integer" } else { "" };

                                out.push_str(&format!("  {} {}{} ;
", pred, constraint, qc));
                            }
                        }
                        out.push_str("}

");
                    }
                }
            }
        }
    }

    Ok(out)
}
