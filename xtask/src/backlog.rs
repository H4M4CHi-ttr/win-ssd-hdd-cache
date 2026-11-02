use crate::{ManifestViolation, finish, json, violation};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
pub(crate) const MAX_BYTES: usize = 4_194_304;
const PREFIX: &str = "https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/";
struct Task {
    id: String,
    title: String,
    agent: String,
    review: String,
    status: String,
    priority: String,
    kind: String,
    area: String,
    owner: bool,
    approval: String,
    evidence: Vec<String>,
    dependencies: Vec<String>,
    files: Vec<String>,
    output: String,
    acceptance: Vec<String>,
    escalation: String,
    number: u64,
    url: String,
    completion: String,
}
const FIELDS: &[&str] = &[
    "id",
    "title",
    "agent",
    "review",
    "status",
    "priority",
    "type",
    "area",
    "owner_decision",
    "owner_approval",
    "approval_evidence",
    "dependencies",
    "files",
    "output",
    "acceptance",
    "escalation",
    "issue_number",
    "issue_url",
    "completion",
];
fn decode(input: &str) -> json::Result<(u64, String, Vec<Task>)> {
    let root = json::parse(input, MAX_BYTES)?;
    let root = json::object(&root, "", &["schema_version", "plan_date", "tasks"])?;
    let version = json::unsigned(root["schema_version"], "/schema_version")?;
    let date = json::string(root["plan_date"], "/plan_date")?;
    let nodes = json::array(root["tasks"], "/tasks")?;
    if nodes.is_empty() || nodes.len() > 1024 {
        return Err(violation("INVALID_JSON", ""));
    }
    let mut tasks = Vec::new();
    for (i, node) in nodes.iter().enumerate() {
        let path = format!("/tasks/{i}");
        let o = json::object(node, &path, FIELDS)?;
        let s = |key| json::string(o[key], &json::pointer(&path, key));
        let a = |key| json::strings(o[key], &json::pointer(&path, key));
        tasks.push(Task {
            id: s("id")?,
            title: s("title")?,
            agent: s("agent")?,
            review: s("review")?,
            status: s("status")?,
            priority: s("priority")?,
            kind: s("type")?,
            area: s("area")?,
            owner: json::boolean(o["owner_decision"], &format!("{path}/owner_decision"))?,
            approval: s("owner_approval")?,
            evidence: a("approval_evidence")?,
            dependencies: a("dependencies")?,
            files: a("files")?,
            output: s("output")?,
            acceptance: a("acceptance")?,
            escalation: s("escalation")?,
            number: json::unsigned(o["issue_number"], &format!("{path}/issue_number"))?,
            url: s("issue_url")?,
            completion: s("completion")?,
        });
    }
    Ok((version, date, tasks))
}
pub fn validate_backlog(
    input: &str,
    known_labels: &BTreeSet<String>,
) -> Result<(), Vec<ManifestViolation>> {
    count(input, known_labels).map(|_| ())
}
pub(crate) fn count(
    input: &str,
    known_labels: &BTreeSet<String>,
) -> Result<usize, Vec<ManifestViolation>> {
    let (version, date, tasks) = decode(input).map_err(|e| vec![e])?;
    let mut errors = Vec::new();
    if version != 2 {
        errors.push(violation("UNSUPPORTED_SCHEMA", "/schema_version"));
    }
    if !valid_date(&date) {
        errors.push(violation("INVALID_FIELD", "/plan_date"));
    }
    let mut ids = BTreeMap::new();
    let mut issues = BTreeSet::new();
    let mut ambiguous = false;
    for (i, t) in tasks.iter().enumerate() {
        let path = format!("/tasks/{i}");
        let p = |key| json::pointer(&path, key);
        if !(1..=32).contains(&t.id.len())
            || !t
                .id
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        {
            errors.push(violation("INVALID_FIELD", p("id")));
            ambiguous = true;
        }
        if ids.insert(t.id.as_str(), i).is_some() {
            errors.push(violation("DUPLICATE_ID", p("id")));
            ambiguous = true;
        }
        if !issues.insert(t.number) {
            errors.push(violation("DUPLICATE_ISSUE", p("issue_number")));
        }
        if t.number == 0 {
            errors.push(violation("INVALID_FIELD", p("issue_number")));
        }
        if t.url != format!("{PREFIX}issues/{}", t.number) {
            errors.push(violation("INVALID_FIELD", p("issue_url")));
        }
        for (key, value, allowed) in [
            ("agent", t.agent.as_str(), &["luna", "sol", "astra"][..]),
            ("review", t.review.as_str(), &["sol", "astra"][..]),
            (
                "status",
                t.status.as_str(),
                &["ready", "blocked", "needs-decision", "done", "cancelled"][..],
            ),
            ("priority", t.priority.as_str(), &["p0", "p1", "p2"][..]),
            ("type", t.kind.as_str(), &["design", "task", "gate"][..]),
        ] {
            if !allowed.contains(&value) {
                errors.push(violation("INVALID_FIELD", p(key)));
            }
            if !known_labels.contains(&format!("{key}:{value}")) {
                errors.push(violation("UNKNOWN_LABEL", p(key)));
            }
        }
        if !known_labels.contains(&format!("area:{}", t.area)) {
            errors.push(violation("UNKNOWN_LABEL", p("area")));
        }
        if t.owner && !known_labels.contains("decision:owner") {
            errors.push(violation("UNKNOWN_LABEL", p("owner_decision")));
        }
        for (key, value) in [
            ("title", &t.title),
            ("output", &t.output),
            ("escalation", &t.escalation),
        ] {
            if value.trim().is_empty() {
                errors.push(violation("INVALID_FIELD", p(key)));
            }
        }
        if t.files.is_empty() {
            errors.push(violation("INVALID_FIELD", p("files")));
        }
        for (j, file) in t.files.iter().enumerate() {
            if !valid_planned_path(file) {
                errors.push(violation("INVALID_FIELD", format!("{path}/files/{j}")));
            }
        }
        if t.acceptance.is_empty() {
            errors.push(violation("INVALID_FIELD", p("acceptance")));
        }
        for (j, a) in t.acceptance.iter().enumerate() {
            if a.trim().is_empty() {
                errors.push(violation("INVALID_FIELD", format!("{path}/acceptance/{j}")));
            }
        }
        let completion = match t.status.as_str() {
            "done" => "accepted",
            "cancelled" => "not-planned",
            _ => "pending",
        };
        if t.completion != completion {
            errors.push(violation("INVALID_COMPLETION", p("completion")));
        }
        let valid_approval = if t.owner {
            match t.approval.as_str() {
                "pending" => t.evidence.is_empty(),
                "approved" => !t.evidence.is_empty(),
                _ => false,
            }
        } else {
            t.approval == "not-required" && t.evidence.is_empty()
        };
        if !valid_approval
            || (t.status == "needs-decision" && (!t.owner || t.approval != "pending"))
        {
            errors.push(violation("INVALID_APPROVAL", p("owner_approval")));
        }
        for (j, url) in t.evidence.iter().enumerate() {
            if !approval_url(url) {
                errors.push(violation(
                    "INVALID_APPROVAL",
                    format!("{path}/approval_evidence/{j}"),
                ));
            }
        }
        if (t.status == "ready" || t.completion == "accepted")
            && !matches!(t.approval.as_str(), "not-required" | "approved")
        {
            errors.push(violation("OWNER_APPROVAL_REQUIRED", p("owner_approval")));
        }
    }
    if !ambiguous {
        dependencies(&tasks, &ids, &mut errors);
    }
    finish(errors)?;
    Ok(tasks.len())
}
fn dependencies(tasks: &[Task], ids: &BTreeMap<&str, usize>, errors: &mut Vec<ManifestViolation>) {
    let mut indegree = vec![0; tasks.len()];
    let mut followers = vec![Vec::new(); tasks.len()];
    for (i, t) in tasks.iter().enumerate() {
        let mut seen = BTreeSet::new();
        for (j, dep) in t.dependencies.iter().enumerate() {
            let path = format!("/tasks/{i}/dependencies/{j}");
            if !seen.insert(dep) {
                errors.push(violation("DUPLICATE_DEPENDENCY", &path));
                continue;
            }
            if dep == &t.id {
                errors.push(violation("SELF_DEPENDENCY", &path));
            }
            let Some(&index) = ids.get(dep.as_str()) else {
                errors.push(violation("MISSING_DEPENDENCY", &path));
                continue;
            };
            indegree[i] += 1;
            followers[index].push(i);
            if (t.status == "ready" || t.completion == "accepted")
                && tasks[index].completion != "accepted"
            {
                errors.push(violation("UNREADY_DEPENDENCY", path));
            }
        }
    }
    let mut queue: VecDeque<_> = indegree
        .iter()
        .enumerate()
        .filter_map(|(i, n)| (*n == 0).then_some(i))
        .collect();
    let mut visited = 0;
    while let Some(i) = queue.pop_front() {
        visited += 1;
        for &next in &followers[i] {
            indegree[next] -= 1;
            if indegree[next] == 0 {
                queue.push_back(next);
            }
        }
    }
    if visited != tasks.len() {
        errors.push(violation("DEPENDENCY_CYCLE", "/tasks"));
    }
}
fn valid_planned_path(path: &str) -> bool {
    !path.trim().is_empty()
        && !path.starts_with('/')
        && !path.contains(['\\', ':', '\0'])
        && !path.split('/').any(|p| p == "..")
}
fn approval_url(url: &str) -> bool {
    let (base, fragment) = url
        .split_once('#')
        .map_or((url, None), |(a, b)| (a, Some(b)));
    if fragment.is_some_and(|f| {
        f.is_empty()
            || f.chars()
                .any(|c| c.is_whitespace() || c.is_control() || c == '#')
    }) {
        return false;
    }
    let Some(rest) = base.strip_prefix(PREFIX) else {
        return false;
    };
    let Some(number) = rest
        .strip_prefix("issues/")
        .or_else(|| rest.strip_prefix("pull/"))
    else {
        return false;
    };
    !number.starts_with('0')
        && !number.is_empty()
        && number.bytes().all(|b| b.is_ascii_digit())
        && number.parse::<u64>().is_ok_and(|n| n > 0)
}
fn valid_date(date: &str) -> bool {
    let b = date.as_bytes();
    if b.len() != 10
        || b[4] != b'-'
        || b[7] != b'-'
        || ![&b[..4], &b[5..7], &b[8..]]
            .iter()
            .all(|p| p.iter().all(u8::is_ascii_digit))
    {
        return false;
    }
    let year = date[..4].parse::<u32>().unwrap_or(0);
    let month = date[5..7].parse::<usize>().unwrap_or(0);
    let day = date[8..].parse::<u32>().unwrap_or(0);
    if year == 0 || !(1..=12).contains(&month) {
        return false;
    }
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    (1..=days[month - 1]).contains(&day)
}
