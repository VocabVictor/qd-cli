use crate::*;

pub(crate) async fn bench_command(
    api: &ApiClient,
    config: &Config,
    args: &mut Args,
    compact: bool,
) -> Result<()> {
    let path = args.pop().context("bench 缺少 PATH")?;
    let service = Service::parse(
        &args
            .take_option("--service")?
            .unwrap_or_else(|| "core".into()),
    )?;
    let requests = option_u64(args, "--requests", 200, 1, 1_000_000)? as usize;
    let warmup = option_u64(args, "--warmup", 5, 0, 10_000)? as usize;
    let concurrency = command_concurrency(args, config.concurrency)?;
    let query = match args.take_option("--file")? {
        Some(path) => read_json(&path)?,
        None => empty_object(),
    };
    args.ensure_empty()?;

    for _ in 0..warmup {
        api.get(service, &path, query.clone()).await?;
    }

    let started = Instant::now();
    let mut pending = requests;
    let mut set = JoinSet::new();
    let mut latencies_us = Vec::with_capacity(requests);
    let mut failures = 0usize;

    while pending > 0 || !set.is_empty() {
        while pending > 0 && set.len() < concurrency {
            pending -= 1;
            let client = api.clone();
            let path = path.clone();
            let query = query.clone();
            set.spawn(async move {
                let request_started = Instant::now();
                let result = client.get(service, &path, query).await;
                (request_started.elapsed().as_micros(), result)
            });
        }
        if let Some(joined) = set.join_next().await {
            match joined {
                Ok((latency, Ok(_))) => latencies_us.push(latency),
                Ok((_, Err(_))) | Err(_) => failures += 1,
            }
        }
    }

    latencies_us.sort_unstable();
    let elapsed = started.elapsed();
    let successes = latencies_us.len();
    let rps = requests as f64 / elapsed.as_secs_f64().max(f64::EPSILON);
    let result = json!({
        "path": path,
        "requests": requests,
        "concurrency": concurrency,
        "successes": successes,
        "failures": failures,
        "elapsedMs": elapsed.as_secs_f64() * 1000.0,
        "requestsPerSecond": rps,
        "latencyMs": {
            "p50": percentile_us(&latencies_us, 50) as f64 / 1000.0,
            "p95": percentile_us(&latencies_us, 95) as f64 / 1000.0,
            "p99": percentile_us(&latencies_us, 99) as f64 / 1000.0,
            "max": latencies_us.last().copied().unwrap_or(0) as f64 / 1000.0
        }
    });
    print_json(&result, compact)?;
    if failures > 0 {
        bail!("性能测试有 {failures} 个请求失败");
    }
    Ok(())
}

pub(crate) fn percentile_us(values: &[u128], percentile: usize) -> u128 {
    if values.is_empty() {
        return 0;
    }
    let index = ((values.len() - 1) * percentile).div_ceil(100);
    values[index.min(values.len() - 1)]
}
