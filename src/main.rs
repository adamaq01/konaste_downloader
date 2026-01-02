use std::sync::atomic::{AtomicUsize, Ordering};

use clap::Parser;
use konaste_downloader::{FileResource, KDownloader, Reporter, Status};
use reqwest::Client;

fn main() {
    match KDownloader::parse()
        .with_reporter(CLIReporter::default())
        .run(Client::new())
    {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct CLIReporter {
    fetched_files: AtomicUsize,
    fetched_bytes: AtomicUsize,
}

impl Reporter for CLIReporter {
    fn report(&self, file: FileResource, status: Status, total_files: usize, total_bytes: usize) {
        match status {
            Status::Downloaded | Status::Skipped => {
                let done = self.fetched_files.fetch_add(1, Ordering::SeqCst) + 1;
                let done_bytes = self
                    .fetched_bytes
                    .fetch_add(file.size as usize, Ordering::SeqCst)
                    + (file.size as usize);
                let pct = (done_bytes as f64 / total_bytes as f64) * 100.0;
                println!("Progress: {}/{} files ({:.2}%)", done, total_files, pct);
            }
            Status::Cancelled => {}
        }
    }
}
