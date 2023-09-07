use std::time::Duration;

use config::get_config;
use error::WorkerError;
use gpg::provision_key;
use log::debug;
use proto::{worker_service_client::WorkerServiceClient, JobStatus, Worker};
use tokio::time::interval;
use tonic::{metadata::MetadataValue, transport::Channel, Code, Request};

mod config;
mod error;
mod gpg;
mod logging;

#[allow(non_snake_case)]
mod proto {
    tonic::include_proto!("worker");
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[allow(while_true)]
#[tokio::main]
async fn main() -> Result<(), WorkerError> {
    // Load env
    if dotenvy::from_filename(".env.local").is_err() {
        dotenvy::dotenv().ok();
    }
    // load config
    let config = get_config().expect("Failed to create config");
    //init logging
    logging::init(&config.log_level, &None).expect("Failed to init logging, check logging config");
    // Make grpc client
    let url = config.url.clone();
    let token: MetadataValue<_> = config
        .token
        .clone()
        .parse()
        .expect("Failed to parse worker token");
    let channel = Channel::from_shared(url.clone())
        .expect("Failed to create grpc channel")
        .connect()
        .await
        .expect("Failed to connect grpc channel, check config.");
    debug!(
        "Tonic channel connected.\nGRPC Connected on URL: {}",
        &config.url
    );
    let mut client = WorkerServiceClient::with_interceptor(channel, move |mut req: Request<()>| {
        req.metadata_mut().insert("authorization", token.clone());
        Ok(req)
    });
    debug!("Tonic client crated");
    let worker_request = tonic::Request::new(Worker {
        id: config.worker_id.clone(),
    });
    //register worker
    match client.register_worker(worker_request).await {
        Ok(_) => {}
        Err(e) => {
            if e.code() != Code::AlreadyExists {
                panic!("Failed to register worker, {}", e.to_string());
            }
            debug!("Worker already registered, proceeding.");
        }
    };
    // worker loop
    let period = Duration::from_secs(config.job_interval.clone());
    let mut client_interval = interval(period);
    loop {
        client_interval.tick().await;
        // attempt to get job
        let worker_request = tonic::Request::new(Worker {
            id: config.worker_id.clone(),
        });
        if let Ok(job_response) = client.get_job(worker_request).await {
            let job_data = job_response.into_inner();
            debug!("Job received : {:?}", &job_data);
            match provision_key(&config, &job_data).await {
                Ok(key_info) => {
                    let job_status: JobStatus = JobStatus {
                        id: config.worker_id.clone(),
                        job_id: job_data.job_id.clone(),
                        success: true,
                        public_key: key_info.pgp,
                        ssh_key: key_info.ssh,
                        fingerprint: key_info.fingerprint,
                        error: "".into(),
                    };
                    let request = tonic::Request::new(job_status);
                    let _ = client.set_job_done(request).await;
                }
                Err(err) => {
                    debug!("Provisioning FAILED: {}", err.to_string());
                    let job_status: JobStatus = JobStatus {
                        id: config.worker_id.clone(),
                        job_id: job_data.job_id.clone(),
                        success: false,
                        public_key: "".into(),
                        ssh_key: "".into(),
                        fingerprint: "".into(),
                        error: err.to_string(),
                    };
                    let request = tonic::Request::new(job_status);
                    let _ = client.set_job_done(request).await;
                    debug!("Job result sent");
                }
            }
        }
    }
}
