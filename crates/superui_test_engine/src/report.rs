//! Terminal summary + HTML report generation from [`crate::trace::TestResult`] traces.

use crate::trace::TestResult;

/// Print a summary of test results to stdout.
///
/// Returns `true` if all tests passed, `false` otherwise.
pub fn print_summary(results: &[(String, Vec<TestResult>)]) -> bool {
    let mut all_passed = true;
    for (file, tests) in results {
        for t in tests {
            let mark = if t.passed {
                "PASS"
            } else {
                all_passed = false;
                "FAIL"
            };
            println!("[{mark}] {file} \u{203a} {}", t.name);
            if let Some(e) = &t.error {
                println!("       {e}");
            }
        }
    }
    all_passed
}

/// Write an HTML report to `path`.
///
/// The report lists each spec file as a section, then each test as a
/// pass/fail heading, followed by collapsible step details including the
/// serialised DOM-after snapshot.
pub fn write_html_report(
    path: &std::path::Path,
    results: &[(String, Vec<TestResult>)],
) -> std::io::Result<()> {
    let mut html = String::from(
        "<!doctype html><meta charset=utf-8><title>superui-test</title>\
         <style>body{font-family:monospace;padding:1em}pre{background:#f4f4f4;padding:.5em;overflow:auto}\
         details{margin:.25em 0}</style><body>",
    );

    let total_tests: usize = results.iter().map(|(_, ts)| ts.len()).sum();
    let total_pass: usize = results
        .iter()
        .flat_map(|(_, ts)| ts.iter())
        .filter(|t| t.passed)
        .count();
    let total_fail = total_tests - total_pass;

    html.push_str(&format!(
        "<h1>superui test report</h1>\
         <p>{total_pass} passed, {total_fail} failed, {total_tests} total</p>"
    ));

    for (file, tests) in results {
        html.push_str(&format!("<h2>{}</h2>", html_escape(file)));
        for t in tests {
            let color = if t.passed { "green" } else { "red" };
            let status = if t.passed { "passed" } else { "failed" };
            html.push_str(&format!(
                "<h3 style=\"color:{color}\">{} \u{2014} {status}</h3>",
                html_escape(&t.name)
            ));
            if let Some(e) = &t.error {
                html.push_str(&format!("<pre>{}</pre>", html_escape(e)));
            }
            for s in &t.steps {
                let step_color = match &s.status {
                    crate::trace::StepStatus::Ok => "green",
                    crate::trace::StepStatus::Failed(_) => "red",
                };
                html.push_str(&format!(
                    "<details><summary style=\"color:{step_color}\">step {}: {}</summary><pre>{}</pre></details>",
                    s.index,
                    html_escape(&s.action),
                    html_escape(&s.dom_after)
                ));
            }
        }
    }

    html.push_str("</body>");
    std::fs::write(path, html)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::{print_summary, write_html_report};
    use crate::trace::{Step, StepStatus, TestResult};

    fn make_results(passed: bool) -> Vec<(String, Vec<TestResult>)> {
        vec![(
            "foo.spec.ts".to_string(),
            vec![TestResult {
                name: "my test".to_string(),
                passed,
                error: if passed { None } else { Some("boom".to_string()) },
                steps: vec![Step {
                    index: 0,
                    action: "click .btn".to_string(),
                    status: StepStatus::Ok,
                    dom_after: "<body></body>".to_string(),
                    screenshot: None,
                }],
            }],
        )]
    }

    #[test]
    fn print_summary_returns_true_when_all_pass() {
        let results = make_results(true);
        assert!(print_summary(&results));
    }

    #[test]
    fn print_summary_returns_false_when_any_fail() {
        let results = make_results(false);
        assert!(!print_summary(&results));
    }

    #[test]
    fn write_html_report_creates_file() {
        let dir = std::env::temp_dir().join("superui_report_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("report.html");
        let results = make_results(false);
        write_html_report(&path, &results).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("failed"), "expected 'failed' in HTML");
        assert!(content.contains("my test"), "expected test name in HTML");
        assert!(content.contains("boom"), "expected error message in HTML");
    }
}
