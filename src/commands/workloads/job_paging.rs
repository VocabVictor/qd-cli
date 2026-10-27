//! 作业列表的原生分页。平台的 /project/job/list/{项目} 原生支持 pageNum/pageSize、
//! jobStatus（单个状态，大小写不敏感）和 keyWords（作业名子串），未结束的作业排最前、其余按 createTime 倒序返回，
//! listCount 是筛选后的精确总数。跨项目汇总时没有原生的全局排序，所以从各项目只取目标页
//! 需要的原生页，再做 k 路归并；总数用各项目 listCount 求和，仍然精确。
use crate::*;

use futures_util::stream;
use serde_json::Map;
use std::future::Future;

/// 一个原生分页来源：某个项目在某组筛选条件下的作业列表。
pub(crate) struct JobSource {
    pub(crate) api: ApiClient,
    pub(crate) project_id: String,
    pub(crate) project_name: String,
    pub(crate) status: Option<String>,
    pub(crate) keyword: Option<String>,
    /// 合并进每条作业的附加字段（例如 spaceId / spaceName），已有字段不覆盖。
    pub(crate) tag: Map<String, Value>,
}

pub(crate) struct JobPage {
    pub(crate) jobs: Vec<Value>,
    pub(crate) total: u64,
    pub(crate) skipped: u64,
}

/// 单次向平台要的原生页大小上限：再大就分多次取。
const MAX_CHUNK: u64 = 500;

pub(crate) async fn fetch_native(source: &JobSource, page_num: u64, page_size: u64) -> Result<(Vec<Value>, u64)> {
    let mut query = json!({"pageNum": page_num, "pageSize": page_size});
    if let Some(status) = &source.status {
        query["jobStatus"] = Value::String(status.clone());
    }
    if let Some(keyword) = &source.keyword {
        query["keyWords"] = Value::String(keyword.clone());
    }
    let path = format!("/project/job/list/{}", source.project_id);
    let response = match source.api.get(Service::Core, &path, query).await {
        Ok(response) => response,
        // 平台对无权查看的项目返回 101000/121000「操作权限不足」：这些作业本来就看不到，
        // 当作空项目，不算读取失败。
        Err(err) if is_denied(&format!("{err:#}")) => return Ok((Vec::new(), 0)),
        Err(err) => return Err(err),
    };
    let total = response.pointer("/data/listCount").and_then(value_u64).unwrap_or(0);
    let mut jobs = response_array(&response, &["/data/jobList", "/data/list"]).to_vec();
    for job in &mut jobs {
        if let Some(object) = job.as_object_mut() {
            object.entry("projectId").or_insert_with(|| Value::String(source.project_id.clone()));
            object.entry("projectName").or_insert_with(|| Value::String(source.project_name.clone()));
            for (key, value) in &source.tag {
                object.entry(key.clone()).or_insert_with(|| value.clone());
            }
        }
    }
    Ok((jobs, total))
}

fn is_denied(message: &str) -> bool {
    message.contains("101000") || message.contains("121000") || message.contains("权限不足")
}

/// 把作用域（mine/shared/public/all）内的项目展开成分页来源。项目列表自带 jobCount，
/// 为 0 的项目不会有任何作业，直接跳过，省掉大量空请求。
pub(crate) async fn job_sources(
    api: &ApiClient,
    scope: &str,
    statuses: &[String],
    keyword: Option<&str>,
    tag: Map<String, Value>,
) -> Result<Vec<JobSource>> {
    let mut sources = Vec::new();
    for project in scoped_projects(api, scope).await? {
        if project.get("jobCount").and_then(value_u64) == Some(0) {
            continue;
        }
        let project_id = field_text(&project, &["projectId", "id"]);
        if project_id == "-" || validate_id(&project_id, "PROJECT_ID").is_err() {
            continue;
        }
        let project_name = field_text(&project, &["projectName", "name"]);
        let mut tag = tag.clone();
        for key in ["spaceId", "spaceName"] {
            if let Some(value) = project.get(key).filter(|value| !value.is_null()) {
                tag.insert(key.to_owned(), value.clone());
            }
        }
        let filters: Vec<Option<String>> = if statuses.is_empty() {
            vec![None]
        } else {
            statuses.iter().cloned().map(Some).collect()
        };
        for status in filters {
            sources.push(JobSource {
                api: api.clone(),
                project_id: project_id.clone(),
                project_name: project_name.clone(),
                status,
                keyword: keyword.map(str::to_owned),
                tag: tag.clone(),
            });
        }
    }
    Ok(sources)
}

/// 取汇总后的第 page 页（每页 size 条，按创建时间倒序）。
pub(crate) async fn merged_job_page(sources: Vec<JobSource>, page: u64, size: u64, concurrency: usize) -> JobPage {
    let need = page.max(1).saturating_mul(size);
    let chunk = need.clamp(1, MAX_CHUNK);
    let sources = &sources;
    let (jobs, total, skipped) =
        merge_pages(sources.len(), page, size, chunk, concurrency, |i, p, s| fetch_native(&sources[i], p, s)).await;
    JobPage { jobs, total, skipped }
}

/// 平台原生排序：未结束的作业（运行中、排队、停止中）排在最前，其余按创建时间倒序。
/// 归并必须用同一个顺序，否则会和各项目自己的原生页对不上。
fn sort_key(job: &Value) -> (u8, u64) {
    let active = matches!(
        job.get("status").and_then(Value::as_str).unwrap_or(""),
        "RUNNING" | "CREATED" | "WAITING" | "STOPPING"
    );
    (active as u8, job.get("createTime").and_then(value_u64).unwrap_or(0))
}

struct Cursor {
    buf: VecDeque<Value>,
    fetched: u64,
    total: u64,
    next_page: u64,
}

/// k 路归并。fetch(i, pageNum, pageSize) 取第 i 个来源的一个原生页，返回 (条目, 原生总数)。
/// 首轮并发取每个来源的第一页；之后只在某个来源的缓冲耗尽、且它后面确实还有数据时才补取。
/// 返回 (本页条目, 总数, 失败跳过的来源数)。
pub(crate) async fn merge_pages<F, Fut>(n: usize, page: u64, size: u64, chunk: u64, concurrency: usize, fetch: F) -> (Vec<Value>, u64, u64)
where
    F: Fn(usize, u64, u64) -> Fut,
    Fut: Future<Output = Result<(Vec<Value>, u64)>>,
{
    let need = page.max(1).saturating_mul(size);
    let offset = need.saturating_sub(size) as usize;
    let fetch = &fetch;
    let first: Vec<(usize, Result<(Vec<Value>, u64)>)> = stream::iter((0..n).map(|i| async move { (i, fetch(i, 1, chunk).await) }))
        .buffer_unordered(concurrency.max(1))
        .collect()
        .await;
    let mut cursors: Vec<Option<Cursor>> = (0..n).map(|_| None).collect();
    let mut skipped = 0u64;
    for (i, result) in first {
        match result {
            Ok((jobs, total)) => {
                cursors[i] = Some(Cursor { fetched: jobs.len() as u64, buf: jobs.into(), total, next_page: 2 });
            }
            Err(_) => skipped += 1,
        }
    }
    let total = cursors.iter().flatten().map(|c| c.total).sum();

    let mut out = Vec::new();
    while (out.len() as u64) < need {
        for (i, slot) in cursors.iter_mut().enumerate() {
            let Some(cursor) = slot else { continue };
            if !cursor.buf.is_empty() || cursor.fetched >= cursor.total {
                continue;
            }
            match fetch(i, cursor.next_page, chunk).await {
                Ok((jobs, _)) if !jobs.is_empty() => {
                    cursor.fetched += jobs.len() as u64;
                    cursor.next_page += 1;
                    cursor.buf.extend(jobs);
                }
                // 补取失败或平台提前返回空页：当作这个来源已经取完，不让整页失败。
                _ => cursor.fetched = cursor.total,
            }
        }
        let best = cursors
            .iter()
            .enumerate()
            .filter_map(|(i, c)| c.as_ref().and_then(|c| c.buf.front().map(|job| (i, sort_key(job)))))
            .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)));
        let Some((i, _)) = best else { break };
        if let Some(job) = cursors[i].as_mut().and_then(|c| c.buf.pop_front()) {
            out.push(job);
        }
    }
    (out.into_iter().skip(offset).collect(), total, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// 每个来源是已按 createTime 倒序排好的时间戳；None 表示这个来源请求失败。
    fn run(data: &[Option<Vec<u64>>], page: u64, size: u64, chunk: u64) -> (Vec<u64>, u64, u64, Vec<(usize, u64, u64)>) {
        let calls = RefCell::new(Vec::new());
        let fetch = |i: usize, p: u64, s: u64| {
            calls.borrow_mut().push((i, p, s));
            let result = data[i].as_ref().map(|times| {
                let start = ((p - 1) * s) as usize;
                let items = times.iter().skip(start).take(s as usize).map(|t| json!({"createTime": t, "src": i})).collect();
                (items, times.len() as u64)
            });
            async move { result.ok_or_else(|| anyhow::anyhow!("boom")) }
        };
        let (jobs, total, skipped) = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(merge_pages(data.len(), page, size, chunk, 4, fetch));
        let times = jobs.iter().map(|j| j["createTime"].as_u64().unwrap()).collect();
        (times, total, skipped, calls.into_inner())
    }

    fn sources() -> Vec<Option<Vec<u64>>> {
        vec![Some(vec![100, 70, 40, 10]), Some(vec![90, 80, 30]), Some(vec![60, 50, 20])]
    }

    #[test]
    fn first_page_order() {
        let (times, total, skipped, _) = run(&sources(), 1, 4, 4);
        assert_eq!(times, vec![100, 90, 80, 70]);
        assert_eq!((total, skipped), (10, 0));
    }

    #[test]
    fn later_pages_order() {
        assert_eq!(run(&sources(), 2, 4, 8).0, vec![60, 50, 40, 30]);
        assert_eq!(run(&sources(), 3, 4, 12).0, vec![20, 10]);
        assert!(run(&sources(), 4, 4, 16).0.is_empty());
    }

    #[test]
    fn lazy_chunk_refill() {
        let (times, _, _, calls) = run(&sources(), 1, 3, 1);
        assert_eq!(times, vec![100, 90, 80]);
        // 来源 2 的最新一条 (60) 比本页最后一条还旧，不应该被补取第二页
        assert!(!calls.contains(&(2, 2, 1)), "{calls:?}");
        assert!(calls.contains(&(1, 2, 1)));
    }

    #[test]
    fn running_first_order() {
        // 和平台一致：运行中的作业即使创建得更早也排在最前
        let calls = RefCell::new(0);
        let data = vec![
            vec![json!({"createTime": 50, "status": "RUNNING"}), json!({"createTime": 90, "status": "SUCCEEDED"})],
            vec![json!({"createTime": 70, "status": "FAILED"})],
        ];
        let fetch = |i: usize, p: u64, s: u64| {
            *calls.borrow_mut() += 1;
            let items: Vec<Value> = data[i].iter().skip(((p - 1) * s) as usize).take(s as usize).cloned().collect();
            let total = data[i].len() as u64;
            async move { Ok::<_, anyhow::Error>((items, total)) }
        };
        let (jobs, total, _) = tokio::runtime::Builder::new_current_thread().build().unwrap()
            .block_on(merge_pages(2, 1, 3, 3, 2, fetch));
        let times: Vec<u64> = jobs.iter().map(|j| j["createTime"].as_u64().unwrap()).collect();
        assert_eq!((times, total), (vec![50, 90, 70], 3));
    }

    #[test]
    fn skips_failed_source() {
        let data = vec![Some(vec![100, 40]), None, Some(vec![60])];
        let (times, total, skipped, _) = run(&data, 1, 5, 5);
        assert_eq!(times, vec![100, 60, 40]);
        assert_eq!((total, skipped), (3, 1));
    }

    #[test]
    fn minimal_page_size() {
        let (_, _, _, calls) = run(&sources(), 1, 2, 2);
        assert!(calls.iter().all(|(_, _, s)| *s == 2), "{calls:?}");
    }
}
