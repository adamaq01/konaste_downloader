mod error;
mod reporter;
mod resources;

use std::{
    fmt::{Debug, Formatter},
    path::PathBuf,
    sync::Arc,
};

use bon::Builder;
use clap::Parser;
pub use error::*;
pub use reporter::*;
use reqwest::Client;
pub use resources::*;
use sha2::{Digest, Sha256};
use tokio::{runtime::Runtime, sync::Semaphore};
use tokio_util::sync::CancellationToken;

/// A simple resource downloader
#[derive(Parser, Builder)]
#[command(version, about)]
pub struct KDownloader {
    /// URL to fetch resource information from
    #[arg(short, long)]
    #[builder(into)]
    url: String,

    /// Path to save downloaded resources
    #[arg(short, long, default_value = ".")]
    #[builder(into, default = ".")]
    output: PathBuf,

    /// Number of concurrent downloads
    #[arg(short, long, default_value_t = 4)]
    #[builder(default = 4)]
    concurrency: usize,

    /// Number of threads to use for downloading, defaults to number of CPU
    /// cores
    #[arg(short, long, default_value_t = 0)]
    #[builder(default = 0)]
    threads: usize,

    #[arg(skip = None)]
    reporter: Option<Arc<dyn Reporter + Send + Sync>>,
}

impl KDownloader {
    pub fn with_reporter<R>(mut self, reporter: R) -> Self
    where
        R: Reporter + Send + Sync + 'static,
    {
        self.reporter = Some(Arc::new(reporter));
        self
    }

    pub fn run(&self, client: Client) -> Result<(), Error> {
        let runtime = match self.threads {
            0 => tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?,
            1 => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?,
            n => tokio::runtime::Builder::new_multi_thread()
                .worker_threads(n)
                .enable_all()
                .build()?,
        };
        self.run_on(runtime, client)
    }

    #[inline(always)]
    pub fn run_on(&self, runtime: Runtime, client: Client) -> Result<(), Error> {
        runtime.block_on(self.run_inner(client))
    }

    async fn run_inner(&self, client: Client) -> Result<(), Error> {
        let response = client.get(&self.url).send().await?.error_for_status()?;

        let (resource_info, ri_bin) = {
            let body = response.bytes().await?;
            let (xml, ri_bin): (_, Option<Vec<_>>) = match kbinxml::from_binary(body.clone()) {
                Ok((nodes, _)) => {
                    (kbinxml::to_text_xml(&nodes).map_err(|err| Error::ConvertResourceInfo(err))?, Some(body.into()))
                }
                Err(_) => (body.into(), None),
            };

            let mut resource_info: ResourceInfo = quick_xml::de::from_reader(xml.as_slice())
                .map_err(|err| Error::ParseResourceInfo(err))?;
            // Remove entries with empty URLs
            resource_info
                .files
                .retain(|file| !file.url.is_empty());
            (resource_info, ri_bin)
        };

        let total = resource_info.files.len();
        let total_len = resource_info
            .files
            .iter()
            .map(|f| f.size as usize)
            .sum::<usize>();

        let cancellation_token = CancellationToken::new();
        let semaphore = Arc::new(Semaphore::new(self.concurrency));
        let mut handles = Vec::new();
        for file in resource_info.files {
            let permit = match tokio::select! {
                _ = cancellation_token.cancelled() => None,
                permit = semaphore.clone().acquire_owned() => permit.ok(),
            } {
                Some(permit) => permit,
                None => break,
            };
            let client = client.clone();
            let output_path = self.output.clone();
            let cancellation_token = cancellation_token.clone();
            let reporter = self.reporter.clone();

            let handle = tokio::spawn(async move {
                // Keep the permit alive for the duration of the task
                let _permit = permit;

                let status = tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        Status::Cancelled
                    }
                    result = file.fetch(client, output_path) => {
                        match result {
                            Err(err) => {
                                // On error, cancel all other tasks
                                cancellation_token.cancel();
                                return Err(err);
                            }
                            Ok(res) => res,
                        }
                    }
                };

                if let Some(reporter) = reporter {
                    reporter.report(file, status, total, total_len);
                }

                Ok(())
            });

            handles.push(handle);
        }

        for handle in handles {
            handle
                .await
                .map_err(|err| Error::InternalError(err.to_string()))??;
        }

        if let Some(ri_bin) = ri_bin {
            let output_path = self.output.join("ri.bin");
            tokio::fs::write(&output_path, &ri_bin).await?;
        }

        Ok(())
    }
}

impl FileResource {
    async fn fetch(&self, client: Client, output_path: PathBuf) -> Result<Status, Error> {
        let output_path = output_path.join(&self.path);
        if let Ok(content) = tokio::fs::read(&output_path).await {
            // Compare hashes
            let hash = format!("{:x}", Sha256::digest(&content));
            if hash == self.sum {
                // File is already up to date
                return Ok(Status::Skipped);
            }
        }

        let response = client.get(&self.url).send().await?.error_for_status()?;
        let bytes = response.bytes().await?;

        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&output_path, &bytes).await?;

        Ok(Status::Downloaded)
    }
}

impl Debug for KDownloader {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KDownloader")
            .field("url", &self.url)
            .field("output", &self.output)
            .field("concurrency", &self.concurrency)
            .field("threads", &self.threads)
            .finish()
    }
}
