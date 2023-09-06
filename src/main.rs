use std::{str::FromStr, time::Duration};

use config::get_config;
use error::WorkerError;
use gpg::provision_key;
use proto::{worker_service_client::WorkerServiceClient, GetJobResponse, Worker};
use tokio::time::interval;
use tonic::{
    metadata::{MetadataMap, MetadataValue},
    service::Interceptor,
    transport::Channel,
    Request, Status,
};

mod client;
mod config;
mod error;
mod gpg;

#[macro_use]
extern crate log;

#[allow(non_snake_case)]
mod proto {
    tonic::include_proto!("worker");
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> Result<(), WorkerError> {
    // Load env
    if dotenvy::from_filename(".env.local").is_err() {
        dotenvy::dotenv().ok();
    }
    let config = get_config().expect("Failed to create config");
    let url = config.url.clone();
    let token: MetadataValue<_> = config
        .token
        .clone()
        .parse()
        .expect("Failed to parse worker token");
    let channel = Channel::from_shared(url.clone())
        .expect("Failed to create grpc channel")
        .connect()
        .await?;
    let mut client = WorkerServiceClient::with_interceptor(channel, move |mut req: Request<()>| {
        req.metadata_mut().insert("authorization", token.clone());
        Ok(req)
    });
    // authorization token
    let period = Duration::from_secs(config.job_interval.clone());
    let mut client_interval = interval(period);
    let worker_request = tonic::Request::new(Worker {
        id: config.worker_id.clone(),
    });
    //register worker
    client.register_worker(worker_request).await?;
    loop {
        client_interval.tick().await;
        // attempt to get job
        let worker_request = tonic::Request::new(Worker {
            id: config.worker_id.clone(),
        });
        if let Ok(job_response) = client.get_job(worker_request).await {
            let key_info = provision_key(&config, &job_response.into_inner()).await?;
            println!("{:?}", key_info);
        }
    }
}
