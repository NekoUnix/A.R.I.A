//! Authored display names and nested parameter folders from a Cubism CDI export.
use anyhow::{Result, ensure};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default, Debug)]
pub struct DisplayInfo {
    pub labels: BTreeMap<String, String>,
    pub parameter_groups: BTreeMap<String, String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct File {
    #[serde(default)]
    parameters: Vec<Entry>,
    #[serde(default)]
    parts: Vec<Entry>,
    #[serde(default)]
    parameter_groups: Vec<Entry>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Entry {
    id: String,
    name: String,
    #[serde(default)]
    group_id: String,
}
impl DisplayInfo {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= aria_core::asset_limits::MODEL_JSON,
            "Display info is too large"
        );
        let file: File = serde_json::from_slice(bytes)?;
        ensure!(
            file.parameters.len() <= 4096
                && file.parts.len() <= 8192
                && file.parameter_groups.len() <= 4096,
            "Too many display-info entries"
        );
        for entry in file
            .parameters
            .iter()
            .chain(&file.parts)
            .chain(&file.parameter_groups)
        {
            ensure!(
                !entry.id.is_empty()
                    && entry.id.len() <= 256
                    && entry.name.len() <= 1024
                    && entry.group_id.len() <= 256,
                "Invalid display-info entry"
            );
        }
        let folders: BTreeMap<_, _> = file
            .parameter_groups
            .iter()
            .map(|g| (g.id.as_str(), g))
            .collect();
        let mut info = Self::default();
        for parameter in &file.parameters {
            let mut path = Vec::new();
            let mut parent = parameter.group_id.as_str();
            let mut visited = BTreeSet::new();
            // Bad or cyclic folder metadata must not prevent the avatar loading.
            while let Some(group) = folders.get(parent) {
                if path.len() == 16 || !visited.insert(parent) {
                    break;
                }
                path.push(group.name.as_str());
                parent = &group.group_id;
            }
            path.reverse();
            if !path.is_empty() {
                info.parameter_groups
                    .insert(parameter.id.clone(), path.join(" / "));
            }
        }
        info.labels = file
            .parameters
            .into_iter()
            .chain(file.parts)
            .map(|e| (e.id, e.name))
            .collect();
        Ok(info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_folders_and_cycles_are_bounded() {
        let info = DisplayInfo::decode(br#"{"Parameters":[{"Id":"P","Name":"Hair style","GroupId":"child"}],"ParameterGroups":[{"Id":"root","Name":"Customize"},{"Id":"child","Name":"Hair","GroupId":"root"}],"Parts":[{"Id":"Part","Name":"Jacket"}]}"#).unwrap();
        assert_eq!(info.parameter_groups["P"], "Customize / Hair");
        assert_eq!(info.labels["Part"], "Jacket");
        let cycle = DisplayInfo::decode(br#"{"Parameters":[{"Id":"P","Name":"Option","GroupId":"a"}],"ParameterGroups":[{"Id":"a","Name":"A","GroupId":"b"},{"Id":"b","Name":"B","GroupId":"a"}]}"#).unwrap();
        assert_eq!(cycle.parameter_groups["P"], "B / A");
    }
}
