use std::time::Duration;

use config::get_config;
use error::WorkerError;
use gpg::provision_key;
use log::{debug, info};
use proto::{worker_service_client::WorkerServiceClient, JobStatus, Worker};
use tokio::time::interval;
use tonic::{
    metadata::MetadataValue,
    transport::{Certificate, ClientTlsConfig, Endpoint},
    Code, Request,
};
use which::which;

use crate::gpg::get_gpg_command;

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
    // Check required binaries
    let gpg_command = get_gpg_command();
    if which("ykman").is_err() {
        panic!("'ykman' not found!");
    }
    // Make grpc client
    let mut url = config.url.clone();
    if config.grpc_ca.is_some() {
        url = url.replace("http://", "https://");
    }
    let token: MetadataValue<_> = config
        .token
        .clone()
        .parse()
        .expect("Failed to parse worker token");
    let endpoint = Endpoint::from_shared(url.clone())?
        .http2_keep_alive_interval(Duration::from_secs(10))
        .tcp_keepalive(Some(Duration::from_secs(10)));
    let endpoint = if let Some(ca) = &config.grpc_ca {
        let ca = std::fs::read_to_string(ca)?;
        let tls = ClientTlsConfig::new().ca_certificate(Certificate::from_pem(ca));
        info!("TLS configured");
        endpoint.tls_config(tls)?
    } else {
        endpoint
    };
    let channel = endpoint.connect_lazy();
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
                panic!("Failed to register worker, {}", e);
            }
            debug!("Worker already registered, proceeding.");
        }
    };
    // worker loop
    let period = Duration::from_secs(2);
    let mut client_interval = interval(period);
    loop {
        client_interval.tick().await;
        // attempt to get job
        let worker_request = tonic::Request::new(Worker {
            id: config.worker_id.clone(),
        });
        if let Ok(job_response) = client.get_job(worker_request).await {
            let job_data = job_response.into_inner();
            debug!("Job received: {job_data:?}");
            match provision_key(&config, &job_data, gpg_command).await {
                Ok(key_info) => {
                    let job_status: JobStatus = JobStatus {
                        id: config.worker_id.clone(),
                        job_id: job_data.job_id,
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
                        job_id: job_data.job_id,
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
