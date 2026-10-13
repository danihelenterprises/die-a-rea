//! Machine-readable report formats: SARIF 2.1.0 and JUnit XML.

use direa_core::ResistanceReport;

fn level(exposure: f64) -> &'static str {
    if exposure >= 0.75 {
        "error"
    } else if exposure >= 0.4 {
        "warning"
    } else {
        "note"
    }
}

/// Render a report as SARIF 2.1.0 (for GitHub code scanning and similar).
pub fn to_sarif(report: &ResistanceReport) -> String {
    let results: Vec<serde_json::Value> = report
        .measurements
        .iter()
        .map(|m| {
            serde_json::json!({
                "ruleId": format!("die-a-rea/{}", m.axis.as_str()),
                "level": level(m.hardened),
                "message": { "text": format!(
                    "{} exposure {:.2} -> {:.2} (resistance {:.2})",
                    m.axis.as_str(), m.baseline, m.hardened, m.resistance()
                ) },
                "properties": { "baseline": m.baseline, "hardened": m.hardened }
            })
        })
        .collect();
    let doc = serde_json::json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": {
                "name": "die-a-rea",
                "informationUri": "https://github.com/danihelenterprises/die-a-rea",
                "rules": [{
                    "id": "die-a-rea/RRS",
                    "shortDescription": { "text": "Reverse-engineering resistance" }
                }]
            }},
            "results": results,
            "properties": { "target": report.target, "rrs": report.rrs, "grade": report.grade.as_str() }
        }]
    });
    serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".to_string())
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Render a report as JUnit XML (one test case per axis; exposed axes fail).
pub fn to_junit(report: &ResistanceReport) -> String {
    let mut cases = String::new();
    let mut failures = 0usize;
    for m in &report.measurements {
        let exposed = m.hardened > 0.5;
        if exposed {
            failures += 1;
        }
        cases.push_str(&format!(
            "    <testcase classname=\"die-a-rea\" name=\"{}\">",
            escape(m.axis.as_str())
        ));
        if exposed {
            cases.push_str(&format!(
                "<failure message=\"exposed (exposure {:.2})\">{}</failure>",
                m.hardened,
                escape(&format!(
                    "baseline {:.2} -> hardened {:.2}",
                    m.baseline, m.hardened
                ))
            ));
        }
        cases.push_str("</testcase>\n");
    }
    let tests = report.measurements.len();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <testsuites name=\"die-a-rea\" tests=\"{tests}\" failures=\"{failures}\">\n\
         \x20 <testsuite name=\"{}\" tests=\"{tests}\" failures=\"{failures}\" RRS=\"{}\" grade=\"{}\">\n\
         {cases}\
         \x20 </testsuite>\n</testsuites>\n",
        escape(&report.target),
        report.rrs,
        report.grade.as_str()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use direa_core::{Axis, AxisMeasurement};

    fn sample() -> ResistanceReport {
        ResistanceReport::new(
            "app.js",
            vec![
                AxisMeasurement::new(Axis::StringExposure, 1.0, 0.1),
                AxisMeasurement::new(Axis::SymbolExposure, 1.0, 0.9),
            ],
            vec![],
        )
    }

    #[test]
    fn sarif_is_valid_json_with_results() {
        let text = to_sarif(&sample());
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["version"], "2.1.0");
        assert_eq!(v["runs"][0]["results"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn junit_reports_failures() {
        let text = to_junit(&sample());
        assert!(text.contains("<testsuites"));
        assert!(text.contains("failures=\"1\""), "exposed axis should fail");
        assert!(text.contains("name=\"symbol-exposure\""));
    }
}
