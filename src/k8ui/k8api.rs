use std::collections::{HashMap};
use std::convert::TryFrom;
use std::error::Error;
use std::net::SocketAddr;
use std::path::PathBuf;
use base64::Engine;
use base64::engine::general_purpose;
use kube::{Api, Client, Config};
use k8s_openapi::api::core::v1::{ConfigMap, Pod, Secret};
use kube::api::{ListParams, LogParams};
use kube::config::{Kubeconfig, KubeConfigOptions};
use anyhow::Context;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::chrono::Utc;
use k8s_openapi::serde_json;
use crate::k8ui::appstate::{ShortKContainer};
use futures::{AsyncBufReadExt, TryStreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener};
use regex::Regex;


pub struct KubeApis {
    // pub client: Client,
    pub api_pods: Api<Pod>,
    pub api_cfm: Api<ConfigMap>,
    pub api_secrets: Api<Secret>,
    pub api_deployments: Api<Deployment>,
}


pub async fn refresh_client(conf_file_path: &str) -> Client {
    let profile = Kubeconfig::read_from(PathBuf::from(conf_file_path)).unwrap();
    let opts = KubeConfigOptions::default();
    // Client::try_from(Config::from_kubeconfig(&opts).await.unwrap()).unwrap()
    Client::try_from(Config::from_custom_kubeconfig(profile, &opts).await.unwrap()).unwrap()
}

pub async fn refresh_apis(client: &Client, namespace: &str) -> KubeApis {
    let api_pods = Api::<Pod>::namespaced(client.clone(), namespace);
    let api_cfm = Api::<ConfigMap>::namespaced(client.clone(), namespace);
    let api_secrets = Api::<Secret>::namespaced(client.clone(), namespace);
    // let api_services = Api::<Service>::namespaced(client.clone(), namespace);
    let api_deployments = Api::<Deployment>::namespaced(client.clone(), namespace);

    KubeApis {
        api_pods,
        api_cfm,
        api_secrets,
        api_deployments,
    }
}

pub async fn refresh_clients(conf_file_path: &str, namespace: &str) -> Result<KubeApis, Box<dyn Error>> {
    let profile = Kubeconfig::read_from(PathBuf::from(conf_file_path))?;
    let opts = KubeConfigOptions::default();
    println!("nu-ka");
    let client = Client::try_from(Config::from_custom_kubeconfig(profile, &opts).await?)?;
    let api_pods = Api::<Pod>::namespaced(client.clone(), namespace);
    let api_cfm = Api::<ConfigMap>::namespaced(client.clone(), namespace);
    let api_secrets = Api::<Secret>::namespaced(client.clone(), namespace);
    // let api_services = Api::<Service>::namespaced(client.clone(), namespace);
    let api_deployments = Api::<Deployment>::namespaced(client.clone(), namespace);


    match api_pods.list(&ListParams::default()).await {
        Ok(list) => {
            let result: Vec<Pod> = list.into_iter()
                .filter(|p| (&p.metadata.name).as_ref().unwrap().starts_with("quality"))
                .collect();
            println!("{:#?}", result.len());
        }
        Err(e) => println!("{:?}", e),
    };
    Ok(KubeApis {
        api_pods,
        api_cfm,
        api_secrets,
        api_deployments,
    })
}

//https://github.com/kube-rs/kube/blob/main/examples/configmapgen_controller.rs
pub async fn refresh_pod_list(prefix: &str, api_pods: &Api<Pod>, api_cfm: &Api<ConfigMap>, api_secrets: &Api<Secret>) -> Result<Vec<ShortKContainer>, Box<dyn Error>> {
    let mut refreshed_pods = Vec::new();

    let list: Vec<Pod> = api_pods.list(&ListParams::default()).await?.into_iter()
        .filter(|p| (&p.metadata.name).as_ref().unwrap().starts_with(prefix))
        .collect();

    for x in &list {        //https://users.rust-lang.org/t/nested-match-hell-in-rust/57628/4
        let spec = x.spec.as_ref().unwrap();
        let container = &spec.containers[0];
        let start = x.metadata.creation_timestamp.as_ref().unwrap().0;
        let now = Utc::now();
        let diff = now - start;

        let pod_name = x.metadata.name.clone().unwrap();
        let age = format!("{}d, {}h, {}m", diff.num_days(), diff.num_hours() - diff.num_days() * 24, diff.num_minutes() - diff.num_hours() * 60);
        let image = container.image.as_ref().unwrap().to_owned();

        let mut restarts = 0u32;
        let mut status = "";
        if let Some(data) = &x.status {
            if let Some(statuses) = &data.container_statuses {
                let s = statuses.get(0).unwrap();
                restarts = s.restart_count as u32;
                if let Some(state) = &s.state {
                    if state.running.is_some() {
                        status = "Running";
                    }

                    if state.waiting.is_some() {
                        status = "Waiting";
                    }

                    if state.terminated.is_some() {
                        status = "Terminated";
                    }
                }
            }
        }

        let mut ports = HashMap::new();
        if let Some(data) = container.ports.as_ref() {
            ports = data.iter()
                .map(|p| { (p.protocol.clone().unwrap(), p.container_port as u16) })
                .collect::<HashMap<String, u16>>();
        }


        let mut cfm = HashMap::new();
        let mut sm = HashMap::new();
        if let Some(envs) = container.env_from.as_ref() {
            for ee in envs {
                if let Some(confmap) = &ee.config_map_ref {
                    let name = confmap.name.as_ref();
                    if let Some(data) = api_cfm.get(name).await.unwrap().data {
                        cfm = data.into_iter()
                            .map(|e| (e.0, e.1))
                            .collect::<HashMap<String, String>>();
                    }
                }
                if let Some(secrets) = &ee.secret_ref {
                    let name = secrets.name.as_ref();
                    if let Some(data) = api_secrets.get(name).await.unwrap().data {
                        sm = data.into_iter()
                            .map(|e| {
                                let key = e.0;
                                let mut val = serde_json::to_string(&e.1).unwrap().replace("\"", "");
                                val = String::from_utf8(general_purpose::STANDARD.decode(val).unwrap()).unwrap();
                                (key, val)
                            })
                            .collect::<HashMap<String, String>>();
                    }
                }
            }
        }

        let c = ShortKContainer::new(pod_name, age, image, status.to_owned(), restarts, ports, cfm, sm);
        println!("{:#?}",&c);

        refreshed_pods.push(c);
    }

    Ok(refreshed_pods)
}

pub async fn logs(pod_name: String, api_pods: &Api<Pod>) -> anyhow::Result<Vec<String>> {
    let mut logs = api_pods
        .log_stream(pod_name.as_str(), &LogParams {
            follow: false,
            tail_lines: Some(100),
            timestamps: false,
            ..LogParams::default()
        })
        .await?
        .lines();

    println!("lines");

    let regex = Regex::new(r"\u001B\[\d*m").unwrap();
    let mut result = vec![];
    while let Some(mut line) = logs.try_next().await? { //if follow
        let fixed_str = regex.replace_all(line.as_str(), "").into_owned();
        result.push(fixed_str);
    }
    println!("ok... {}", result.len());
    Ok(result)
}

pub async fn forward_connection(api_pods: Api<Pod>, pod_name: &str, port: u16,
                                mut client_conn: impl AsyncRead + AsyncWrite + Unpin, ) -> anyhow::Result<()> {
    let mut forwarder = api_pods.portforward(pod_name, &[port]).await?;
    let mut upstream_conn = forwarder.take_stream(port).context("port not found in forwarder")?;
    tokio::io::copy_bidirectional(&mut client_conn, &mut upstream_conn).await?;
    drop(upstream_conn);//reuse connection?
    forwarder.join().await?;
    println!("connection closed");
    Ok(())
}


pub async fn forward_connection2(api_pods: Api<Pod>, pod_name: &str, port: u16) -> anyhow::Result<()> {
    let mut forwarder = api_pods.portforward(pod_name, &[port]).await?;
    let mut upstream_conn = forwarder.take_stream(port).context("port not found in forwarder")?;

    let addr = SocketAddr::from(([127, 0, 0, 1], port + 1));
    let listener = TcpListener::bind(addr).await?;
    let (mut socket, _) = listener.accept().await?;
    println!("spawn bidir copy");

    tokio::spawn(async move {
        tokio::io::copy_bidirectional(&mut socket, &mut upstream_conn).await.unwrap();
        println!("some task cant get here");
    });

    forwarder.join().await?;
    println!("connection closed");
    Ok(())
}
