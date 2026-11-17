//! Suggest complete, unambiguous profile groups from declared model dependencies.
//! Never combine files merely because they have matching names or directories.
use std::collections::{BTreeMap, BTreeSet};

use powerio::{Collection, CollectionSelection};
use quick_xml::{events::Event, name::ResolveResult, NsReader};

const MD: &[u8] = b"http://iec.ch/TC57/61970-552/ModelDescription/1#";
const RDF: &[u8] = b"http://www.w3.org/1999/02/22-rdf-syntax-ns#";
const HEADER_BYTES: usize = 1 << 20;
const INSPECTION_BYTES: usize = 8 << 20;

#[derive(Default, Debug)]
struct Header {
    path: String,
    id: String,
    profiles: BTreeSet<String>,
    dependencies: BTreeSet<String>,
    scenario: String,
    single_model: bool,
}

fn rdf_attribute(
    reader: &NsReader<&[u8]>,
    event: &quick_xml::events::BytesStart<'_>,
    name: &[u8],
) -> Option<String> {
    for attribute in event.attributes() {
        let attribute = attribute.ok()?;
        let (namespace, local) = reader.resolver().resolve_attribute(attribute.key);
        if local.as_ref() == name
            && matches!(namespace, ResolveResult::Bound(ns) if ns.as_ref() == RDF)
        {
            return attribute
                .decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, reader.decoder())
                .ok()
                .map(|s| s.into_owned());
        }
    }
    None
}

fn header(bytes: &[u8], complete_document: bool) -> Option<Header> {
    let mut reader = NsReader::from_reader(bytes);
    let mut result = Header {
        single_model: complete_document,
        ..Header::default()
    };
    let mut depth = 0usize;
    let mut in_model = false;
    loop {
        let event = match reader.read_event() {
            Ok(event) => event,
            Err(_) => {
                result.single_model = false;
                return (!result.id.is_empty() && !result.profiles.is_empty()).then_some(result);
            }
        };
        match &event {
            Event::Start(_) => depth += 1,
            Event::End(_) => depth = depth.checked_sub(1)?,
            _ => {}
        }
        match event {
            Event::Start(event) | Event::Empty(event) => {
                let (namespace, local) = reader.resolver().resolve_element(event.name());
                if !matches!(namespace, ResolveResult::Bound(ns) if ns.as_ref() == MD) {
                    continue;
                }
                match local.as_ref() {
                    b"FullModel" if !in_model => {
                        if !result.id.is_empty() {
                            result.single_model = false;
                            return Some(result);
                        }
                        result.id = rdf_attribute(&reader, &event, b"about")?;
                        in_model = true;
                    }
                    b"Model.DependentOn" if in_model => {
                        result
                            .dependencies
                            .insert(rdf_attribute(&reader, &event, b"resource")?);
                    }
                    b"Model.profile" | b"Model.scenarioTime" if in_model => {
                        let is_profile = local.as_ref() == b"Model.profile";
                        let text = reader.read_text(event.name()).ok()?;
                        depth = depth.checked_sub(1)?;
                        let decoded = text.decode().ok()?;
                        let text = quick_xml::escape::unescape(&decoded)
                            .ok()?
                            .trim()
                            .to_owned();
                        if is_profile {
                            result.profiles.insert(text);
                        } else {
                            result.scenario = text;
                        }
                    }
                    _ => {}
                }
            }
            Event::End(event) if in_model => {
                let (namespace, local) = reader.resolver().resolve_element(event.name());
                if local.as_ref() == b"FullModel"
                    && matches!(namespace, ResolveResult::Bound(ns) if ns.as_ref() == MD)
                {
                    in_model = false;
                }
            }
            Event::Eof => {
                result.single_model &= depth == 0 && !in_model;
                return (!result.id.is_empty() && !result.profiles.is_empty()).then_some(result);
            }
            Event::DocType(_) => return None,
            _ => {}
        }
    }
}

#[derive(Default)]
pub(super) struct Groups {
    pub members: Vec<Vec<String>>,
    pub related: BTreeSet<String>,
    pub profiles: BTreeSet<String>,
    pub incomplete: bool,
    pub xml_files: usize,
}

pub(super) fn inspect(collection: &Collection) -> Result<Groups, String> {
    let mut headers = Vec::new();
    let mut remaining = INSPECTION_BYTES;
    let mut complete = true;
    let mut xml_files = 0;
    for entry in collection.entries() {
        if !entry.path.to_ascii_lowercase().ends_with(".xml") {
            continue;
        }
        xml_files += 1;
        let source = collection
            .source(&CollectionSelection::file(&entry.path))
            .map_err(|e| e.to_string())?;
        let buffer = source.primary_buffer().map_err(|e| e.to_string())?;
        let bytes = buffer.content_bytes();
        let count = bytes.len().min(HEADER_BYTES);
        if count > remaining {
            complete = false;
            continue;
        }
        remaining -= count;
        if let Some(mut parsed) = header(&bytes[..count], bytes.len() == count) {
            parsed.path = entry.path.clone();
            headers.push(parsed);
        } else {
            complete = false;
        }
    }
    let mut by_id = BTreeMap::new();
    let mut duplicate_ids = false;
    for (index, item) in headers.iter().enumerate() {
        duplicate_ids |= by_id.insert(item.id.as_str(), index).is_some();
    }
    let mut edges = vec![BTreeSet::new(); headers.len()];
    let mut result = Groups {
        xml_files,
        profiles: headers.iter().map(|header| header.path.clone()).collect(),
        incomplete: !complete || duplicate_ids || headers.iter().any(|header| !header.single_model),
        ..Groups::default()
    };
    for (index, item) in headers.iter().enumerate() {
        for dependency in &item.dependencies {
            if let Some(&other) = by_id.get(dependency.as_str()) {
                edges[index].insert(other);
                edges[other].insert(index);
                result.related.insert(item.path.clone());
                result.related.insert(headers[other].path.clone());
            }
        }
    }
    // An uninspected XML could contain another scenario or an overriding profile.
    // Leave grouping explicit rather than offer a partial automatic selection.
    if duplicate_ids {
        // Repeated model IDs cannot select one owner's companions reliably.
        result
            .related
            .extend(headers.iter().map(|header| header.path.clone()));
    }
    if !complete || duplicate_ids {
        return Ok(result);
    }
    let mut seen = BTreeSet::new();
    for start in 0..headers.len() {
        if !seen.insert(start) {
            continue;
        }
        let mut component = vec![start];
        let mut cursor = 0;
        while cursor < component.len() {
            for &other in &edges[component[cursor]] {
                if seen.insert(other) {
                    component.push(other);
                }
            }
            cursor += 1;
        }
        if component.len() < 2 {
            continue;
        }
        let mut profiles = BTreeSet::new();
        let mut scenarios = BTreeSet::new();
        let mut unambiguous = true;
        for &index in &component {
            let item = &headers[index];
            unambiguous &= item.single_model;
            unambiguous &= item
                .dependencies
                .iter()
                .all(|id| by_id.contains_key(id.as_str()));
            for profile in &item.profiles {
                unambiguous &= profiles.insert(profile);
            }
            if !item.scenario.is_empty() {
                scenarios.insert(&item.scenario);
            }
        }
        if unambiguous && scenarios.len() <= 1 {
            let mut members: Vec<_> = component
                .into_iter()
                .map(|index| headers[index].path.clone())
                .collect();
            members.sort();
            result.members.push(members);
        }
    }
    result.members.sort();
    Ok(result)
}
