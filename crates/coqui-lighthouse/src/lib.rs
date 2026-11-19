//! Parse Lighthouse JSON into a compact summary for storage and display.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LighthouseAudit {
    pub id: String,
    pub title: String,
    pub score: Option<f64>,
    pub display_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LighthouseSummary {
    pub performance: Option<f64>,
    pub accessibility: Option<f64>,
    pub best_practices: Option<f64>,
    pub seo: Option<f64>,
    pub audits: Vec<LighthouseAudit>,
    pub final_url: Option<String>,
    pub fetch_time: Option<String>,
}

pub fn summarize(report: &str) -> Result<LighthouseSummary, String> {
    let value: Value = serde_json::from_str(report).map_err(|error| error.to_string())?;
    let category = |id: &str| -> Option<f64> {
        value
            .pointer(&format!("/categories/{id}/score"))
            .and_then(Value::as_f64)
    };
    let mut audits = Vec::new();
    if let Some(map) = value.get("audits").and_then(Value::as_object) {
        for (id, audit) in map {
            let score = audit.get("score").and_then(Value::as_f64);
            let mode = audit
                .get("scoreDisplayMode")
                .and_then(Value::as_str)
                .unwrap_or("");
            if score.is_none() || mode == "notApplicable" || mode == "manual" || score == Some(1.0)
            {
                continue;
            }
            audits.push(LighthouseAudit {
                id: id.clone(),
                title: audit
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or(id)
                    .to_string(),
                score,
                display_value: audit
                    .get("displayValue")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            });
        }
    }
    audits.sort_by(|a, b| {
        a.score
            .unwrap_or(1.0)
            .partial_cmp(&b.score.unwrap_or(1.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    audits.truncate(12);
    Ok(LighthouseSummary {
        performance: category("performance"),
        accessibility: category("accessibility"),
        best_practices: category("best-practices"),
        seo: category("seo"),
        audits,
        final_url: value
            .get("finalDisplayedUrl")
            .or_else(|| value.get("finalUrl"))
            .and_then(Value::as_str)
            .map(str::to_string),
        fetch_time: value
            .get("fetchTime")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_categories_and_failing_audits() {
        let report = r#"{
            "finalDisplayedUrl": "https://example.com/",
            "fetchTime": "2026-11-19T00:00:00.000Z",
            "categories": {
                "performance": {"score": 0.42},
                "accessibility": {"score": 0.93},
                "best-practices": {"score": 1.0},
                "seo": {"score": 0.8}
            },
            "audits": {
                "largest-contentful-paint": {"score": 0.2, "title": "LCP", "displayValue": "5.2 s", "scoreDisplayMode": "numeric"},
                "uses-responsive-images": {"score": 0.5, "title": "Responsive images", "displayValue": "Potential savings", "scoreDisplayMode": "numeric"},
                "network-requests": {"score": null, "title": "Requests", "scoreDisplayMode": "informative"},
                "server-response-time": {"score": 1.0, "title": "TTFB", "scoreDisplayMode": "numeric"}
            }
        }"#;
        let summary = summarize(report).unwrap();
        assert_eq!(summary.performance, Some(0.42));
        assert_eq!(summary.seo, Some(0.8));
        assert_eq!(summary.audits.len(), 2);
        assert_eq!(summary.audits[0].id, "largest-contentful-paint");
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(summarize("not json").is_err());
    }
}
