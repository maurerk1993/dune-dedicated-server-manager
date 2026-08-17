use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::store::Store;

use super::data::{self, Item};

pub const OVERRIDE_JSON_KEY: &str = "item_catalog_override_json";
pub const OVERRIDE_META_JSON_KEY: &str = "item_catalog_override_meta_json";

const ALLOWED_KEYS: &[&str] = &[
    "id",
    "name",
    "category",
    "source",
    "gradeable",
    "tier",
    "stackMax",
];
const MAX_CATALOG_ITEMS: usize = 20_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSummary {
    pub source: String,
    pub item_count: usize,
    pub gradeable_count: usize,
    pub stackable_count: usize,
    pub catalog_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogMetadata {
    pub source: String,
    pub source_url: Option<String>,
    pub source_version: Option<String>,
    pub source_hash: String,
    pub catalog_hash: String,
    pub applied_at: String,
    pub item_count: usize,
    pub gradeable_count: usize,
    pub stackable_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogStatus {
    pub active: CatalogSummary,
    pub bundled: CatalogSummary,
    pub override_meta: Option<CatalogMetadata>,
    pub override_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogChange {
    pub id: String,
    pub before: Item,
    pub after: Item,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDiff {
    pub current: CatalogSummary,
    pub candidate: CatalogSummary,
    pub source_url: Option<String>,
    pub source_version: Option<String>,
    pub added: Vec<Item>,
    pub removed: Vec<Item>,
    pub changed: Vec<CatalogChange>,
    pub warnings: Vec<String>,
    pub blocking_errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogExport {
    pub catalog_json: String,
    pub suggested_file_name: String,
    pub summary: CatalogSummary,
}

#[derive(Debug, Clone)]
pub struct ActiveCatalog {
    pub items: Vec<Item>,
    pub status: CatalogStatus,
}

pub fn active_catalog(store: &Store) -> Result<ActiveCatalog> {
    let bundled = bundled_items();
    let bundled_summary = summarize_catalog("bundled", &bundled);
    let override_json = store.get_config(OVERRIDE_JSON_KEY)?;
    let override_meta = store
        .get_config(OVERRIDE_META_JSON_KEY)?
        .and_then(|raw| serde_json::from_str::<CatalogMetadata>(&raw).ok());

    if let Some(raw) = override_json {
        match parse_and_validate_catalog_json(&raw) {
            Ok(items) => {
                let active = summarize_catalog("override", &items);
                return Ok(ActiveCatalog {
                    items,
                    status: CatalogStatus {
                        active,
                        bundled: bundled_summary,
                        override_meta,
                        override_error: None,
                    },
                });
            }
            Err(err) => {
                tracing::warn!(error = %err, "item catalog override is invalid; using bundled catalog");
                return Ok(ActiveCatalog {
                    items: bundled,
                    status: CatalogStatus {
                        active: bundled_summary.clone(),
                        bundled: bundled_summary,
                        override_meta,
                        override_error: Some(err.to_string()),
                    },
                });
            }
        }
    }

    Ok(ActiveCatalog {
        items: bundled,
        status: CatalogStatus {
            active: bundled_summary.clone(),
            bundled: bundled_summary,
            override_meta: None,
            override_error: None,
        },
    })
}

pub fn status(store: &Store) -> Result<CatalogStatus> {
    Ok(active_catalog(store)?.status)
}

pub fn diff_active_catalog(
    store: &Store,
    candidate: &Value,
    source_url: Option<String>,
    source_version: Option<String>,
) -> Result<(CatalogDiff, Vec<Item>)> {
    let active = active_catalog(store)?;
    let candidate_items = parse_and_validate_catalog_value(candidate)?;
    let diff = diff_catalog(&active.items, &candidate_items, source_url, source_version);
    Ok((diff, candidate_items))
}

pub fn apply_catalog(
    store: &Store,
    candidate: &Value,
    source_url: Option<String>,
    source_version: Option<String>,
    confirm_removals: bool,
) -> Result<CatalogStatus> {
    let (diff, candidate_items) =
        diff_active_catalog(store, candidate, source_url.clone(), source_version.clone())?;
    if !diff.blocking_errors.is_empty() {
        return Err(anyhow!(
            "catalog update blocked: {}",
            diff.blocking_errors.join("; ")
        ));
    }
    if !confirm_removals && !diff.removed.is_empty() {
        return Err(anyhow!(
            "catalog update removes {} item(s); confirm removals before applying",
            diff.removed.len()
        ));
    }

    let catalog_json = canonical_catalog_json(&candidate_items)?;
    let summary = summarize_catalog("override", &candidate_items);
    let meta = CatalogMetadata {
        source: "github-release-asset".to_string(),
        source_url,
        source_version,
        source_hash: summary.catalog_hash.clone(),
        catalog_hash: summary.catalog_hash,
        applied_at: Utc::now().to_rfc3339(),
        item_count: diff.candidate.item_count,
        gradeable_count: diff.candidate.gradeable_count,
        stackable_count: diff.candidate.stackable_count,
    };
    let meta_json = serde_json::to_string(&meta)?;
    store.set_configs(&[
        (OVERRIDE_JSON_KEY, catalog_json.as_str()),
        (OVERRIDE_META_JSON_KEY, meta_json.as_str()),
    ])?;
    status(store)
}

pub fn revert_catalog(store: &Store) -> Result<CatalogStatus> {
    store.delete_configs(&[OVERRIDE_JSON_KEY, OVERRIDE_META_JSON_KEY])?;
    status(store)
}

pub fn export_active_catalog(store: &Store) -> Result<CatalogExport> {
    let active = active_catalog(store)?;
    let catalog_json = canonical_catalog_json(&active.items)?;
    Ok(CatalogExport {
        catalog_json,
        suggested_file_name: "item-catalog.json".to_string(),
        summary: active.status.active,
    })
}

pub fn bundled_items() -> Vec<Item> {
    data::items().to_vec()
}

pub fn summarize_catalog(source: &str, items: &[Item]) -> CatalogSummary {
    CatalogSummary {
        source: source.to_string(),
        item_count: items.len(),
        gradeable_count: items.iter().filter(|item| item.gradeable).count(),
        stackable_count: items
            .iter()
            .filter(|item| item.stack_max.unwrap_or(1) > 1)
            .count(),
        catalog_hash: catalog_hash(items),
    }
}

pub fn parse_and_validate_catalog_json(raw: &str) -> Result<Vec<Item>> {
    let value = serde_json::from_str::<Value>(raw).map_err(|err| anyhow!("invalid JSON: {err}"))?;
    parse_and_validate_catalog_value(&value)
}

pub fn parse_and_validate_catalog_value(value: &Value) -> Result<Vec<Item>> {
    let rows = value
        .as_array()
        .ok_or_else(|| anyhow!("catalog root must be a JSON array"))?;
    if rows.is_empty() {
        return Err(anyhow!("catalog must contain at least one item"));
    }
    if rows.len() > MAX_CATALOG_ITEMS {
        return Err(anyhow!(
            "catalog contains {} items, over safety limit {}",
            rows.len(),
            MAX_CATALOG_ITEMS
        ));
    }

    let allowed: BTreeSet<&str> = ALLOWED_KEYS.iter().copied().collect();
    let mut items = Vec::with_capacity(rows.len());
    for (idx, row) in rows.iter().enumerate() {
        let obj = row
            .as_object()
            .ok_or_else(|| anyhow!("catalog row {} must be an object", idx + 1))?;
        reject_unknown_keys(idx, obj, &allowed)?;
        let item: Item = serde_json::from_value(row.clone())
            .map_err(|err| anyhow!("catalog row {} is invalid: {err}", idx + 1))?;
        validate_item(&item, idx)?;
        items.push(item);
    }

    validate_sentinel_items(&items)?;
    Ok(sort_items(items))
}

pub fn canonical_catalog_json(items: &[Item]) -> Result<String> {
    let sorted = sort_items(items.to_vec());
    serde_json::to_string_pretty(&sorted).map_err(Into::into)
}

pub fn diff_catalog(
    current: &[Item],
    candidate: &[Item],
    source_url: Option<String>,
    source_version: Option<String>,
) -> CatalogDiff {
    let current_sorted = sort_items(current.to_vec());
    let candidate_sorted = sort_items(candidate.to_vec());
    let current_by_id = by_id(&current_sorted);
    let candidate_by_id = by_id(&candidate_sorted);

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();

    for (id, next) in &candidate_by_id {
        match current_by_id.get(id) {
            None => added.push((**next).clone()),
            Some(prev) => {
                let fields = changed_fields(*prev, *next);
                if !fields.is_empty() {
                    changed.push(CatalogChange {
                        id: next.id.clone(),
                        before: (**prev).clone(),
                        after: (**next).clone(),
                        fields,
                    });
                }
            }
        }
    }
    for (id, prev) in &current_by_id {
        if !candidate_by_id.contains_key(id) {
            removed.push((**prev).clone());
        }
    }

    let current_summary = summarize_catalog("active", &current_sorted);
    let candidate_summary = summarize_catalog("candidate", &candidate_sorted);
    let mut warnings = Vec::new();
    let mut blocking_errors = Vec::new();

    if !removed.is_empty() {
        warnings.push(format!(
            "{} item(s) will be removed and require explicit confirmation",
            removed.len()
        ));
    }
    let candidate_duplicates = duplicate_ids(&candidate_sorted);
    if !candidate_duplicates.is_empty() {
        warnings.push(format!(
            "candidate catalog contains {} duplicate template id(s): {}",
            candidate_duplicates.len(),
            candidate_duplicates
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if candidate_summary.item_count * 10 < current_summary.item_count * 9 {
        blocking_errors.push(format!(
            "candidate catalog has {} items, more than 10% below current {}",
            candidate_summary.item_count, current_summary.item_count
        ));
    }
    if candidate_summary.gradeable_count * 4 < current_summary.gradeable_count * 3 {
        blocking_errors.push(format!(
            "candidate catalog has {} gradeable items, more than 25% below current {}",
            candidate_summary.gradeable_count, current_summary.gradeable_count
        ));
    }
    if candidate_summary.stackable_count * 4 < current_summary.stackable_count * 3 {
        blocking_errors.push(format!(
            "candidate catalog has {} stackable items, more than 25% below current {}",
            candidate_summary.stackable_count, current_summary.stackable_count
        ));
    }

    CatalogDiff {
        current: current_summary,
        candidate: candidate_summary,
        source_url,
        source_version,
        added,
        removed,
        changed,
        warnings,
        blocking_errors,
    }
}

fn reject_unknown_keys(
    idx: usize,
    obj: &Map<String, Value>,
    allowed: &BTreeSet<&str>,
) -> Result<()> {
    for key in obj.keys() {
        if !allowed.contains(key.as_str()) {
            return Err(anyhow!("catalog row {} has unknown field {}", idx + 1, key));
        }
    }
    Ok(())
}

fn validate_item(item: &Item, idx: usize) -> Result<()> {
    let row = idx + 1;
    if item.id.trim().is_empty() {
        return Err(anyhow!("catalog row {row} has empty id"));
    }
    if item.name.trim().is_empty() {
        return Err(anyhow!("catalog row {row} ({}) has empty name", item.id));
    }
    if item.category.trim().is_empty() {
        return Err(anyhow!(
            "catalog row {row} ({}) has empty category",
            item.id
        ));
    }
    if item.source.trim().is_empty() {
        return Err(anyhow!("catalog row {row} ({}) has empty source", item.id));
    }
    if item.tier == Some(0) {
        return Err(anyhow!(
            "catalog row {row} ({}) has invalid tier 0",
            item.id
        ));
    }
    if item.stack_max == Some(0) {
        return Err(anyhow!(
            "catalog row {row} ({}) has invalid stackMax 0",
            item.id
        ));
    }
    if item.gradeable && item.stack_max != Some(1) {
        return Err(anyhow!(
            "catalog row {row} ({}) is gradeable but does not declare stackMax 1",
            item.id
        ));
    }
    Ok(())
}

fn validate_sentinel_items(items: &[Item]) -> Result<()> {
    let Some(augment) = data::find_item_in(items, "T6_Augment_Acuracy1") else {
        return Err(anyhow!(
            "sentinel item T6_Augment_Acuracy1 is missing from catalog"
        ));
    };
    if !augment.gradeable || augment.tier != Some(6) || augment.stack_max != Some(1) {
        return Err(anyhow!(
            "sentinel item T6_Augment_Acuracy1 lost gradeable/tier/stack metadata"
        ));
    }

    let Some(aluminum) = data::find_item_in(items, "AluminiumBar") else {
        return Err(anyhow!(
            "sentinel item AluminiumBar is missing from catalog"
        ));
    };
    if aluminum.stack_max != Some(500) {
        return Err(anyhow!("sentinel item AluminiumBar lost stackMax 500"));
    }
    Ok(())
}

fn changed_fields(before: &Item, after: &Item) -> Vec<String> {
    let mut fields = Vec::new();
    if before.name != after.name {
        fields.push("name".to_string());
    }
    if before.category != after.category {
        fields.push("category".to_string());
    }
    if before.source != after.source {
        fields.push("source".to_string());
    }
    if before.gradeable != after.gradeable {
        fields.push("gradeable".to_string());
    }
    if before.tier != after.tier {
        fields.push("tier".to_string());
    }
    if before.stack_max != after.stack_max {
        fields.push("stackMax".to_string());
    }
    fields
}

fn sort_items(mut items: Vec<Item>) -> Vec<Item> {
    items.sort_by(|a, b| a.id.to_lowercase().cmp(&b.id.to_lowercase()));
    items
}

fn by_id(items: &[Item]) -> BTreeMap<String, &Item> {
    items
        .iter()
        .map(|item| (item.id.to_ascii_lowercase(), item))
        .collect()
}

fn duplicate_ids(items: &[Item]) -> Vec<String> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for item in items {
        *counts.entry(item.id.to_ascii_lowercase()).or_default() += 1;
    }
    counts
        .into_iter()
        .filter_map(|(id, count)| if count > 1 { Some(id) } else { None })
        .collect()
}

fn catalog_hash(items: &[Item]) -> String {
    let canonical = canonical_catalog_json(items).unwrap_or_else(|_| "[]".to_string());
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in canonical.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_catalog_validates_and_summarizes() {
        let raw = canonical_catalog_json(data::items()).unwrap();
        let items = parse_and_validate_catalog_json(&raw).unwrap();
        let summary = summarize_catalog("test", &items);
        assert_eq!(summary.item_count, data::items().len());
        assert!(summary.gradeable_count > 0);
        assert!(summary.stackable_count > 0);
    }

    #[test]
    fn validation_allows_duplicate_template_ids_with_other_valid_metadata() {
        let mut items = bundled_items();
        items.push(items[0].clone());
        let raw = canonical_catalog_json(&items).unwrap();
        let parsed = parse_and_validate_catalog_json(&raw).unwrap();
        assert!(duplicate_ids(&parsed).len() >= 1);
    }

    #[test]
    fn validation_rejects_missing_sentinel_metadata() {
        let mut items = bundled_items();
        let target = items
            .iter_mut()
            .find(|item| item.id == "T6_Augment_Acuracy1")
            .unwrap();
        target.gradeable = false;
        let raw = canonical_catalog_json(&items).unwrap();
        let err = parse_and_validate_catalog_json(&raw).unwrap_err();
        assert!(err.to_string().contains("sentinel item"));
    }

    #[test]
    fn diff_reports_added_changed_and_removed_items() {
        let mut current = bundled_items();
        let removed = current.pop().unwrap();
        let mut candidate = current.clone();
        candidate[0].name = format!("{} Updated", candidate[0].name);
        candidate.push(Item {
            id: "TestNewItem".to_string(),
            name: "Test New Item".to_string(),
            category: "test".to_string(),
            source: "test".to_string(),
            gradeable: false,
            tier: None,
            stack_max: Some(1),
        });

        let diff = diff_catalog(&[current, vec![removed]].concat(), &candidate, None, None);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.changed.len(), 1);
        assert!(diff.warnings.iter().any(|w| w.contains("removed")));
    }

    #[test]
    fn apply_requires_removal_confirmation() {
        let store = Store::open(&temp_store_path()).unwrap();
        let mut candidate = bundled_items();
        candidate.pop();
        let value = serde_json::to_value(candidate).unwrap();
        let err = apply_catalog(&store, &value, None, None, false).unwrap_err();
        assert!(err.to_string().contains("confirm removals"));
    }

    #[test]
    fn invalid_override_falls_back_to_bundled() {
        let store = Store::open(&temp_store_path()).unwrap();
        store.set_config(OVERRIDE_JSON_KEY, "not-json").unwrap();
        let active = active_catalog(&store).unwrap();
        assert_eq!(active.status.active.source, "bundled");
        assert!(active.status.override_error.is_some());
    }

    fn temp_store_path() -> std::path::PathBuf {
        let unique = format!(
            "dune-catalog-test-{}-{}.sqlite",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        std::env::temp_dir().join(unique)
    }
}
