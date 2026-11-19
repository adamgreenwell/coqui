use coqui_lighthouse::summarize;

fn main() {
    let path = std::env::args().nth(1).expect("usage: parse <report.json>");
    let report = std::fs::read_to_string(path).expect("read report");
    match summarize(&report) {
        Ok(summary) => println!("{}", serde_json::to_string_pretty(&summary).unwrap()),
        Err(error) => eprintln!("parse failed: {error}"),
    }
}
