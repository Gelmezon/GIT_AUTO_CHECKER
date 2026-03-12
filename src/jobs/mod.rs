pub mod git_review;
pub mod test_gen;

#[derive(Debug, Clone)]
pub struct JobOutput {
    pub task_result: String,
    pub content: String,
    pub summary: String,
    pub repo_name: Option<String>,
    pub report_path: Option<String>,
    pub commit_range: Option<String>,
}
