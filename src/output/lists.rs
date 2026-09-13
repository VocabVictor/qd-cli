use crate::*;

pub(crate) fn print_project_list(response: &Value) -> Result<()> {
    let projects = response_array(response, &["/data/projectList", "/data/list"]);
    let rows = projects
        .iter()
        .map(|project| {
            let status = joined_fields(project, &["jobenvStatus", "jobenvSubStatus"], " / ");
            vec![
                field_text(project, &["projectName", "name"]),
                field_text(project, &["projectId", "id"]),
                if status == "-" {
                    "未创建".to_owned()
                } else {
                    status
                },
                field_text(project, &["jobCount"]),
                field_text(project, &["spaceName", "spaceId"]),
                field_text(project, &["ownerDisplayName", "createDisplayName"]),
            ]
        })
        .collect::<Vec<_>>();
    let count = response_count(response, &["/data/listCount", "/data/total"], rows.len());
    print_pretty_table(
        &format!("项目列表（共 {count} 个）"),
        &[
            "项目名称",
            "项目 ID",
            "开发环境",
            "任务数",
            "空间",
            "负责人",
        ],
        &rows,
        &[30, 22, 20, 8, 18, 14],
    );
    Ok(())
}

pub(crate) fn print_job_list(response: &Value) -> Result<()> {
    let jobs = response_array(response, &["/data/jobList", "/data/list"]);
    let rows = jobs
        .iter()
        .map(|job| {
            vec![
                field_text(job, &["jobName", "name"]),
                field_text(job, &["jobId", "id"]),
                field_text(job, &["projectName", "projectId"]),
                field_text(job, &["statusName", "status"]),
                field_text(job, &["rsgroupName", "rsgroupId"]),
                field_text(job, &["statGpu", "gpuCount", "GPU"]),
                format_cpu_field(job),
            ]
        })
        .collect::<Vec<_>>();
    let count = response_count(response, &["/data/listCount", "/data/total"], rows.len());
    print_pretty_table(
        &format!("任务列表（共 {count} 个）"),
        &[
            "任务名称",
            "任务 ID",
            "项目",
            "状态",
            "资源组",
            "GPU",
            "CPU",
        ],
        &rows,
        &[28, 22, 24, 16, 20, 20, 12],
    );
    Ok(())
}

pub(crate) fn print_dev_list(response: &Value) -> Result<()> {
    let environments = response_array(response, &["/data/devList"]);
    let rows = environments
        .iter()
        .map(|environment| {
            let status = joined_fields(environment, &["jobenvStatus", "jobenvSubStatus"], " / ");
            vec![
                field_text(environment, &["projectName", "projectId"]),
                field_text(environment, &["jobenvId"]),
                status,
                field_text(environment, &["jobenvJobId"]),
                field_text(environment, &["ownerDisplayName", "createDisplayName"]),
            ]
        })
        .collect::<Vec<_>>();
    let count = response_count(response, &["/data/listCount"], rows.len());
    print_pretty_table(
        &format!("开发环境列表（共 {count} 个）"),
        &["项目", "环境 ID", "状态", "调度任务 ID", "负责人"],
        &rows,
        &[30, 14, 22, 22, 14],
    );
    Ok(())
}

pub(crate) fn print_resource_group_list(response: &Value) -> Result<()> {
    let groups = response_array(response, &["/data/rsgroupList", "/data/list"]);
    let rows = groups
        .iter()
        .map(|group| {
            let cpu = numeric_field(group, &["cpuTotal"])
                .map(|value| format!("{} 核", format_number(value / 1000.0)))
                .unwrap_or_else(|| "-".to_owned());
            let memory = numeric_field(group, &["memoryTotal"])
                .map(format_memory_mib)
                .unwrap_or_else(|| "-".to_owned());
            let gpu_count = numeric_field(group, &["gpuTotal"]).unwrap_or(0.0);
            let gpu_model = field_text(group, &["graphicsCards"]);
            let gpu = if gpu_count <= 0.0 {
                "无 GPU".to_owned()
            } else if gpu_model == "-" {
                format!("{} 张", format_number(gpu_count))
            } else {
                format!("{gpu_model} × {}", format_number(gpu_count))
            };
            vec![
                field_text(group, &["rsgroupName", "name"]),
                field_text(group, &["rsgroupId", "id"]),
                field_text(group, &["nodeCount"]),
                cpu,
                memory,
                gpu,
            ]
        })
        .collect::<Vec<_>>();
    let count = response_count(response, &["/data/listCount", "/data/total"], rows.len());
    print_pretty_table(
        &format!("资源组列表（共 {count} 个）"),
        &[
            "资源组",
            "资源组 ID",
            "节点",
            "CPU 总量",
            "内存总量",
            "GPU 总量",
        ],
        &rows,
        &[24, 22, 8, 14, 14, 36],
    );
    Ok(())
}

pub(crate) fn print_image_list(response: &Value) -> Result<()> {
    let images = response_array(response, &["/data/jobenvImageList", "/data/imageList"]);
    let rows = images
        .iter()
        .map(|image| {
            vec![
                field_text(
                    image,
                    &["repositoryDisplayName", "imageName", "imageDesc", "name"],
                ),
                field_text(image, &["imageId", "id"]),
                field_text(image, &["jobenvName", "sourceName", "imageSource"]),
                numeric_field(image, &["imageSize", "size"])
                    .map(format_bytes)
                    .unwrap_or_else(|| "-".to_owned()),
                field_text(image, &["createDisplayName", "ownerDisplayName", "creator"]),
            ]
        })
        .collect::<Vec<_>>();
    let count = response_count(
        response,
        &["/data/totalCount", "/data/listCount", "/data/total"],
        rows.len(),
    );
    print_pretty_table(
        &format!("镜像列表（共 {count} 个）"),
        &["镜像", "镜像 ID", "来源环境", "大小", "创建人"],
        &rows,
        &[32, 22, 24, 12, 14],
    );
    Ok(())
}

pub(crate) fn print_image_repository_list(response: &Value) -> Result<()> {
    let repositories = response_array(
        response,
        &[
            "/data/imageRepositoryList",
            "/data/repositoryList",
            "/data/list",
        ],
    );
    let rows = repositories
        .iter()
        .map(|repository| {
            vec![
                field_text(
                    repository,
                    &["repositoryDisplayName", "repositoryName", "name"],
                ),
                field_text(repository, &["imageRepositoryId", "repositoryId", "id"]),
                field_text(
                    repository,
                    &["repositoryAddress", "repositoryUrl", "address", "url"],
                ),
                field_text(
                    repository,
                    &["createDisplayName", "ownerDisplayName", "creator"],
                ),
            ]
        })
        .collect::<Vec<_>>();
    let count = response_count(
        response,
        &["/data/totalCount", "/data/listCount", "/data/total"],
        rows.len(),
    );
    print_pretty_table(
        &format!("镜像仓库列表（共 {count} 个）"),
        &["仓库", "仓库 ID", "地址", "创建人"],
        &rows,
        &[28, 22, 44, 14],
    );
    Ok(())
}
