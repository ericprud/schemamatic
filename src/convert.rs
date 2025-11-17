use anyhow::Context;
use serde_json::Value as JsonValue;
use serde_yaml::Mapping as YamlMapping;
use serde_yaml::Value as YamlValue;
use std::path::Path;
use serde::{Deserialize, Serialize};

/// Internal, slightly richer shape metadata used by the converters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShapeInfo {
    pub id: String,
    pub name: String,
    pub properties: Vec<PropertyInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyInfo {
    pub name: String,
    pub predicate: String,
    pub range: String, // datatype or a class name
    pub min: Option<u64>,
    pub max: Option<u64>,
}

/// Convert a rudof AST (shex_ast::Schema) into our ShapeInfo vector
pub fn shapes_from_rudof_ast(schema: &shex_ast::Schema) -> anyhow::Result<Vec<ShapeInfo>> {
    // Serialize the AST to JSON Value and use heuristics similar to the original
    let ast_json = serde_json::to_value(schema).context("serialize AST")?;
    Ok(extract_shapes_from_ast(&ast_json))
}

fn extract_shapes_from_ast(ast: &JsonValue) -> Vec<ShapeInfo> {
    use serde_json::Map as JsonMap;

    let mut shapes = Vec::new();

    fn walk_for_shapes(v: &JsonValue, out: &mut Vec<ShapeInfo>) {
        if let Some(obj) = v.as_object() {
            // find objects that look like shapeDecls or shapes
            if obj.contains_key("shapeExprs") || obj.contains_key("shapes") || obj.contains_key("shapeDecls") {
                // attempt to extract map-like entries
                for (_k, v2) in obj.iter() {
                    if let Some(m) = v2.as_object() {
                        // If children look like shapes (have expression / tripleConstraints)
                        for (label, possible_shape) in m.iter() {
                            let props = extract_props_from_shape(possible_shape);
                            if !props.is_empty() {
                                let name = label.clone();
                                out.push(ShapeInfo { id: label.clone(), name: name.clone(), properties: props });
                            }
                        }
                        return;
                    }
                }
            }

            // otherwise recursively search
            for (_k, v2) in obj.iter() { walk_for_shapes(v2, out); }
        } else if let Some(arr) = v.as_array() {
            for e in arr { walk_for_shapes(e, out); }
        }
    }

    walk_for_shapes(ast, &mut shapes);
    shapes
}

fn extract_props_from_shape(shape_val: &JsonValue) -> Vec<PropertyInfo> {
    use serde_json::Map as JsonMap;
    let mut props = Vec::new();

    if let Some(obj) = shape_val.as_object() {
        // Common locations: expression.tripleConstraints OR tripleConstraints direct
        if let Some(expr) = obj.get("expression").or_else(|| obj.get("shapeExpr")) {
            if let Some(tcs) = expr.get("tripleConstraints").or_else(|| expr.get("triple_constraints")) {
                if let Some(arr) = tcs.as_array() {
                    for tc in arr.iter() {
                        if let Some(tcobj) = tc.as_object() {
                            props.push(build_prop_from_tc(tcobj));
                        }
                    }
                }
            }

            // arrays in `items` or `expressions` sometimes used
            if props.is_empty() {
                if let Some(items) = expr.get("items").or_else(|| expr.get("expressions")) {
                    if let Some(arr) = items.as_array() {
                        for it in arr.iter() {
                            if let Some(itobj) = it.as_object() {
                                if itobj.contains_key("predicate") {
                                    props.push(build_prop_from_tc(itobj));
                                }
                            }
                        }
                    }
                }
            }
        }

        // fallback: direct tripleConstraints
        if props.is_empty() {
            if let Some(tcs) = obj.get("tripleConstraints").or_else(|| obj.get("triple_constraints")) {
                if let Some(arr) = tcs.as_array() {
                    for tc in arr.iter() {
                        if let Some(tcobj) = tc.as_object() {
                            props.push(build_prop_from_tc(tcobj));
                        }
                    }
                }
            }
        }
    }

    props
}

fn build_prop_from_tc(tcobj: &serde_json::Map<String, JsonValue>) -> PropertyInfo {
    let predicate = tcobj.get("predicate").and_then(|v| v.as_str()).unwrap_or("<unknown>").to_string();
    // property name: if a CURIE/IRI, take last segment after / or # or :
    let name = predicate.split(|c| c == '/' || c == '#' || c == ':').last().unwrap_or(&predicate).to_string();

    let range = infer_range_from_tc(tcobj);
    let min = tcobj.get("min").and_then(|v| v.as_u64());
    let max = tcobj.get("max").and_then(|v| v.as_u64());

    PropertyInfo { name, predicate, range, min, max }
}

fn infer_range_from_tc(tcobj: &serde_json::Map<String, JsonValue>) -> String {
    if let Some(dt) = tcobj.get("datatype").and_then(|v| v.as_str()) {
        match dt {
            "http://www.w3.org/2001/XMLSchema#integer" => "integer".to_string(),
            "http://www.w3.org/2001/XMLSchema#decimal" => "number".to_string(),
            "http://www.w3.org/2001/XMLSchema#boolean" => "boolean".to_string(),
            s if s.starts_with("http://www.w3.org/2001/XMLSchema#") => "string".to_string(),
            other => other.to_string(),
        }
    } else if let Some(nk) = tcobj.get("nodeKind").and_then(|v| v.as_str()) {
        match nk {
            "iri" => "string".to_string(),
            "literal" => "string".to_string(),
            _ => "string".to_string(),
        }
    } else if let Some(vc) = tcobj.get("valueClass") {
        // valueClass may be an object or a string referring to another shape
        if vc.is_string() {
            vc.as_str().unwrap().to_string()
        } else if let Some(o) = vc.as_object() {
            // might have a reference like { "type": "ShapeRef", "reference": "<label>" }
            if let Some(reference) = o.get("reference").and_then(|v| v.as_str()) {
                reference.to_string()
            } else { "string".to_string() }
        } else { "string".to_string() }
    } else { "string".to_string() }
}

/// Build a LinkML YAML document from shapes
pub fn build_linkml_doc(input: &Path, shapes: &[ShapeInfo]) -> anyhow::Result<String> {
    // Build YAML mapping using serde_yaml::Value
    let mut root = YamlMapping::new();

    let id = input.file_stem().and_then(|s| s.to_str()).unwrap_or("schema");
    root.insert(YamlValue::String("id".to_string()), YamlValue::String(id.to_string()));

    // prefixes: allow conversion back to CURIEs later
    let mut prefixes = YamlMapping::new();
    prefixes.insert(YamlValue::String("ex".to_string()), YamlValue::String("http://example.org/".to_string()));
    root.insert(YamlValue::String("prefixes".to_string()), YamlValue::Mapping(prefixes));

    // classes and slots
    let mut classes_map = YamlMapping::new();
    let mut slots_map = YamlMapping::new();

    for s in shapes.iter() {
        let class_name = s.name.clone();
        let mut class_map = YamlMapping::new();
        // slot refs
        let slot_refs: Vec<YamlValue> = s.properties.iter().map(|p| YamlValue::String(p.name.clone())).collect();
        class_map.insert(YamlValue::String("slots".to_string()), YamlValue::Sequence(slot_refs));
        classes_map.insert(YamlValue::String(class_name.clone()), YamlValue::Mapping(class_map));

        for p in s.properties.iter() {
            let mut slot_entry = YamlMapping::new();
            // range may be a data type or another class name
            let range = if p.range.contains(':') || p.range.starts_with("http") { // IRI/fq
                // preserve as IRI string in the slot mapping
                YamlValue::String(p.range.clone())
            } else {
                YamlValue::String(p.range.clone())
            };
            slot_entry.insert(YamlValue::String("range".to_string()), range);
            if let Some(min) = p.min { slot_entry.insert(YamlValue::String("min_count".to_string()), YamlValue::Number(min.into())); }
            if let Some(max) = p.max { slot_entry.insert(YamlValue::String("max_count".to_string()), YamlValue::Number(max.into())); }
            slots_map.insert(YamlValue::String(p.name.clone()), YamlValue::Mapping(slot_entry));
        }
    }

    root.insert(YamlValue::String("classes".to_string()), YamlValue::Mapping(classes_map));
    root.insert(YamlValue::String("slots".to_string()), YamlValue::Mapping(slots_map));

    let doc = YamlValue::Mapping(root);
    Ok(serde_yaml::to_string(&doc).context("serialize LinkML YAML")?)
}

/// Build a basic JSON Schema (draft-07) with definitions per shape
pub fn build_json_schema(_input: &Path, shapes: &[ShapeInfo]) -> serde_json::Value {
    use serde_json::{json, Map as JsonMap, Value as JsonValue};

    let mut defs = JsonMap::new();

    for s in shapes.iter() {
        let mut props = JsonMap::new();
        let mut required: Vec<JsonValue> = Vec::new();
        for p in s.properties.iter() {
            let jt = match p.range.as_str() {
                "integer" => json!({ "type": "integer" }),
                "number" => json!({ "type": "number" }),
                "boolean" => json!({ "type": "boolean" }),
                _ => json!({ "type": "string" }),
            };
            props.insert(p.name.clone(), jt);
            if p.min.unwrap_or(0) > 0 {
                required.push(JsonValue::String(p.name.clone()));
            }
        }
        let mut obj = JsonMap::new();
        obj.insert("type".to_string(), JsonValue::String("object".to_string()));
        obj.insert("properties".to_string(), JsonValue::Object(props));
        if !required.is_empty() { obj.insert("required".to_string(), JsonValue::Array(required)); }
        defs.insert(s.name.clone(), JsonValue::Object(obj));
    }

    let mut root = JsonMap::new();
    root.insert("$schema".to_string(), JsonValue::String("http://json-schema.org/draft-07/schema#".to_string()));
    root.insert("$id".to_string(), JsonValue::String("http://example.org/generated-schema".to_string()));
    root.insert("definitions".to_string(), JsonValue::Object(defs));

    JsonValue::Object(root)
}
