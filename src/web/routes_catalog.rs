//! 节点与镜像仓库列表，都走平台原生分页。
//!
//! 节点：/jobQueue/node/job/list 是集群级的（各空间看到同一份），原生支持 pageNum/pageSize、
//! rsgroup（资源组 ID）、gpuModel（完整型号名，精确匹配）、leftGpuCount（至少空闲几卡）、
//! sortField（left_gpu_count / left_cpu / left_memory）+ sortOrder，返回 total 和筛选后的资源汇总。
//!
//! 镜像仓库（在真实集群上核对过）：current 下各空间的仓库互不重叠，直接按空间原生分页再归并；
//! public 是全局列表，每个空间返回同一份，只查一个空间；shared / mine 会有同一个仓库出现在
//! 多个空间，而且条目很少，整份取回去重后再分页，保证总数准确。
//!
//! 镜像仓库：每个空间的仓库互不重叠，各自走平台原生分页（按创建时间倒序，
//! 也就是 imageRepositoryId 倒序），跨空间用 merge_pages 归并；总数是各空间 listCount 之和。
use crate::*;
use crate::web::roles::CurrentUser;
use crate::web::state::*;

use axum::extract::{Query, State};
use axum::Extension;
use std::{collections::HashMap, sync::Arc};

/// 平台的 source 取值：current 全部可见，public 公开，shared 空间共享，mine 我创建的。
const SOURCES: [&str; 4] = ["current", "public", "shared", "mine"];

const CREATOR_FIELDS: &[&str] = &["displayName", "userName", "createUserId", "createDisplayName", "ownerDisplayName"];

/// 普通用户看不到镜像是谁建的：递归去掉所有创建人字段。
pub(crate) fn strip_creator(value: &mut Value) {
    match value {
        Value::Object(obj) => {
            for &key in CREATOR_FIELDS {
                obj.remove(key);
            }
            obj.values_mut().for_each(strip_creator);
        }
        Value::Array(items) => items.iter_mut().for_each(strip_creator),
        _ => {}
    }
}

pub(crate) async fn list_images(
    State(state): State<Arc<WebState>>,
    Extension(user): Extension<CurrentUser>,
    Query(query): Query<HashMap<String, String>>,
) -> WebResult {
    let page = query_u64(&query, "page", 1, 1, 100_000)?;
    let size = query_u64(&query, "size", 50, 1, 500)?;
    let source = query.get("source").map(String::as_str).filter(|v| !v.is_empty()).unwrap_or("current");
    if !SOURCES.contains(&source) {
        return Err(WebError::bad_request("source 只接受 current、public、shared 或 mine"));
    }
    let keyword = query.get("q").map(|v| v.trim().to_owned()).unwrap_or_default();
    if keyword.chars().count() > 64 {
        return Err(WebError::bad_request("关键字最多 64 个字"));
    }
    let only = query.get("space").filter(|v| !v.is_empty());
    let mut spaces = Vec::new();
    for space in state.spaces().await {
        if only.is_some_and(|id| *id != space.id) {
            continue;
        }
        spaces.push((state.api_for(Some(&space.id)).await?, space));
    }
    if source == "public" && only.is_none() {
        spaces.truncate(1);
    }
    let spaces = &spaces;
    let keyword = keyword.as_str();
    let fetch = |i: usize, num: u64, len: u64| {
        let (api, space) = &spaces[i];
        let body = json!({"spaceId": space.id, "source": source, "keyWords": keyword, "pageNum": num, "pageSize": len});
        async move {
            let response = api.get(Service::Core, "/imageRepository/list", body).await?;
            let total = response.pointer("/data/listCount").and_then(value_u64).unwrap_or(0);
            let mut list = response_array(&response, &["/data/imageList"]).to_vec();
            for item in &mut list {
                attach_space(item, space);
            }
            Ok((list, total))
        }
    };
    let key = |item: &Value| (0u8, item.get("imageRepositoryId").and_then(value_u64).unwrap_or(0));
    let concurrency = state.config().await.concurrency;
    let (images, total, skipped) = if matches!(source, "shared" | "mine") && spaces.len() > 1 {
        // 跨空间会重复：每个空间按原生分页取全（最多 2000 条），按仓库 ID 去重
        let mut seen = std::collections::HashSet::new();
        let mut all = Vec::new();
        let mut skipped = 0u64;
        for i in 0..spaces.len() {
            for num in 1..=4u64 {
                match fetch(i, num, 500).await {
                    Ok((list, total)) => {
                        let got = list.len() as u64;
                        for item in list {
                            if seen.insert(key(&item).1) {
                                all.push(item);
                            }
                        }
                        if got < 500 || num * 500 >= total {
                            break;
                        }
                    }
                    Err(_) => {
                        skipped += 1;
                        break;
                    }
                }
            }
        }
        all.sort_by(|a, b| key(b).cmp(&key(a)));
        let total = all.len() as u64;
        let start = ((page - 1) * size) as usize;
        (all.into_iter().skip(start).take(size as usize).collect(), total, skipped)
    } else {
        let chunk = page.saturating_mul(size).clamp(1, 500);
        merge_pages(spaces.len(), page, size, chunk, concurrency, key, fetch).await
    };
    let mut images = Value::Array(images);
    if !user.is_admin() {
        strip_creator(&mut images);
    }
    reply(json!({
        "code": 0,
        "data": {"listCount": total, "imageList": images, "page": page, "size": size, "source": source, "skippedSpaces": skipped}
    }))
}

const NODE_PATH: &str = "/jobQueue/node/job/list";

fn node_sort(raw: Option<&String>) -> Result<&'static str, WebError> {
    match raw.map(String::as_str).unwrap_or("gpu") {
        "gpu" => Ok("left_gpu_count"),
        "cpu" => Ok("left_cpu"),
        "memory" => Ok("left_memory"),
        _ => Err(WebError::bad_request("sort 只接受 gpu、cpu 或 memory")),
    }
}

/// 平台的 gpuModels / resourceGroups 是逗号分隔的字符串。
fn split_list(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items.iter().filter_map(value_string).collect(),
        Some(Value::String(text)) => text.split(',').map(str::trim).filter(|t| !t.is_empty()).map(str::to_owned).collect(),
        _ => Vec::new(),
    }
}

pub(crate) async fn list_nodes(
    State(state): State<Arc<WebState>>,
    Query(query): Query<HashMap<String, String>>,
) -> WebResult {
    let page = query_u64(&query, "page", 1, 1, 10_000)?;
    let size = query_u64(&query, "size", 50, 1, 500)?;
    let mut body = json!({"pageNum": page, "pageSize": size, "sortField": node_sort(query.get("sort"))?,
        "sortOrder": if query.get("dir").map(String::as_str) == Some("asc") { "asc" } else { "desc" }});
    if let Some(group) = query.get("group").filter(|v| !v.is_empty()) {
        validate_id(group, "RSGROUP_ID")?;
        body["rsgroup"] = Value::String(group.clone());
    }
    if let Some(model) = query.get("model").filter(|v| !v.is_empty()) {
        if model.chars().count() > 64 {
            return Err(WebError::bad_request("GPU 型号最多 64 个字"));
        }
        body["gpuModel"] = Value::String(model.clone());
    }
    let min_gpu = query_u64(&query, "minGpu", 0, 0, 64)?;
    if min_gpu > 0 {
        body["leftGpuCount"] = Value::from(min_gpu);
    }
    let api = state.api().await?;
    let response = api.get(Service::Core, NODE_PATH, body).await?;
    let data = response.get("data").cloned().unwrap_or_else(empty_object);
    let mut summary = data.as_object().cloned().unwrap_or_default();
    let nodes = summary.remove("nodes").unwrap_or_else(|| json!([]));
    reply(json!({"nodes": nodes, "summary": summary, "page": page, "size": size}))
}

/// 筛选项：全部 GPU 型号与资源组名。按原生分页翻完整个集群（约百台节点）。
pub(crate) async fn node_facets(State(state): State<Arc<WebState>>) -> WebResult {
    let api = state.api().await?;
    let mut models = std::collections::BTreeSet::new();
    let mut groups = std::collections::BTreeSet::new();
    for page in 1..=50u64 {
        let response = api.get(Service::Core, NODE_PATH, json!({"pageNum": page, "pageSize": 200})).await?;
        let nodes = response.pointer("/data/nodes").and_then(Value::as_array).cloned().unwrap_or_default();
        for node in &nodes {
            models.extend(split_list(node.get("gpuModels")));
            groups.extend(split_list(node.get("resourceGroups")));
        }
        let total = response.pointer("/data/total").and_then(value_u64).unwrap_or(0);
        if nodes.len() < 200 || page * 200 >= total {
            break;
        }
    }
    reply(json!({"models": models, "groups": groups}))
}
